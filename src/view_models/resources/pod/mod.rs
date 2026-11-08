//! ViewModel for Pods: the shared list feed plus the open pod detail.

pub mod exec;
pub mod logs;

use gpui_kit::component::table::{TableEvent, TableState};
use gpui_kit::{App, AppContext as _, Context, Entity, Window};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::pod::PodsModel;
use crate::models::resources::pod::detail::{PodDetailState, PodDetailUpdate, UsageHistory};
use crate::models::{Item, NamespaceScope, ResourceFilter, ResourceKind, SnapshotPhase, WatchMode};
use crate::repository;
use crate::view_models::resources::generic::ResourceListViewModel;
use crate::view_models::resources::table::ResourceTable;

pub struct PodsViewModel {
    list: Entity<ResourceListViewModel>,
    table: Entity<TableState<ResourceTable>>,
    detail_model: Entity<PodsModel>,
    detail_cancel: Option<watch::Sender<bool>>,
    epoch: u64,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the table events
    table_sub: gpui_kit::Subscription,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl PodsViewModel {
    pub fn new(scope: NamespaceScope, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let epoch = cx.global::<ClusterState>().context_epoch();
        let list = cx.new(|cx| ResourceListViewModel::new(ResourceKind::Pods, scope, window, cx));
        let table = list.read(cx).table().clone();
        let table_sub = cx.subscribe(&table, |this, _, event: &TableEvent, cx| {
            if let TableEvent::SelectRow(row) = event {
                this.open_row(*row, cx);
            }
        });
        let detail_model = cx.new(|_| PodsModel::new());
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let epoch = cx.global::<ClusterState>().context_epoch();
            if epoch == this.epoch {
                return;
            }
            this.epoch = epoch;
            this.cancel_detail();
            this.detail_model.update(cx, |model, cx| {
                model.close_detail();
                cx.notify();
            });
        });
        Self {
            list,
            table,
            detail_model,
            detail_cancel: None,
            epoch,
            table_sub,
            cluster_sub,
        }
    }

    pub fn table(&self) -> &Entity<TableState<ResourceTable>> {
        &self.table
    }

    pub fn scope(&self, cx: &App) -> NamespaceScope {
        self.list.read(cx).scope().clone()
    }

    /// Switch this tab's namespace filter and restart its watcher.
    pub fn set_scope(&mut self, scope: NamespaceScope, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| list.set_scope(scope, cx));
        self.close_detail(cx);
    }

    pub fn set_filter(&mut self, filter: ResourceFilter, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| list.set_filter(filter, cx));
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        self.list.read(cx).phase(cx)
    }

    pub fn mode(&self, cx: &App) -> WatchMode {
        self.list.read(cx).mode(cx)
    }

    pub fn item_count(&self, cx: &App) -> usize {
        self.list.read(cx).item_count(cx)
    }

    pub fn detail(&self, cx: &App) -> PodDetailState {
        self.detail_model.read(cx).detail().clone()
    }

    pub fn usage(&self, cx: &App) -> UsageHistory {
        self.detail_model.read(cx).usage().clone()
    }

    pub fn usage_error(&self, cx: &App) -> Option<String> {
        self.detail_model.read(cx).usage_error().map(str::to_string)
    }

    pub fn item_at(&self, row: usize, cx: &App) -> Option<Item> {
        self.list.read(cx).item_at(row, cx)
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| list.set_active(active, cx));
        if active {
            if let Some((namespace, name)) = self.current_detail_identity(cx) {
                self.attach_detail(namespace, name, cx);
            }
        } else {
            self.cancel_detail();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| list.refresh(cx));
        if let Some((namespace, name)) = self.current_detail_identity(cx) {
            self.start_detail(namespace, name, cx);
        }
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.cancel_detail();
        self.detail_model.update(cx, |model, cx| {
            model.close_detail();
            cx.notify();
        });
    }

    /// Open the detail pane for a pod by name, used by kubectl commands.
    pub fn open_detail(&mut self, namespace: String, name: String, cx: &mut Context<Self>) {
        if self.current_detail_identity(cx) == Some((namespace.clone(), name.clone())) {
            return;
        }
        self.start_detail(namespace, name, cx);
    }

    fn current_detail_identity(&self, cx: &App) -> Option<(String, String)> {
        self.detail_model
            .read(cx)
            .detail()
            .identity()
            .map(|(namespace, name)| (namespace.to_string(), name.to_string()))
    }

    fn cancel_detail(&mut self) {
        if let Some(cancel) = self.detail_cancel.take() {
            let _ = cancel.send(true);
        }
    }

    fn open_row(&mut self, row: usize, cx: &mut Context<Self>) {
        let Some(item) = self.item_at(row, cx) else {
            return;
        };
        if self.current_detail_identity(cx) == Some((item.namespace.clone(), item.name.clone())) {
            return;
        }
        self.start_detail(item.namespace, item.name, cx);
    }

    fn start_detail(&mut self, namespace: String, name: String, cx: &mut Context<Self>) {
        self.cancel_detail();
        self.detail_model.update(cx, |model, cx| {
            model.begin_detail(namespace.clone(), name.clone());
            cx.notify();
        });
        self.attach_detail(namespace, name, cx);
    }

    /// Restart the detail stream without resetting the selected detail state,
    /// used when a hidden tab becomes visible again.
    fn attach_detail(&mut self, namespace: String, name: String, cx: &mut Context<Self>) {
        self.cancel_detail();
        let mut detail_sub =
            match repository::resources::pods::subscribe_detail(cx, &namespace, &name) {
                Ok(subscription) => subscription,
                Err(error) => {
                    self.detail_model.update(cx, |model, cx| {
                        model.apply_update(PodDetailUpdate::Detail {
                            namespace,
                            name,
                            result: Err(format!("Could not open pod details: {error}")),
                        });
                        cx.notify();
                    });
                    return;
                }
            };
        self.detail_cancel = Some(detail_sub.cancel_handle());
        let model = self.detail_model.downgrade();
        cx.spawn(async move |_, cx: &mut gpui_kit::AsyncApp| {
            while let Some(update) = detail_sub.next().await {
                let applied = model.update(cx, |model, cx| {
                    if model.apply_update(update) {
                        cx.notify();
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}
