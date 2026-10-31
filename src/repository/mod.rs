//! Repository layer: owns Kubernetes list/watch I/O and publishes immutable
//! snapshots. UI threads never perform network I/O or mutate the store.

pub mod custom_resources;
pub mod resources;
pub mod retry;
pub mod watch_prefs;

pub use watch_prefs::{WatchPreference, WatchPrefs};

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use gpui_kit::{App, BorrowAppContext as _, Global};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, PollRate, ResourceKind, ResourceSnapshot, WatchMode};

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
    prefs: watch::Sender<WatchPrefs>,
}

pub struct Repository {
    entries: HashMap<SubscriptionKey, Weak<Entry>>,
    watch_prefs: watch_prefs::WatchPrefsStore,
}

impl Global for Repository {}

pub fn init(cx: &mut App) {
    cx.set_global(Repository {
        entries: HashMap::new(),
        watch_prefs: watch_prefs::WatchPrefsStore::load(),
    });
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

        let mut initial_prefs = repository.watch_prefs.prefs_for(kind);
        if initial_prefs.mode == WatchPreference::Watch
            && resources::watcher::watch_denied(key.epoch, kind)
        {
            // Watch is known to be denied for this kind: go straight to
            // polling instead of re-trying the doomed watch on every
            // namespace filter change. The saved preference stays `Watch`.
            initial_prefs.mode = WatchPreference::Poll;
        }
        let initial_mode = match initial_prefs.mode {
            WatchPreference::Watch => WatchMode::Watch,
            WatchPreference::Poll => WatchMode::Poll(initial_prefs.rate),
        };

        let (snapshots, rx) = watch::channel(Arc::new(
            ResourceSnapshot::loading().with_mode(initial_mode),
        ));
        let (cancel, cancel_rx) = watch::channel(false);
        let (refresh, refresh_rx) = watch::channel(0u64);
        let (prefs, prefs_rx) = watch::channel(initial_prefs);
        let entry = Arc::new(Entry {
            snapshots: snapshots.clone(),
            _cancel: cancel,
            refresh,
            prefs,
        });
        let watch_task = resources::watcher::run(
            client, kind, scope, key.epoch, snapshots, cancel_rx, refresh_rx, prefs_rx,
        );
        tokio.spawn(watch_task);
        repository.entries.insert(key, Arc::downgrade(&entry));
        Subscription { rx, _entry: entry }
    }))
}

/// Set whether a resource kind is watched live or polled. The saved poll rate
/// is kept. Applies to every running watch task for the kind and is saved.
pub fn set_watch_preference(cx: &mut App, kind: ResourceKind, mode: WatchPreference) {
    if mode == WatchPreference::Watch {
        // Explicit request: forget any RBAC denial so the watch is retried.
        let epoch = cx.global::<ClusterState>().context_epoch();
        resources::watcher::clear_watch_denied(epoch, kind);
    }
    update_watch_prefs(cx, kind, |prefs| prefs.mode = mode);
}

/// Set a kind's poll rate and switch it to poll mode. Applies to every
/// running watch task for the kind and is saved.
pub fn set_poll_rate(cx: &mut App, kind: ResourceKind, rate: PollRate) {
    update_watch_prefs(cx, kind, |prefs| {
        prefs.mode = WatchPreference::Poll;
        prefs.rate = rate;
    });
}

fn update_watch_prefs(cx: &mut App, kind: ResourceKind, change: impl FnOnce(&mut WatchPrefs)) {
    let epoch = cx.global::<ClusterState>().context_epoch();
    cx.update_global::<Repository, _>(|repository, _| {
        repository.watch_prefs.update(kind, change);
        if let Err(error) = repository.watch_prefs.save() {
            tracing::warn!(%error, "repository: failed to save watch prefs");
        }
        let prefs = repository.watch_prefs.prefs_for(kind);
        for (key, entry) in &repository.entries {
            if key.kind == kind
                && key.epoch == epoch
                && let Some(entry) = entry.upgrade()
            {
                entry.prefs.send_replace(prefs);
            }
        }
    });
}
