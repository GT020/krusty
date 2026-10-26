//! ViewModel for Nodes: subscription pump + column definitions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::component::table::{Column, TableDelegate, TableState};
use gpui_kit::px;
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _, Window, div};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::{Item, NamespaceScope, ResourceKind, SnapshotPhase};
use crate::models::nodes::NodesModel;
use crate::repository;

pub struct NodesViewModel {
    active: Arc<AtomicBool>,
    scope: NamespaceScope,
    epoch: u64,
    model: Entity<NodesModel>,
    table: Entity<TableState<NodesViewModelTable>>,
    subscription: Option<repository::Subscription>,
    pump_cancel: Option<watch::Sender<()>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl NodesViewModel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.global::<ClusterState>();
        let scope = state.scope().clone();
        let epoch = state.context_epoch();
        let active = Arc::new(AtomicBool::new(true));
        let model = cx.new(|_| NodesModel::new());
        let table_model = model.clone();
        let table = cx.new(|cx| TableState::new(NodesViewModelTable { model: table_model }, _window, cx));
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
            cluster_sub,
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
    }

    fn cancel_pump(&mut self) {
        self.subscription = None;
        self.pump_cancel = None;
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.cancel_pump();
        let subscription = match repository::nodes::subscribe(cx) {
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
                let Some(snapshot) = snapshot else { break; };
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
        Column::new("name",     "Name").width(px(220.)),
        Column::new("status",   "Status").width(px(110.)),
        Column::new("version",  "Version").width(px(130.)),
        Column::new("capacity", "CPU").width(px(90.)).text_right(),
        Column::new("age",      "Age").width(px(90.)).text_right(),
    ]
}

fn column_key(col_ix: usize) -> gpui_kit::SharedString {
    columns().get(col_ix).map(|c| c.key.clone()).unwrap_or_default()
}

fn cell(item: &Item, key: &str) -> String {
    match key {
        "name"      => item.name.clone(),
        "namespace" => item.namespace.clone(),
        "ready"     => item.ready.clone(),
        "restarts"  => item.restarts.to_string(),
        "capacity"  => item.capacity.clone(),
        "version"   => item.version.clone(),
        "status"    => item.status.clone(),
        "age"       => item.age(),
        _           => String::new(),
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
        columns().get(col_ix).cloned().unwrap_or_else(|| Column::new("empty", ""))
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
