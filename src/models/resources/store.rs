//! Authoritative, incrementally maintained collection for one resource kind.
//!
//! The store is owned by the watch task (never by the UI thread). Objects live in
//! a `BTreeMap` ordered by `(namespace, name)`, so rows are stable and sorted
//! without a separate sort pass. A UID index lets deletes/updates find the prior
//! key in O(1) even if metadata changes.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use super::Item;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    pub namespace: String,
    pub name: String,
}

impl ObjectKey {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn from_item(item: &Item) -> Self {
        Self::new(item.namespace.clone(), item.name.clone())
    }
}

/// Immutable view of a store handed to the UI. Cloning a snapshot is O(1).
#[derive(Clone, Debug)]
pub struct ResourceSnapshot {
    items: Arc<[Arc<Item>]>,
    revision: u64,
    phase: SnapshotPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotPhase {
    Loading,
    Ready,
    Error(String),
}

impl ResourceSnapshot {
    pub fn loading() -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            revision: 0,
            phase: SnapshotPhase::Loading,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            revision: 0,
            phase: SnapshotPhase::Error(message.into()),
        }
    }

    pub fn items(&self) -> &[Arc<Item>] {
        &self.items
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn phase(&self) -> &SnapshotPhase {
        &self.phase
    }
}

/// Accumulates watch events; the repository flushes snapshots at a bounded rate.
pub struct ResourceStore {
    objects: BTreeMap<ObjectKey, Arc<Item>>,
    by_uid: HashMap<String, ObjectKey>,
    revision: u64,
    pending: usize,
}

impl Default for ResourceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceStore {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            by_uid: HashMap::new(),
            revision: 0,
            pending: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn pending(&self) -> usize {
        self.pending
    }

    pub fn reset<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = Item>,
    {
        self.objects.clear();
        self.by_uid.clear();
        for item in items {
            self.upsert(item);
        }
        self.pending = 0;
    }

    pub fn upsert(&mut self, item: Item) {
        let item = Arc::new(item);
        let key = ObjectKey::from_item(&item);
        if !item.uid.is_empty()
            && let Some(previous) = self.by_uid.insert(item.uid.clone(), key.clone())
            && previous != key
        {
            self.objects.remove(&previous);
        }
        self.objects.insert(key, item);
        self.revision = self.revision.wrapping_add(1);
        self.pending = self.pending.saturating_add(1);
    }

    pub fn remove(&mut self, key: &ObjectKey) {
        if let Some(item) = self.objects.remove(key)
            && !item.uid.is_empty()
        {
            self.by_uid.remove(&item.uid);
        }
        self.revision = self.revision.wrapping_add(1);
        self.pending = self.pending.saturating_add(1);
    }

    pub fn snapshot(&mut self, phase: SnapshotPhase) -> ResourceSnapshot {
        self.pending = 0;
        ResourceSnapshot {
            items: self.objects.values().cloned().collect(),
            revision: self.revision,
            phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod(name: &str, namespace: &str, uid: &str) -> Item {
        let mut item = Item::new(name.into(), Some(namespace.into()));
        item.uid = uid.into();
        item
    }

    #[test]
    fn keeps_rows_sorted_and_replaces_by_key() {
        let mut store = ResourceStore::new();
        store.upsert(pod("b", "ns-2", "1"));
        store.upsert(pod("a", "ns-1", "2"));
        store.upsert(pod("c", "ns-2", "3"));

        let mut updated = pod("b", "ns-2", "1");
        updated.status = "crashloopbackoff".into();
        store.upsert(updated);

        let snapshot = store.snapshot(SnapshotPhase::Ready);
        let names: Vec<_> = snapshot.items().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
        assert_eq!(snapshot.items()[1].status, "crashloopbackoff");
        assert_eq!(snapshot.revision(), 4);
    }

    #[test]
    fn removes_by_key_and_uid() {
        let mut store = ResourceStore::new();
        store.upsert(pod("a", "ns", "uid-a"));
        store.remove(&ObjectKey::new("ns", "a"));
        assert_eq!(store.len(), 0);
        assert!(store.by_uid.is_empty());
    }

    #[test]
    fn handles_150_namespaces_with_100_pods_each() {
        let mut store = ResourceStore::new();
        for namespace in 0..150 {
            for index in 0..100 {
                store.upsert(pod(
                    &format!("pod-{index:03}"),
                    &format!("namespace-{namespace:03}"),
                    &format!("uid-{namespace}-{index}"),
                ));
            }
        }
        assert_eq!(store.len(), 15_000);

        let mut updated = pod("pod-050", "namespace-075", "uid-75-50");
        updated.status = "crashloopbackoff".into();
        store.upsert(updated);
        store.remove(&ObjectKey::new("namespace-000", "pod-000"));

        let snapshot = store.snapshot(SnapshotPhase::Ready);
        assert_eq!(snapshot.items().len(), 14_999);
        assert_eq!(snapshot.items()[0].namespace, "namespace-000");
        assert_eq!(snapshot.items()[0].name, "pod-001");
        assert!(
            snapshot
                .items()
                .windows(2)
                .all(|pair| ObjectKey::from_item(&pair[0]) <= ObjectKey::from_item(&pair[1]))
        );
    }

    #[test]
    fn reset_discards_objects_missing_from_a_relist() {
        let mut store = ResourceStore::new();
        store.upsert(pod("a", "ns", "1"));
        store.reset([pod("b", "ns", "2")]);
        let snapshot = store.snapshot(SnapshotPhase::Ready);
        assert_eq!(snapshot.items().len(), 1);
        assert_eq!(snapshot.items()[0].name, "b");
    }
}
