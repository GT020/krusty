//! Shared, per-kind store primitives.
//!
//! This module is the Freelens-style replacement for the current
//! per-`(kind, scope)` watch subscriptions. A [`KubeObjectRecord`] keeps the
//! full typed Kubernetes object alongside the small [`Item`] row summary.
//! [`StoreData`] owns the authoritative object map and UID index, while
//! [`StoreView`] provides a namespace-filtered view for one screen.

mod data;
mod engine;
mod mapping;
mod plan;
mod record;
mod view;

pub use data::{StoreData, StoreSnapshot};
pub(crate) use engine::KubeObjectStoreHandle;
pub use plan::{ClusterCapabilities, StoreWatchPlan, watch_plan};
pub use record::{KubeObjectRecord, StoredObject};
pub use view::StoreView;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use gpui_kit::{App, BorrowAppContext as _};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, ObjectKey, ResourceKind};
use crate::repository::watch_prefs::{WatchPreference, WatchPrefs};
use crate::repository::{Repository, StoreEntry, Subscription};

/// Kinds that must start polling immediately when watch is denied. Namespaces
/// are required by every namespaced view, and Events/Leases are high-churn
/// enough that a failed watch should not delay their first useful poll.
const IMMEDIATE_POLL_ON_DENIAL: &[ResourceKind] = &[
    ResourceKind::Namespaces,
    ResourceKind::Events,
    ResourceKind::Leases,
];

fn requires_immediate_poll(kind: ResourceKind) -> bool {
    IMMEDIATE_POLL_ON_DENIAL.contains(&kind)
}

fn denied_registry() -> &'static Mutex<HashSet<(u64, ResourceKind)>> {
    static REGISTRY: OnceLock<Mutex<HashSet<(u64, ResourceKind)>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn watch_denied(epoch: u64, kind: ResourceKind) -> bool {
    denied_registry()
        .lock()
        .is_ok_and(|set| set.contains(&(epoch, kind)))
}

pub(crate) fn mark_watch_denied(epoch: u64, kind: ResourceKind) {
    if let Ok(mut set) = denied_registry().lock() {
        set.retain(|(stale_epoch, _)| *stale_epoch == epoch);
        set.insert((epoch, kind));
    }
}

pub(crate) fn clear_watch_denied(epoch: u64, kind: ResourceKind) {
    if let Ok(mut set) = denied_registry().lock() {
        set.remove(&(epoch, kind));
    }
}

/// Subscribe a view to the shared store for `kind`.
pub(crate) fn subscribe(
    cx: &mut App,
    kind: ResourceKind,
    scope: NamespaceScope,
) -> Result<Subscription, String> {
    let (store, tokio) = ensure_store(cx, kind)?;
    let scope = if kind.is_namespaced() {
        scope
    } else {
        NamespaceScope::All
    };
    Ok(store.subscribe(scope, &tokio))
}

/// Read a cached full object without creating a store or a network request.
pub(crate) fn cached_object(
    cx: &App,
    kind: ResourceKind,
    key: &ObjectKey,
) -> Option<Arc<KubeObjectRecord>> {
    let repository = cx.global::<Repository>();
    let epoch = cx.global::<ClusterState>().context_epoch();
    let entry = repository.stores.get(&kind)?;
    if entry.epoch != epoch {
        return None;
    }
    entry.store.object(key)
}

/// Observe an already-running store, if one exists. Detail panes use this for
/// live updates when a list view is already watching; they never start a store
/// or a per-object watch themselves.
pub(crate) fn existing_store_snapshot(
    cx: &App,
    kind: ResourceKind,
) -> Option<watch::Receiver<Arc<StoreSnapshot>>> {
    let repository = cx.global::<Repository>();
    let epoch = cx.global::<ClusterState>().context_epoch();
    let entry = repository.stores.get(&kind)?;
    if entry.epoch != epoch {
        return None;
    }
    Some(entry.store.snapshot_receiver())
}

fn ensure_store(
    cx: &mut App,
    kind: ResourceKind,
) -> Result<(Arc<KubeObjectStoreHandle>, tokio::runtime::Handle), String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let epoch = state.context_epoch();
    let global_watch = state.global_watch();
    let global_list = state.global_list();

    let store = cx.update_global::<Repository, _>(|repository, _| {
        if let Some(entry) = repository.stores.get(&kind)
            && entry.epoch == epoch
        {
            return Ok(entry.store.clone());
        }

        let mut prefs: WatchPrefs = repository.watch_prefs.prefs_for(kind);
        if prefs.mode == WatchPreference::Watch
            && (watch_denied(epoch, kind) || (!global_watch && requires_immediate_poll(kind)))
        {
            // The first attempt already established that watch is unavailable,
            // or this kind is required before namespaced views can start.
            prefs.mode = WatchPreference::Poll;
        }
        let Some(store) = engine::spawn(
            kind,
            client,
            tokio.clone(),
            epoch,
            global_watch,
            global_list,
            prefs,
        ) else {
            return Err(format!("{} is not available in this build.", kind.label()));
        };
        repository.stores.insert(
            kind,
            StoreEntry {
                epoch,
                store: store.clone(),
            },
        );
        tracing::debug!(?kind, epoch, "store: created");
        Ok(store)
    })?;

    Ok((store, tokio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_are_in_the_immediate_poll_set() {
        assert!(requires_immediate_poll(ResourceKind::Namespaces));
        assert!(requires_immediate_poll(ResourceKind::Events));
        assert!(requires_immediate_poll(ResourceKind::Leases));
        assert!(!requires_immediate_poll(ResourceKind::Pods));
    }

    #[test]
    fn watch_denial_is_remembered_and_can_be_cleared() {
        let epoch = u64::MAX;
        clear_watch_denied(epoch, ResourceKind::Namespaces);
        assert!(!watch_denied(epoch, ResourceKind::Namespaces));

        mark_watch_denied(epoch, ResourceKind::Namespaces);
        assert!(watch_denied(epoch, ResourceKind::Namespaces));
        assert!(!watch_denied(epoch, ResourceKind::Pods));

        clear_watch_denied(epoch, ResourceKind::Namespaces);
        assert!(!watch_denied(epoch, ResourceKind::Namespaces));
    }
}
