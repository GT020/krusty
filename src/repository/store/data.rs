//! Incrementally maintained full-object store for one resource kind.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use crate::models::{ObjectKey, SnapshotPhase, WatchMode};

use super::record::KubeObjectRecord;

/// Immutable store state published after a flush.
pub struct StoreSnapshot {
    revision: u64,
    phase: SnapshotPhase,
    mode: WatchMode,
    objects: Arc<[Arc<KubeObjectRecord>]>,
    /// Last revision at which each namespace changed. `All` views use the
    /// global revision; `One(ns)` views can skip unrelated namespaces.
    namespace_revisions: Arc<BTreeMap<String, u64>>,
}

impl Clone for StoreSnapshot {
    fn clone(&self) -> Self {
        Self {
            revision: self.revision,
            phase: self.phase.clone(),
            mode: self.mode,
            objects: self.objects.clone(),
            namespace_revisions: self.namespace_revisions.clone(),
        }
    }
}

impl std::fmt::Debug for StoreSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoreSnapshot")
            .field("revision", &self.revision)
            .field("phase", &self.phase)
            .field("mode", &self.mode)
            .field("objects", &self.objects.len())
            .finish_non_exhaustive()
    }
}

impl StoreSnapshot {
    pub fn phase(&self) -> &SnapshotPhase {
        &self.phase
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn mode(&self) -> WatchMode {
        self.mode
    }

    pub fn objects(&self) -> &[Arc<KubeObjectRecord>] {
        &self.objects
    }

    pub fn object(&self, key: &ObjectKey) -> Option<&Arc<KubeObjectRecord>> {
        let index = self
            .objects
            .binary_search_by(|record| record.key.cmp(key))
            .ok()?;
        self.objects.get(index)
    }

    pub fn namespace_revision(&self, namespace: &str) -> Option<u64> {
        self.namespace_revisions.get(namespace).copied()
    }

    #[cfg(test)]
    pub fn summary_items(&self) -> Arc<[Arc<crate::models::Item>]> {
        self.objects
            .iter()
            .map(|record| record.summary.clone())
            .collect()
    }
}

/// Authoritative object collection for one kind. Owned by the store task and
/// mutated only from that task, so no shared lock is needed on the hot path.
#[derive(Debug, Default)]
pub struct StoreData {
    objects: BTreeMap<ObjectKey, Arc<KubeObjectRecord>>,
    by_uid: HashMap<String, ObjectKey>,
    pending: usize,
    revision: u64,
    namespace_revisions: BTreeMap<String, u64>,
    changed_namespaces: HashSet<String>,
}

impl StoreData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pending(&self) -> usize {
        self.pending
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    #[cfg(test)]
    pub fn get_by_uid(&self, uid: &str) -> Option<&Arc<KubeObjectRecord>> {
        self.by_uid.get(uid).and_then(|key| self.objects.get(key))
    }

    /// Insert or replace an object. A repeated `(key, uid, resourceVersion)` is
    /// a no-op, which makes poll relists and duplicate watch events cheap.
    pub fn upsert(&mut self, record: KubeObjectRecord) -> bool {
        let key = record.key.clone();
        if let Some(existing) = self.objects.get(&key)
            && !record.resource_version.is_empty()
            && existing.uid == record.uid
            && existing.resource_version == record.resource_version
        {
            return false;
        }

        let record = Arc::new(record);
        if let Some(existing) = self.objects.get(&key)
            && !existing.uid.is_empty()
            && existing.uid != record.uid
        {
            self.by_uid.remove(&existing.uid);
        }

        if !record.uid.is_empty()
            && let Some(previous) = self.by_uid.insert(record.uid.clone(), key.clone())
            && previous != key
        {
            self.objects.remove(&previous);
        }

        self.objects.insert(key.clone(), record);
        self.note_changed(&key);
        true
    }

    pub fn remove(&mut self, key: &ObjectKey) -> bool {
        let Some(record) = self.objects.remove(key) else {
            return false;
        };

        if !record.uid.is_empty() {
            self.by_uid.remove(&record.uid);
        }

        self.note_changed(key);
        true
    }

    pub fn replace_namespace<I>(&mut self, namespace: &str, records: I) -> bool
    where
        I: IntoIterator<Item = KubeObjectRecord>,
    {
        let mut seen = HashSet::new();
        let mut changed = false;

        for record in records {
            seen.insert(record.key.clone());
            changed |= self.upsert(record);
        }

        let stale: Vec<ObjectKey> = self
            .objects
            .range(namespace_range(namespace))
            .filter(|(key, _)| !seen.contains(*key))
            .map(|(key, _)| key.clone())
            .collect();

        for key in stale {
            changed |= self.remove(&key);
        }

        if changed {
            self.changed_namespaces.insert(namespace.to_string());
        }
        changed
    }

