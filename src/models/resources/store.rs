//! Authoritative, incrementally maintained collection for one resource kind.
//!
//! The store is owned by the watch task (never by the UI thread). Objects live in
//! a `BTreeMap` ordered by `(namespace, name)`, so rows are stable and sorted
//! without a separate sort pass. A UID index lets deletes/updates find the prior
//! key in O(1) even if metadata changes.

use std::collections::{BTreeMap, HashMap, HashSet};
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
    phase: SnapshotPhase,
    mode: WatchMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotPhase {
    Loading,
    Ready,
    Error(String),
}

/// How the snapshot is kept up to date. `Poll` carries the cadence so the UI
/// can show and adjust it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchMode {
    /// Live watch stream.
    Watch,
    /// Watch is not permitted; the store is re-listed on this cadence.
    Poll(PollRate),
}

/// Automatic re-list cadence while polling. `Off` disables the timer; a
/// manual refresh still re-lists once. Persisted per resource kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PollRate {
    /// No automatic polling.
    Off,
    /// Poll every this many seconds.
    Every(u64),
}

impl PollRate {
    pub const DEFAULT: PollRate = PollRate::Every(5);
}

impl Default for PollRate {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ResourceSnapshot {
    pub fn loading() -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            phase: SnapshotPhase::Loading,
            mode: WatchMode::Watch,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            phase: SnapshotPhase::Error(message.into()),
            mode: WatchMode::Watch,
        }
    }

    pub fn with_mode(mut self, mode: WatchMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn items(&self) -> &[Arc<Item>] {
        &self.items
    }

    pub fn phase(&self) -> &SnapshotPhase {
        &self.phase
    }

    pub fn mode(&self) -> WatchMode {
        self.mode
    }
}

