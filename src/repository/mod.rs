//! Repository layer: owns Kubernetes list/watch I/O and publishes immutable
//! snapshots. UI threads never perform network I/O or mutate the store.

pub mod config_maps;
pub mod daemon_sets;
pub mod deployments;
pub mod ingresses;
pub mod jobs;
pub mod nodes;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pods;
pub mod secrets;
pub mod services;
mod watcher;

pub use watcher::namespace_names;

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use gpui_kit::{App, BorrowAppContext as _, Global};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, ResourceKind, ResourceSnapshot};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SubscriptionKey {
    epoch: u64,
    kind: ResourceKind,
    scope: NamespaceScope,
}

struct Entry {
    snapshots: watch::Sender<Arc<ResourceSnapshot>>,
    _cancel: watch::Sender<bool>,
    refresh: watch::Sender<u64>,
}

#[derive(Default)]
pub struct Repository {
    entries: HashMap<SubscriptionKey, Weak<Entry>>,
}

impl Global for Repository {}

pub fn init(cx: &mut App) {
    cx.set_global(Repository::default());
}

/// A live list/watch subscription. Dropping the last handle for a key cancels
/// the background watch task.
#[derive(Clone)]
pub struct Subscription {
    rx: watch::Receiver<Arc<ResourceSnapshot>>,
    _entry: Arc<Entry>,
}

impl Subscription {
    pub fn snapshot(&self) -> Arc<ResourceSnapshot> {
        self.rx.borrow().clone()
    }

    /// Wait for the next snapshot. `None` means the subscription ended.
    pub async fn next(&mut self) -> Option<Arc<ResourceSnapshot>> {
        self.rx.changed().await.ok()?;
        Some(self.rx.borrow_and_update().clone())
    }

    /// Ask the shared watch task to perform a fresh list.
    pub fn refresh(&self) {
        self._entry.refresh.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });
    }
}

/// Subscribe to a kind in a namespace scope. Subscriptions with the same key
/// share one list/watch task, so opening the same resource twice costs nothing.
#[allow(dead_code)]
pub fn subscribe(
    cx: &mut App,
    kind: ResourceKind,
    scope: NamespaceScope,
) -> Result<Subscription, String> {
    subscribe_kind(cx, kind, scope)
}

pub(crate) fn subscribe_kind(
    cx: &mut App,
    kind: ResourceKind,
    scope: NamespaceScope,
) -> Result<Subscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let scope = if kind.is_namespaced() {
        scope
    } else {
        NamespaceScope::All
    };
    let key = SubscriptionKey {
        epoch: state.context_epoch(),
        kind,
        scope: scope.clone(),
    };

    Ok(cx.update_global::<Repository, _>(|repository, _| {
        repository
            .entries
            .retain(|_, entry| entry.strong_count() > 0);

        if let Some(entry) = repository.entries.get(&key).and_then(Weak::upgrade) {
            return Subscription {
                rx: entry.snapshots.subscribe(),
                _entry: entry,
            };
        }

        let (snapshots, rx) = watch::channel(Arc::new(ResourceSnapshot::loading()));
        let (cancel, cancel_rx) = watch::channel(false);
        let (refresh, refresh_rx) = watch::channel(0u64);
        let entry = Arc::new(Entry {
            snapshots: snapshots.clone(),
            _cancel: cancel,
            refresh,
        });
        let watch_task = watcher::run(client, kind, scope, snapshots, cancel_rx, refresh_rx);
        tokio.spawn(watch_task);
        repository.entries.insert(key, Arc::downgrade(&entry));
        Subscription { rx, _entry: entry }
    }))
}
