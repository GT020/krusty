//! Namespace-filtered view over a shared store.
//!
//! A `StoreView` is not a cache and owns no network. It keeps the sorted `Item`
//! rows for one screen scope and rebuilds them only when the store flush is
//! relevant to that scope.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::models::{Item, NamespaceScope, ResourceSnapshot, SnapshotPhase, WatchMode};

use super::data::StoreSnapshot;
use super::record::KubeObjectRecord;

pub struct StoreView {
    scope: NamespaceScope,
    items: Arc<[Arc<Item>]>,
    phase: SnapshotPhase,
    mode: WatchMode,
    last_revision: u64,
    last_namespace_revisions: BTreeMap<String, Option<u64>>,
    initialized: bool,
}

impl StoreView {
    pub fn new(scope: NamespaceScope) -> Self {
        Self {
            scope,
            items: Arc::from(Vec::<Arc<Item>>::new()),
            phase: SnapshotPhase::Loading,
            mode: WatchMode::Watch,
            last_revision: 0,
            last_namespace_revisions: BTreeMap::new(),
            initialized: false,
        }
    }

    fn namespace_revision_changed(&self, snapshot: &StoreSnapshot, namespace: &str) -> bool {
        snapshot.namespace_revision(namespace)
            != self
                .last_namespace_revisions
                .get(namespace)
                .copied()
                .flatten()
    }

