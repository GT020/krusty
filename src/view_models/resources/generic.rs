//! ViewModel shared by every resource kind without a dedicated view.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::component::table::TableState;
use gpui_kit::{App, AppContext as _, Context, Entity, Window};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::resource::ResourceModel;
use crate::models::{Item, NamespaceScope, ResourceFilter, ResourceKind, SnapshotPhase, WatchMode};
use crate::repository;
use crate::view_models::resources::table::ResourceTable;

pub struct ResourceListViewModel {
    kind: ResourceKind,
    active: Arc<AtomicBool>,
    scope: NamespaceScope,
    filter: ResourceFilter,
    epoch: u64,
    refresh_interval: Option<Duration>,
    model: Entity<ResourceModel>,
    table: Entity<TableState<ResourceTable>>,
    subscription: Option<repository::Subscription>,
    pump_cancel: Option<watch::Sender<()>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl ResourceListViewModel {
    pub fn new(
        kind: ResourceKind,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_interval(kind, scope, window, cx, None)
    }

    /// Build a list whose snapshots are coalesced to `interval`, used by data
    /// that changes faster than the UI should repaint.
    pub fn with_interval(
        kind: ResourceKind,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
        refresh_interval: Option<Duration>,
    ) -> Self {
        let epoch = cx.global::<ClusterState>().context_epoch();
        let active = Arc::new(AtomicBool::new(true));
        let model = cx.new(|_| ResourceModel::new());
        let table = ResourceTable::new_state(model.clone(), kind, window, cx);
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let epoch = cx.global::<ClusterState>().context_epoch();
            if epoch == this.epoch {
                return;
            }
            this.epoch = epoch;
            if !this.is_active() {
                this.cancel_pump();
                return;
            }
            this.subscribe(cx);
            cx.notify();
        });
        let mut this = Self {
            kind,
            active,
            scope,
            filter: ResourceFilter::default(),
            epoch,
            refresh_interval,
            model,
            table,
            subscription: None,
            pump_cancel: None,
            cluster_sub,
        };
        this.subscribe(cx);
        this
    }

    pub fn table(&self) -> &Entity<TableState<ResourceTable>> {
        &self.table
    }

    pub fn scope(&self) -> &NamespaceScope {
        &self.scope
    }

    /// Switch this tab's namespace filter and restart its watcher.
    pub fn set_scope(&mut self, scope: NamespaceScope, cx: &mut Context<Self>) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        self.subscribe(cx);
        cx.notify();
    }

    pub fn set_filter(&mut self, filter: ResourceFilter, cx: &mut Context<Self>) {
        if self.filter == filter {
            return;
        }
        self.filter = filter.clone();
        if let Some(subscription) = &self.subscription {
            subscription.set_filter(filter);
        }
        cx.notify();
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        self.model.read(cx).phase().clone()
    }

    pub fn mode(&self, cx: &App) -> WatchMode {
        self.model.read(cx).snapshot().mode()
    }

    pub fn item_count(&self, cx: &App) -> usize {
        self.model.read(cx).item_count()
    }

    pub fn item_at(&self, row: usize, cx: &App) -> Option<Item> {
        self.model
            .read(cx)
            .snapshot()
            .items()
            .get(row)
            .map(|item| (**item).clone())
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        let was_active = self.active.swap(active, Ordering::Relaxed);
        if was_active == active {
            return;
        }
        if let Some(subscription) = &self.subscription {
            subscription.set_paused(!active);
        }
        if active {
            if self.subscription.is_none() {
                self.subscribe(cx);
                return;
            }
            if let Some(snapshot) = self.subscription.as_ref().map(|s| s.snapshot()) {
                self.model.update(cx, |model, cx| {
                    model.apply_snapshot(snapshot);
                    cx.notify();
                });
            } else {
                cx.notify();
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.is_active() {
            return;
        }
        if let Some(sub) = &self.subscription {
            sub.refresh();
        } else {
            self.subscribe(cx);
        }
    }

    fn cancel_pump(&mut self) {
        self.subscription = None;
        self.pump_cancel = None;
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.cancel_pump();
        let subscription =
            match repository::subscribe(cx, self.kind, self.scope.clone(), self.filter.clone()) {
                Ok(subscription) => subscription,
                Err(error) => {
                    self.model.update(cx, |model, cx| {
                        model.apply_snapshot(Arc::new(crate::models::ResourceSnapshot::failed(
                            error,
                        )));
                        cx.notify();
                    });
                    return;
                }
            };
        subscription.set_paused(!self.is_active());
        let initial = subscription.snapshot();
        let notify_now = self.is_active();
        self.model.update(cx, |model, cx| {
            model.apply_snapshot(initial);
            if notify_now {
                cx.notify();
            }
        });
        let (cancel, mut cancel_rx) = watch::channel(());
        self.pump_cancel = Some(cancel);
        let model = self.model.downgrade();
        let mut pump = subscription.clone();
        let refresh_interval = self.refresh_interval;
        cx.spawn(async move |_, cx: &mut gpui_kit::AsyncApp| {
            let mut last_refresh: Option<Instant> = None;
            loop {
                let snapshot = tokio::select! {
                    _ = cancel_rx.changed() => break,
                    snapshot = pump.next() => snapshot,
                };
                let Some(snapshot) = snapshot else {
                    break;
                };
                if let Some(interval) = refresh_interval
                    && let Some(last) = last_refresh
                {
                    let elapsed = last.elapsed();
                    if elapsed < interval {
                        tokio::select! {
                            _ = cancel_rx.changed() => break,
                            _ = cx.background_executor().timer(interval - elapsed) => {}
                        }
                    }
                }
                last_refresh = Some(Instant::now());
                let applied = model.update(cx, |model, cx| {
                    model.apply_snapshot(snapshot);
                    cx.notify();
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
        self.subscription = Some(subscription);
    }
}
