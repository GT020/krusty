//! ViewModel layer: GPUI entities owning presentation state, bound to models.

pub mod resources;
pub mod shell;

pub use resources::{
    ClusterOverviewViewModel, NodesViewModel, PodsViewModel, ResourceListViewModel,
};

use crate::models::{Item, NamespaceScope, ResourceFilter, ResourceKind, SnapshotPhase, WatchMode};
use gpui_kit::component::table::{DataTable, TableEvent};
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, Subscription, Window};

/// One open resource list, with a dedicated view for kinds that add
/// detail-specific panels and a shared view for everything else.
pub enum ResourceViewModel {
    Pods(Entity<PodsViewModel>),
    Nodes(Entity<NodesViewModel>),
    Generic(Entity<ResourceListViewModel>),
}

impl ResourceViewModel {
    pub fn new<V: 'static>(
        kind: ResourceKind,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Self {
        match kind {
            ResourceKind::Pods => {
                Self::Pods(cx.new(|cx| PodsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Nodes => Self::Nodes(cx.new(|cx| NodesViewModel::new(scope, window, cx))),
            kind => Self::Generic(cx.new(|cx| ResourceListViewModel::new(kind, scope, window, cx))),
        }
    }

    pub fn observe<V: 'static>(
        &self,
        cx: &mut Context<V>,
        on_notify: impl Fn(&mut V, &mut Context<V>) + 'static,
    ) -> Subscription {
        match self {
            Self::Pods(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Nodes(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Generic(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
        }
    }

    pub fn item_at(&self, row: usize, cx: &App) -> Option<Item> {
        match self {
            Self::Pods(vm) => vm.read(cx).item_at(row, cx),
            Self::Nodes(vm) => vm.read(cx).item_at(row, cx),
            Self::Generic(vm) => vm.read(cx).item_at(row, cx),
        }
    }

    pub fn observe_row_selected<V: 'static>(
        &self,
        cx: &mut Context<V>,
        mut on_select: impl FnMut(&mut V, usize, &mut Context<V>) + 'static,
    ) -> Subscription {
        match self {
            Self::Pods(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Nodes(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Generic(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
        }
    }

    pub fn set_active<V: 'static>(&mut self, active: bool, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Generic(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
        }
    }

    pub fn set_scope<V: 'static>(&self, scope: NamespaceScope, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Generic(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
        }
    }

    pub fn set_filter<V: 'static>(&self, filter: ResourceFilter, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_filter(filter.clone(), cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_filter(filter.clone(), cx)),
            Self::Generic(vm) => vm.update(cx, |vm, cx| vm.set_filter(filter.clone(), cx)),
        }
    }

    pub fn refresh<V: 'static>(&self, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Generic(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
        }
    }

    pub fn scope(&self, cx: &App) -> NamespaceScope {
        match self {
            Self::Pods(vm) => vm.read(cx).scope(cx),
            Self::Nodes(vm) => vm.read(cx).scope(cx),
            Self::Generic(vm) => vm.read(cx).scope().clone(),
        }
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        match self {
            Self::Pods(vm) => vm.read(cx).phase(cx),
            Self::Nodes(vm) => vm.read(cx).phase(cx),
            Self::Generic(vm) => vm.read(cx).phase(cx),
        }
    }

    pub fn mode(&self, cx: &App) -> WatchMode {
        match self {
            Self::Pods(vm) => vm.read(cx).mode(cx),
            Self::Nodes(vm) => vm.read(cx).mode(cx),
            Self::Generic(vm) => vm.read(cx).mode(cx),
        }
    }

    pub fn item_count(&self, cx: &App) -> usize {
        match self {
            Self::Pods(vm) => vm.read(cx).item_count(cx),
            Self::Nodes(vm) => vm.read(cx).item_count(cx),
            Self::Generic(vm) => vm.read(cx).item_count(cx),
        }
    }

    pub fn render_table(&self, cx: &App) -> gpui_kit::AnyElement {
        match self {
            Self::Pods(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Nodes(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Generic(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
        }
    }
}