    /// Replace the whole store with the result of an all-namespace list.
    pub fn replace_all<I>(&mut self, records: I) -> bool
    where
        I: IntoIterator<Item = KubeObjectRecord>,
    {
        let mut seen = HashSet::new();
        let mut changed = false;

        for record in records {
            seen.insert(record.key.clone());
            changed |= self.upsert(record);
        }

        let stale: Vec<ObjectKey> = self
            .objects
            .keys()
            .filter(|key| !seen.contains(*key))
            .cloned()
            .collect();

        for key in stale {
            changed |= self.remove(&key);
        }

        changed
    }

    /// Remove the oldest objects until at most `max_items` remain.
    pub fn trim_oldest(&mut self, max_items: usize) -> bool {
        if self.objects.len() <= max_items {
            return false;
        }

        let mut by_age: Vec<(ObjectKey, i64)> = self
            .objects
            .iter()
            .map(|(key, record)| (key.clone(), record.summary.created_at.unwrap_or(i64::MIN)))
            .collect();
        by_age.sort_by_key(|(_, created_at)| *created_at);

        let remove = self.objects.len() - max_items;
        let mut changed = false;
        for (key, _) in by_age.into_iter().take(remove) {
            changed |= self.remove(&key);
        }
        changed
    }

    /// Produce an immutable snapshot and advance the revision if anything
    /// changed since the previous snapshot.
    pub fn snapshot(&mut self, phase: SnapshotPhase, mode: WatchMode) -> StoreSnapshot {
        let changed = self.pending > 0;
        if changed {
            self.revision = self.revision.wrapping_add(1);
            for namespace in self.changed_namespaces.drain() {
                self.namespace_revisions.insert(namespace, self.revision);
            }
        }

        self.pending = 0;
        StoreSnapshot {
            revision: self.revision,
            phase,
            mode,
            objects: self.objects.values().cloned().collect(),
            namespace_revisions: Arc::new(self.namespace_revisions.clone()),
        }
    }

    fn note_changed(&mut self, key: &ObjectKey) {
        self.pending = self.pending.saturating_add(1);
        self.changed_namespaces.insert(key.namespace.clone());
    }
}

fn namespace_range(namespace: &str) -> impl std::ops::RangeBounds<ObjectKey> {
    use std::ops::Bound::{Excluded, Included};

    (
        Included(ObjectKey::new(namespace, "")),
        Excluded(ObjectKey::new(namespace, "\u{10FFFF}")),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use kube::ResourceExt;

    use super::*;
    use crate::models::Item;
    use crate::models::{NamespaceScope, ResourceFilter, WatchMode};
    use crate::repository::store::StoreView;

    fn record(name: &str, namespace: &str, uid: &str, version: &str) -> KubeObjectRecord {
        let object: kube::api::DynamicObject = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "uid": uid,
                "resourceVersion": version,
                "labels": { "app": "demo" },
                "ownerReferences": [
                    { "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "demo", "uid": "owner-1" }
                ]
            }
        }))
        .expect("dynamic object");