    #[cfg(test)]
    pub fn items(&self) -> &[Arc<Item>] {
        &self.items
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[cfg(test)]
    pub fn phase(&self) -> &SnapshotPhase {
        &self.phase
    }

    pub fn snapshot(&self) -> Arc<ResourceSnapshot> {
        Arc::new(ResourceSnapshot::from_items(
            self.items.clone(),
            self.phase.clone(),
            self.mode,
        ))
    }

    /// Apply a store snapshot. Returns the new view snapshot when the scope saw
    /// a relevant change; `None` means the view is unchanged.
    pub fn apply(&mut self, snapshot: &StoreSnapshot) -> Option<Arc<ResourceSnapshot>> {
        let phase_changed = self.phase != *snapshot.phase();
        let mode_changed = self.mode != snapshot.mode();

        let namespace_changed = match &self.scope {
            NamespaceScope::All => self.last_revision != snapshot.revision(),
            NamespaceScope::One(namespace) => self.namespace_revision_changed(snapshot, namespace),
            NamespaceScope::Many(namespaces) => namespaces
                .iter()
                .any(|namespace| self.namespace_revision_changed(snapshot, namespace)),
        };

        if self.initialized && !phase_changed && !mode_changed && !namespace_changed {
            return None;
        }

        let objects = snapshot.objects();
        self.items = match &self.scope {
            NamespaceScope::All => objects
                .iter()
                .map(|record| record.summary.clone())
                .collect(),
            NamespaceScope::One(namespace) => namespace_objects(objects, namespace)
                .iter()
                .map(|record| record.summary.clone())
                .collect(),
            NamespaceScope::Many(namespaces) => namespaces
                .iter()
                .flat_map(|namespace| namespace_objects(objects, namespace))
                .map(|record| record.summary.clone())
                .collect(),
        };
        self.phase = snapshot.phase().clone();
        self.mode = snapshot.mode();
        self.last_revision = snapshot.revision();
        self.last_namespace_revisions = self
            .scope
            .namespaces()
            .map(|namespace| {
                (
                    namespace.to_string(),
                    snapshot.namespace_revision(namespace),
                )
            })
            .collect();
        self.initialized = true;

        Some(self.snapshot())
    }
}

fn namespace_objects<'a>(
    objects: &'a [Arc<KubeObjectRecord>],
    namespace: &str,
) -> &'a [Arc<KubeObjectRecord>] {
    let start = objects.partition_point(|record| record.key.namespace.as_str() < namespace);
    let end = objects.partition_point(|record| record.key.namespace.as_str() <= namespace);
    &objects[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NamespaceScope, ResourceSnapshot, SnapshotPhase, WatchMode};

    fn view_for(scope: NamespaceScope, snapshot: &StoreSnapshot) -> StoreView {
        let mut view = StoreView::new(scope);
        view.apply(snapshot).expect("initial apply always emits");
        view
    }

    #[test]
    fn starts_loading_until_a_snapshot_arrives() {
        let view = StoreView::new(NamespaceScope::All);
        assert!(matches!(view.phase(), SnapshotPhase::Loading));
        assert_eq!(view.len(), 0);
        assert_eq!(view.snapshot().mode(), WatchMode::Watch);
    }

    #[test]
    fn all_scope_includes_every_namespace() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        data.upsert(test_record("b", "ns-2", "uid-b", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let view = view_for(NamespaceScope::All, &snapshot);
        assert_eq!(view.len(), 2);
        assert_eq!(view.snapshot().items().len(), 2);
    }

    #[test]
    fn namespace_scope_includes_only_that_namespace() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        data.upsert(test_record("b", "ns-2", "uid-b", "1"));
        data.upsert(test_record("c", "ns-10", "uid-c", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let view = view_for(NamespaceScope::One("ns-2".into()), &snapshot);
        assert_eq!(view.len(), 1);
        assert_eq!(view.items()[0].name, "b");
    }

    #[test]
    fn multi_namespace_scope_includes_only_the_selected_namespaces() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        data.upsert(test_record("b", "ns-2", "uid-b", "1"));
        data.upsert(test_record("c", "ns-3", "uid-c", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let view = view_for(
            NamespaceScope::Many(vec!["ns-1".into(), "ns-3".into()]),
            &snapshot,
        );
        assert_eq!(view.len(), 2);
        assert_eq!(view.items()[0].name, "a");
        assert_eq!(view.items()[1].name, "c");
    }

    #[test]
    fn multi_namespace_scope_ignores_unrelated_namespace_changes() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        data.upsert(test_record("b", "ns-2", "uid-b", "1"));
        let first = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let mut view = view_for(
            NamespaceScope::Many(vec!["ns-1".into(), "ns-2".into()]),
            &first,
        );

        data.upsert(test_record("c", "ns-3", "uid-c", "1"));
        let second = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert!(view.apply(&second).is_none());

        data.upsert(test_record("d", "ns-2", "uid-d", "1"));
        let third = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert!(view.apply(&third).is_some());
        assert_eq!(view.len(), 3);
    }

    #[test]
    fn unchanged_global_revision_does_not_emit() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        let snapshot = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);

        let mut view = view_for(NamespaceScope::All, &snapshot);
        assert!(view.apply(&snapshot).is_none());
    }

    #[test]
    fn unrelated_namespace_change_does_not_emit_for_a_namespace_view() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        let first = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let mut view = view_for(NamespaceScope::One("ns-1".into()), &first);

        data.upsert(test_record("b", "ns-2", "uid-b", "1"));
        let second = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert!(
            view.apply(&second).is_none(),
            "ns-2 change must not repaint ns-1"
        );

        data.upsert(test_record("c", "ns-1", "uid-c", "1"));
        let third = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        assert!(view.apply(&third).is_some());
        assert_eq!(view.len(), 2);
    }

    #[test]
    fn phase_changes_always_emit() {
        let mut data = super::super::data::StoreData::new();
        let mut view = StoreView::new(NamespaceScope::All);
        let ready = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let first = view.apply(&ready).expect("initial emit");
        assert_eq!(first.mode(), WatchMode::Watch);

        let mut errored_data = super::super::data::StoreData::new();
        errored_data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        let errored =
            errored_data.snapshot(SnapshotPhase::Error("offline".into()), WatchMode::Watch);
        let emitted = view.apply(&errored).expect("phase change emits");
        assert!(matches!(emitted.phase(), SnapshotPhase::Error(message) if message == "offline"));
    }

    #[test]
    fn snapshot_is_a_resource_snapshot_for_existing_models() {
        let mut data = super::super::data::StoreData::new();
        data.upsert(test_record("a", "ns-1", "uid-a", "1"));
        let store = data.snapshot(SnapshotPhase::Ready, WatchMode::Watch);
        let view = view_for(NamespaceScope::All, &store);
        let snapshot: Arc<ResourceSnapshot> = view.snapshot();
        assert_eq!(snapshot.items().len(), 1);
        assert_eq!(snapshot.phase(), &SnapshotPhase::Ready);
    }

    fn test_record(
        name: &str,
        namespace: &str,
        uid: &str,
        version: &str,
    ) -> super::super::record::KubeObjectRecord {
        let object: kube::api::DynamicObject = serde_json::from_value(serde_json::json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": name,
                "namespace": namespace,
                "uid": uid,
                "resourceVersion": version
            }
        }))
        .expect("dynamic object");

        let mut item = Item::new(name.to_string(), Some(namespace.to_string()));
        item.uid = uid.to_string();
        item.resource_version = version.to_string();
        super::super::record::KubeObjectRecord::new(object, item)
    }
}
