
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

use std::sync::Arc;

use gpui_kit::{App, BorrowAppContext as _};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, ObjectKey, ResourceKind};
use crate::repository::{Repository, StoreEntry, Subscription};

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

    let store = cx.update_global::<Repository, _>(|repository, _| {
        if let Some(entry) = repository.stores.get(&kind)
            && entry.epoch == epoch
        {
            return Ok(entry.store.clone());
        }

        let Some(store) = engine::spawn(kind, client, tokio.clone(), epoch, global_watch) else {
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