        let mut item = Item::new(name.to_string(), Some(namespace.to_string()));
        item.uid = uid.to_string();
        item.resource_version = version.to_string();
        item.status = "running".to_string();
        KubeObjectRecord::new(object, item)
    }

    #[test]
    fn stores_full_objects_and_summaries() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));

        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(snapshot.objects().len(), 1);
        assert_eq!(snapshot.summary_items().len(), 1);
        assert_eq!(
            snapshot
                .object(&ObjectKey::new("ns-a", "pod-a"))
                .unwrap()
                .uid,
            "uid-a"
        );
        assert_eq!(
            snapshot
                .object(&ObjectKey::new("ns-a", "pod-a"))
                .unwrap()
                .typed::<kube::api::DynamicObject>()
                .unwrap()
                .name_any(),
            "pod-a"
        );
        let object = snapshot
            .object(&ObjectKey::new("ns-a", "pod-a"))
            .unwrap()
            .typed::<kube::api::DynamicObject>()
            .unwrap();
        assert_eq!(
            object
                .metadata
                .labels
                .as_ref()
                .and_then(|labels| labels.get("app"))
                .map(String::as_str),
            Some("demo")
        );
    }

    #[test]
    fn unchanged_resource_version_is_a_no_op() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "10"));
        let _ = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let mut same = record("pod-a", "ns-a", "uid-a", "10");
        same.summary = Arc::new(Item::new("pod-a".into(), Some("ns-a".into())));
        assert!(!data.upsert(same));
        assert_eq!(data.pending(), 0);
    }

    #[test]
    fn replacing_a_namespace_keeps_other_namespaces() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));
        data.upsert(record("pod-b", "ns-b", "uid-b", "1"));
        data.upsert(record("pod-c", "ns-a", "uid-c", "1"));

        assert!(data.replace_namespace(
            "ns-a",
            [
                record("pod-a", "ns-a", "uid-a", "2"),
                record("pod-d", "ns-a", "uid-d", "1")
            ]
        ));

        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let names: Vec<_> = snapshot
            .objects()
            .iter()
            .map(|record| (record.key.namespace.as_str(), record.key.name.as_str()))
            .collect();
        assert_eq!(
            names,
            [("ns-a", "pod-a"), ("ns-a", "pod-d"), ("ns-b", "pod-b")]
        );
        assert!(
            snapshot.object(&ObjectKey::new("ns-a", "pod-c")).is_none(),
            "objects absent from the namespace list are removed"
        );
    }

    #[test]
    fn replacing_all_marks_each_namespace_that_changed() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));

        assert!(data.replace_all([
            record("pod-b", "ns-a", "uid-b", "1"),
            record("pod-c", "ns-b", "uid-c", "1"),
        ]));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        assert_eq!(snapshot.namespace_revision("ns-a"), Some(1));
        assert_eq!(snapshot.namespace_revision("ns-b"), Some(1));

        let mut ns_a = crate::repository::store::StoreView::new(
            NamespaceScope::One("ns-a".into()),
            ResourceFilter::default(),
        );
        assert!(ns_a.apply(&snapshot).is_some());
        assert_eq!(ns_a.len(), 1);
    }

    #[test]
    fn replacing_all_removes_everything_absent() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));
        data.upsert(record("pod-b", "ns-b", "uid-b", "1"));

        assert!(data.replace_all([record("pod-c", "ns-c", "uid-c", "1")]));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(snapshot.objects().len(), 1);
        assert_eq!(snapshot.objects()[0].key, ObjectKey::new("ns-c", "pod-c"));
    }

    #[test]
    fn uid_index_follows_replaced_objects() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-old", "1"));
        assert!(data.get_by_uid("uid-old").is_some());

        data.upsert(record("pod-a", "ns-a", "uid-new", "2"));
        assert!(data.get_by_uid("uid-old").is_none());
        assert!(data.get_by_uid("uid-new").is_some());
    }

    #[test]
    fn trims_the_oldest_objects_first() {
        let mut data = StoreData::new();
        for (name, age) in [("a", 10), ("b", 30), ("c", 20)] {
            let mut record = record(name, "ns", &format!("uid-{name}"), "1");
            record.summary = Arc::new({
                let mut item = Item::new(name.to_string(), Some("ns".to_string()));
                item.created_at = Some(age);
                item
            });
            data.upsert(record);
        }

        assert!(data.trim_oldest(2));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let names: Vec<_> = snapshot
            .objects()
            .iter()
            .map(|record| record.key.name.as_str())
            .collect();
        assert_eq!(names, ["b", "c"]);
    }

    #[test]
    fn revision_advances_only_when_a_flush_changed_something() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));
        let first = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(first.revision(), 1);

        let second = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(second.revision(), 1);
        assert_eq!(second.objects().len(), 1);

        data.upsert(record("pod-b", "ns-a", "uid-b", "1"));
        let third = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(third.revision(), 2);
    }

    #[test]
    fn snapshot_carries_namespace_revisions() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));
        data.upsert(record("pod-b", "ns-b", "uid-b", "1"));

        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(snapshot.namespace_revision("ns-a"), Some(1));
        assert_eq!(snapshot.namespace_revision("ns-b"), Some(1));

        data.upsert(record("pod-c", "ns-a", "uid-c", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(snapshot.namespace_revision("ns-a"), Some(2));
        assert_eq!(snapshot.namespace_revision("ns-b"), Some(1));
    }

    #[test]
    fn snapshot_exposes_scope_and_summary() {
        let mut data = StoreData::new();
        data.upsert(record("pod-a", "ns-a", "uid-a", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Error("offline".into()), WatchMode::Watch);
        assert_eq!(snapshot.phase(), &SnapshotPhase::Error("offline".into()));
        assert_eq!(snapshot.mode(), WatchMode::Watch);
        assert_eq!(snapshot.summary_items().len(), 1);
    }

    #[test]
    fn namespace_range_contains_only_the_requested_namespace() {
        let mut data = StoreData::new();
        data.upsert(record("a", "ns-1", "uid-1", "1"));
        data.upsert(record("b", "ns-2", "uid-2", "1"));
        data.upsert(record("c", "ns-10", "uid-3", "1"));

        assert!(data.replace_namespace("ns-1", [record("a", "ns-1", "uid-1", "2")]));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert_eq!(snapshot.objects().len(), 3);
        assert_eq!(
            snapshot
                .object(&ObjectKey::new("ns-1", "a"))
                .expect("ns-1 object")
                .resource_version,
            "2"
        );
    }

    #[test]
    fn store_view_filters_by_scope() {
        let mut data = StoreData::new();
        data.upsert(record("a", "ns-1", "uid-1", "1"));
        data.upsert(record("b", "ns-2", "uid-2", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let mut all = StoreView::new(NamespaceScope::All, ResourceFilter::default());
        let mut one = StoreView::new(
            NamespaceScope::One("ns-2".into()),
            ResourceFilter::default(),
        );
        let all = all.apply(&snapshot).unwrap();
        let one = one.apply(&snapshot).unwrap();
        assert_eq!(all.items().len(), 2);
        assert_eq!(one.items().len(), 1);
        assert_eq!(one.items()[0].name, "b");
    }
}