/// Accumulates watch events; the repository flushes snapshots at a bounded rate.
pub struct ResourceStore {
    objects: BTreeMap<ObjectKey, Arc<Item>>,
    by_uid: HashMap<String, ObjectKey>,
    pending: usize,
    /// Keys seen since [`begin_sync`](Self::begin_sync). Present only while a
    /// list is being merged into the store.
    sync_seen: Option<HashSet<ObjectKey>>,
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
            pending: 0,
            sync_seen: None,
        }
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn pending(&self) -> usize {
        self.pending
    }

    #[cfg(test)]
    pub fn reset<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = Item>,
    {
        self.objects.clear();
        self.by_uid.clear();
        self.sync_seen = None;
        for item in items {
            self.upsert(item);
        }
        self.pending = 0;
    }

    /// Start merging a full list into the store. The old objects stay visible
    /// until [`finish_sync`](Self::finish_sync), so a relist does not blank the
    /// UI while its pages arrive.
    pub fn begin_sync(&mut self) {
        self.sync_seen = Some(HashSet::new());
    }

    /// Remove objects that were absent from the list started by
    /// [`begin_sync`](Self::begin_sync). Returns whether anything was removed.
    pub fn finish_sync(&mut self) -> bool {
        let Some(seen) = self.sync_seen.take() else {
            return false;
        };
        let stale: Vec<ObjectKey> = self
            .objects
            .keys()
            .filter(|key| !seen.contains(*key))
            .cloned()
            .collect();
        let mut changed = false;
        for key in stale {
            changed |= self.remove(&key);
        }
        changed
    }

    /// Abandon an interrupted list without deleting objects it did not reach.
    pub fn abort_sync(&mut self) {
        self.sync_seen = None;
    }

    /// Insert or replace an object. Returns whether the store actually changed:
    /// a repeated `(key, uid, resourceVersion)` is a no-op, which makes poll
    /// relists and duplicate watch events cheap.
    pub fn upsert(&mut self, item: Item) -> bool {
        let key = ObjectKey::from_item(&item);
        if let Some(seen) = &mut self.sync_seen {
            seen.insert(key.clone());
        }

        if let Some(existing) = self.objects.get(&key)
            && !item.resource_version.is_empty()
            && existing.uid == item.uid
            && existing.resource_version == item.resource_version
        {
            return false;
        }

        let item = Arc::new(item);
        if let Some(existing) = self.objects.get(&key)
            && !existing.uid.is_empty()
            && existing.uid != item.uid
        {
            self.by_uid.remove(&existing.uid);
        }
        if !item.uid.is_empty()
            && let Some(previous) = self.by_uid.insert(item.uid.clone(), key.clone())
            && previous != key
        {
            self.objects.remove(&previous);
        }
        self.objects.insert(key, item);
        self.pending = self.pending.saturating_add(1);
        true
    }

    pub fn remove(&mut self, key: &ObjectKey) -> bool {
        let Some(item) = self.objects.remove(key) else {
            return false;
        };
        if !item.uid.is_empty() {
            self.by_uid.remove(&item.uid);
        }
        self.pending = self.pending.saturating_add(1);
        true
    }

    /// Remove the oldest objects until at most `max_items` remain. Used for
    /// high-churn kinds such as Events, where the API can retain thousands.
    /// Returns whether anything was removed.
    pub fn trim_oldest(&mut self, max_items: usize) -> bool {
        if self.objects.len() <= max_items {
            return false;
        }
        let mut by_age: Vec<(ObjectKey, i64)> = self
            .objects
            .iter()
            .map(|(key, item)| (key.clone(), item.created_at.unwrap_or(i64::MIN)))
            .collect();
        by_age.sort_by_key(|(_, created_at)| *created_at);
        let remove = self.objects.len() - max_items;
        let mut changed = false;
        for (key, _) in by_age.into_iter().take(remove) {
            changed |= self.remove(&key);
        }
        changed
    }

    pub fn snapshot(&mut self, phase: SnapshotPhase) -> ResourceSnapshot {
        self.pending = 0;
        ResourceSnapshot {
            items: self.objects.values().cloned().collect(),
            phase,
            mode: WatchMode::Watch,
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

    fn versioned_pod(name: &str, namespace: &str, uid: &str, version: &str) -> Item {
        let mut item = pod(name, namespace, uid);
        item.resource_version = version.into();
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

    #[test]
    fn upsert_skips_an_unchanged_resource_version() {
        let mut store = ResourceStore::new();
        store.upsert(versioned_pod("a", "ns", "uid-a", "10"));
        let _ = store.snapshot(SnapshotPhase::Ready);

        let mut same = versioned_pod("a", "ns", "uid-a", "10");
        same.status = "running".into();
        assert!(!store.upsert(same));
        assert_eq!(store.pending(), 0);
        assert_eq!(store.snapshot(SnapshotPhase::Ready).items()[0].status, "");
    }

    #[test]
    fn sync_keeps_unchanged_items_and_removes_absent_ones() {
        let mut store = ResourceStore::new();
        store.reset([
            versioned_pod("a", "ns", "uid-a", "1"),
            versioned_pod("b", "ns", "uid-b", "1"),
            versioned_pod("c", "ns", "uid-c", "1"),
        ]);
        let _ = store.snapshot(SnapshotPhase::Ready);

        store.begin_sync();
        assert!(!store.upsert(versioned_pod("a", "ns", "uid-a", "1")));
        assert!(store.upsert(versioned_pod("b", "ns", "uid-b", "2")));
        assert!(store.upsert(versioned_pod("d", "ns", "uid-d", "1")));
        store.finish_sync();

        let snapshot = store.snapshot(SnapshotPhase::Ready);
        let names: Vec<_> = snapshot.items().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "d"]);
    }

    #[test]
    fn an_unchanged_sync_does_not_bump_pending() {
        let mut store = ResourceStore::new();
        store.reset([versioned_pod("a", "ns", "uid-a", "1")]);
        let _ = store.snapshot(SnapshotPhase::Ready);

        store.begin_sync();
        assert!(!store.upsert(versioned_pod("a", "ns", "uid-a", "1")));
        store.finish_sync();

        assert_eq!(store.pending(), 0);
        assert_eq!(store.snapshot(SnapshotPhase::Ready).items().len(), 1);
    }

    #[test]
    fn aborting_a_sync_does_not_delete_unseen_items() {
        let mut store = ResourceStore::new();
        store.reset([
            versioned_pod("a", "ns", "uid-a", "1"),
            versioned_pod("b", "ns", "uid-b", "1"),
        ]);
        let _ = store.snapshot(SnapshotPhase::Ready);

        store.begin_sync();
        store.upsert(versioned_pod("a", "ns", "uid-a", "2"));
        store.abort_sync();

        let snapshot = store.snapshot(SnapshotPhase::Ready);
        assert_eq!(snapshot.items().len(), 2);
    }

    #[test]
    fn removing_an_unknown_key_is_a_no_op() {
        let mut store = ResourceStore::new();
        let _ = store.snapshot(SnapshotPhase::Ready);
        assert!(!store.remove(&ObjectKey::new("ns", "missing")));
        assert_eq!(store.pending(), 0);
    }

    #[test]
    fn trim_oldest_removes_the_oldest_objects_first() {
        let mut store = ResourceStore::new();
        for (index, created_at) in [(0, 10), (1, 30), (2, 20), (3, 40)] {
            let mut item = versioned_pod(
                &format!("event-{index}"),
                "ns",
                &format!("uid-{index}"),
                "1",
            );
            item.created_at = Some(created_at);
            store.upsert(item);
        }
        let _ = store.snapshot(SnapshotPhase::Ready);

        assert!(store.trim_oldest(2));
        let snapshot = store.snapshot(SnapshotPhase::Ready);
        let names: Vec<_> = snapshot.items().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["event-1", "event-3"]);

        assert!(!store.trim_oldest(2));
        assert_eq!(store.pending(), 0);
    }

    #[test]
    fn snapshots_carry_the_watch_mode() {
        let mut store = ResourceStore::new();
        let snapshot = store.snapshot(SnapshotPhase::Ready);
        assert_eq!(snapshot.mode(), WatchMode::Watch);
        let snapshot = snapshot.with_mode(WatchMode::Poll(PollRate::Every(5)));
        assert_eq!(snapshot.mode(), WatchMode::Poll(PollRate::Every(5)));
        assert_eq!(ResourceSnapshot::loading().mode(), WatchMode::Watch);
        assert_eq!(ResourceSnapshot::failed("x").mode(), WatchMode::Watch);
    }
}
