//! Repository layer: owns Kubernetes list/watch I/O and publishes immutable
//! snapshots. UI threads never perform network I/O or mutate the store.

pub mod custom_resources;
pub mod resources;
pub mod retry;
pub mod store;
pub mod terminal;

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{App, Global};
use tokio::sync::watch;

use crate::models::{NamespaceScope, ResourceFilter, ResourceKind, ResourceSnapshot};

pub(crate) struct StoreEntry {
    pub epoch: u64,
    /// Held strongly for the cluster lifetime. The watch itself still stops
    /// when the last subscriber leaves; only the cached data is retained.
    pub store: Arc<store::KubeObjectStoreHandle>,
}

pub struct Repository {
    pub(crate) stores: HashMap<ResourceKind, StoreEntry>,
    pub(crate) terminal: terminal::TerminalSessionStore,
}

impl Global for Repository {}

pub fn init(cx: &mut App) {
    cx.set_global(Repository {
        stores: HashMap::new(),
        terminal: terminal::TerminalSessionStore::default(),
    });
}

/// A live store subscription. Dropping the last handle releases the view and
/// stops the shared watch when no other view needs it.
///
/// The source is intentionally generic: views consume the same snapshot API
/// regardless of which store feeds them.
#[derive(Clone)]
pub struct Subscription {
    rx: watch::Receiver<Arc<ResourceSnapshot>>,
    paused: watch::Sender<bool>,
    paused_rx: watch::Receiver<bool>,
    filter: watch::Sender<ResourceFilter>,
    refresh: Arc<dyn Fn() + Send + Sync>,
    _keep_alive: Arc<dyn Any + Send + Sync>,
}

impl Subscription {
    pub(crate) fn new(
        rx: watch::Receiver<Arc<ResourceSnapshot>>,
        paused: watch::Sender<bool>,
        paused_rx: watch::Receiver<bool>,
        filter: watch::Sender<ResourceFilter>,
        refresh: Arc<dyn Fn() + Send + Sync>,
        keep_alive: Arc<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            rx,
            paused,
            paused_rx,
            filter,
            refresh,
            _keep_alive: keep_alive,
        }
    }

    pub fn snapshot(&self) -> Arc<ResourceSnapshot> {
        self.rx.borrow().clone()
    }

    /// Stop delivering snapshots to this subscriber. The shared watch task
    /// keeps running; [`next`](Self::next) returns the latest snapshot as soon
    /// as the subscription is resumed.
    pub fn set_paused(&self, paused: bool) {
        self.paused.send_replace(paused);
    }

    /// Replace the row filter applied to this subscription's snapshots.
    pub fn set_filter(&self, filter: ResourceFilter) {
        self.filter.send_replace(filter);
    }

    /// Wait for the next snapshot relevant to this subscriber. `None` means the
    /// subscription ended. While paused, snapshots are coalesced and only the
    /// latest one is returned on resume.
    pub async fn next(&mut self) -> Option<Arc<ResourceSnapshot>> {
        let mut paused = *self.paused_rx.borrow_and_update();
        loop {
            if paused {
                tokio::select! {
                    result = self.paused_rx.changed() => {
                        result.ok()?;
                        if !*self.paused_rx.borrow_and_update() {
                            return Some(self.rx.borrow_and_update().clone());
                        }
                    }
                    result = self.rx.changed() => {
                        result.ok()?;
                        self.rx.borrow_and_update();
                    }
                }
            } else {
                tokio::select! {
                    result = self.paused_rx.changed() => {
                        result.ok()?;
                        paused = *self.paused_rx.borrow_and_update();
                    }
                    result = self.rx.changed() => {
                        result.ok()?;
                        if *self.paused_rx.borrow() {
                            self.rx.borrow_and_update();
                            paused = true;
                        } else {
                            return Some(self.rx.borrow_and_update().clone());
                        }
                    }
                }
            }
        }
    }

    /// Ask the shared watch task to perform a fresh list.
    pub fn refresh(&self) {
        (self.refresh)();
    }
}

/// Subscribe to a kind in a namespace scope. Subscriptions with the same key
/// share one list/watch task, so opening the same resource twice costs nothing.
pub fn subscribe(
    cx: &mut App,
    kind: ResourceKind,
    scope: NamespaceScope,
    filter: ResourceFilter,
) -> Result<Subscription, String> {
    subscribe_kind(cx, kind, scope, filter)
}

pub(crate) fn subscribe_kind(
    cx: &mut App,
    kind: ResourceKind,
    scope: NamespaceScope,
    filter: ResourceFilter,
) -> Result<Subscription, String> {
    store::subscribe(cx, kind, scope, filter)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::models::SnapshotPhase;

    #[tokio::test]
    async fn a_paused_subscription_coalesces_until_resumed() {
        let (snapshots, rx) = watch::channel(Arc::new(ResourceSnapshot::loading()));
        let (paused, paused_rx) = watch::channel(false);
        let (filter, _) = watch::channel(ResourceFilter::default());
        let refresh: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut subscription =
            Subscription::new(rx, paused, paused_rx, filter, refresh, Arc::new(()));

        subscription.set_paused(true);
        snapshots.send_replace(Arc::new(ResourceSnapshot::failed("offline")));
        let resume = subscription.clone();
        let mut next = Box::pin(subscription.next());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut next)
                .await
                .is_err(),
            "a paused subscription must not deliver snapshots"
        );

        resume.set_paused(false);
        let snapshot = tokio::time::timeout(Duration::from_millis(250), next)
            .await
            .expect("resume must wake the pump")
            .expect("resume snapshot");
        assert!(matches!(snapshot.phase(), SnapshotPhase::Error(_)));
    }
}
