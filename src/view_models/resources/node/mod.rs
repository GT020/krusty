//! ViewModel for Nodes: subscription pump, table columns, and detail selection.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::component::table::{Column, TableDelegate, TableEvent, TableState};
use gpui_kit::px;
use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _, Window,
    div,
};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::node::NodesModel;
use crate::models::resources::node::detail::{NodeDetailState, UsageHistory};
use crate::models::{Item, NamespaceScope, ResourceKind, SnapshotPhase};
use crate::repository;

pub struct NodesViewModel {
    active: Arc<AtomicBool>,
    scope: NamespaceScope,
    epoch: u64,
    model: Entity<NodesModel>,
    table: Entity<TableState<NodesViewModelTable>>,
    subscription: Option<repository::Subscription>,
    pump_cancel: Option<watch::Sender<()>>,
    detail_cancel: Option<watch::Sender<bool>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the table events
    table_sub: gpui_kit::Subscription,
}

impl NodesViewModel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.global::<ClusterState>();
        let scope = state.scope().clone();
        let epoch = state.context_epoch();
        let active = Arc::new(AtomicBool::new(true));
        let model = cx.new(|_| NodesModel::new());
        let table_model = model.clone();
        let table = cx.new(|cx| {
            TableState::new(NodesViewModelTable { model: table_model }, _window, cx)
                .row_selectable(true)
        });
        let table_sub = cx.subscribe(&table, |this, _, event: &TableEvent, cx| {
            if let TableEvent::SelectRow(row) = event {
                this.open_row(*row, cx);
            }
        });
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let (scope, epoch) = {
                let state = cx.global::<ClusterState>();
                (state.scope().clone(), state.context_epoch())
            };
            if scope == this.scope && epoch == this.epoch {
                return;
            }
            this.scope = scope;
            this.epoch = epoch;
            this.subscribe(cx);
        });
        let mut this = Self {
            active,
            scope,
            epoch,
            model,
            table,
            subscription: None,
            pump_cancel: None,
            detail_cancel: None,
            cluster_sub,
            table_sub,
        };
        this.subscribe(cx);
        this
    }

    pub fn table(&self) -> &Entity<TableState<NodesViewModelTable>> {
        &self.table
    }

    pub fn scope(&self) -> &NamespaceScope {
        &self.scope
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> ResourceKind {
        ResourceKind::Nodes
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        self.model.read(cx).phase().clone()
    }

    pub fn item_count(&self, cx: &App) -> usize {
        self.model.read(cx).item_count()
    }

    pub fn detail(&self, cx: &App) -> NodeDetailState {
        self.model.read(cx).detail().clone()
    }

    pub fn usage(&self, cx: &App) -> UsageHistory {
        self.model.read(cx).usage().clone()
    }

    pub fn usage_error(&self, cx: &App) -> Option<String> {
        self.model.read(cx).usage_error().map(str::to_string)
    }

    pub fn item_at(&self, row: usize, cx: &App) -> Option<crate::models::Item> {
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
        if active {
            cx.notify();
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
        if let Some(node) = self.current_detail_node(cx) {
            self.start_detail(node, cx);
        }
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.cancel_detail();
        self.model.update(cx, |model, cx| {
            model.close_detail();
            cx.notify();
        });
    }

    fn current_detail_node(&self, cx: &App) -> Option<String> {
        self.model.read(cx).detail().node().map(str::to_string)
    }

    fn cancel_detail(&mut self) {
        if let Some(cancel) = self.detail_cancel.take() {
            let _ = cancel.send(true);
        }
    }

    fn open_row(&mut self, row: usize, cx: &mut Context<Self>) {
        let name = {
            let model = self.model.read(cx);
            model
                .snapshot()
                .items()
                .get(row)
                .map(|item| item.name.clone())
        };
        let Some(name) = name else { return };
        if self.current_detail_node(cx).as_deref() == Some(name.as_str()) {
            return;
        }
        self.start_detail(name, cx);
    }

    fn start_detail(&mut self, node: String, cx: &mut Context<Self>) {
        self.cancel_detail();
        self.model.update(cx, |model, cx| {
            model.begin_detail(node.clone());
            cx.notify();
        });
        let mut detail_sub = match repository::resources::nodes::subscribe_detail(cx, &node) {
            Ok(subscription) => subscription,
            Err(error) => {
                self.model.update(cx, |model, cx| {
                    model.apply_update(
                        crate::models::resources::node::detail::NodeDetailUpdate::Detail {
                            node,
                            result: Err(format!("Could not open node details: {error}")),
                        },
                    );
                    cx.notify();
                });
                return;
            }
        };
        self.detail_cancel = Some(detail_sub.cancel_handle());
        let model = self.model.downgrade();
        cx.spawn(async move |_, cx| {
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

    fn cancel_pump(&mut self) {
        self.subscription = None;
        self.pump_cancel = None;
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.cancel_pump();
        self.cancel_detail();
        self.model.update(cx, |model, _| model.close_detail());
        let subscription = match repository::resources::nodes::subscribe(cx) {
            Ok(s) => s,
            Err(error) => {
                use crate::models::ResourceSnapshot;
                self.model.update(cx, |model, cx| {
                    model.apply_snapshot(Arc::new(ResourceSnapshot::failed(error)));
                    cx.notify();
                });
                return;
            }
        };
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
        let active = Arc::clone(&self.active);
        let mut pump = subscription.clone();
        cx.spawn(async move |_, cx: &mut gpui_kit::AsyncApp| {
            loop {
                let snapshot = tokio::select! {
                    _ = cancel_rx.changed() => break,
                    snapshot = pump.next() => snapshot,
                };
                let Some(snapshot) = snapshot else {
                    break;
                };
                let should_notify = active.load(Ordering::Relaxed);
                let applied = model.update(cx, |model, cx| {
                    model.apply_snapshot(snapshot);
                    if should_notify {
                        cx.notify();
                    }
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

// ─── Table delegate ───────────────────────────────────────────────────────────

pub struct NodesViewModelTable {
    model: Entity<NodesModel>,
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("name", "Name").width(px(220.)),
        Column::new("status", "Status").width(px(110.)),
        Column::new("version", "Version").width(px(130.)),
        Column::new("capacity", "CPU").width(px(90.)).text_right(),
        Column::new("age", "Age").width(px(90.)).text_right(),
    ]
}

fn column_key(col_ix: usize) -> gpui_kit::SharedString {
    columns()
        .get(col_ix)
        .map(|c| c.key.clone())
        .unwrap_or_default()
}

fn cell(item: &Item, key: &str) -> String {
    match key {
        "name" => item.name.clone(),
        "namespace" => item.namespace.clone(),
        "ready" => item.ready.clone(),
        "restarts" => item.restarts.to_string(),
        "capacity" => item.capacity.clone(),
        "version" => item.version.clone(),
        "status" => item.status.clone(),
        "age" => item.age(),
        _ => String::new(),
    }
}

impl TableDelegate for NodesViewModelTable {
    fn columns_count(&self, _cx: &App) -> usize {
        columns().len()
    }

    fn rows_count(&self, cx: &App) -> usize {
        self.model.read(cx).item_count()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        columns()
            .get(col_ix)
            .cloned()
            .unwrap_or_else(|| Column::new("empty", ""))
    }

    fn render_td(
        &mut self,
        row: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        let model = self.model.read(cx);
        let Some(item) = model.snapshot().items().get(row) else {
            return div().into_any_element();
        };
        let key = column_key(col_ix);
        let text = cell(item, &key);
        if col_ix == 0 {
            return div().child(text).into_any_element();
        }
        let color = if key == "status" && !item.is_ok() {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        };
        div().text_color(color).child(text).into_any_element()
    }

    fn cell_text(&self, row: usize, col_ix: usize, cx: &App) -> String {
        let model = self.model.read(cx);
        let Some(item) = model.snapshot().items().get(row) else {
            return String::new();
        };
        cell(item, &column_key(col_ix))
    }
}
