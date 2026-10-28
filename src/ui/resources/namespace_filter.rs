//! Per-tab namespace filter: the selector state and its sync with the cluster.

use crate::cluster::ClusterState;
use crate::models::NamespaceScope;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, SharedString, Styled as _, Subscription,
    Window, px,
};

/// One row in the namespace selector. `name` is empty for the "All namespaces"
/// sentinel; `value()` distinguishes the scope.
#[derive(Clone, Debug)]
pub struct NamespaceItem {
    name: String,
}

impl NamespaceItem {
    fn all() -> Self {
        Self {
            name: String::new(),
        }
    }

    fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    fn is_all(&self) -> bool {
        self.name.is_empty()
    }
}

impl SelectItem for NamespaceItem {
    type Value = String;

    fn title(&self) -> SharedString {
        if self.is_all() {
            "All namespaces".into()
        } else {
            self.name.clone().into()
        }
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

/// Build the select items: an "All namespaces" sentinel first, then namespaces.
fn namespace_items(namespaces: &[String]) -> SearchableVec<NamespaceItem> {
    let mut items = Vec::with_capacity(namespaces.len() + 1);
    items.push(NamespaceItem::all());
    items.extend(namespaces.iter().cloned().map(NamespaceItem::named));
    SearchableVec::new(items)
}

pub type NamespaceSelect = SelectState<SearchableVec<NamespaceItem>>;

/// Emitted when the user picks a namespace, or when the selected namespace
/// disappears from the cluster.
#[derive(Clone, Debug)]
pub enum NamespaceFilterEvent {
    Changed(NamespaceScope),
}

/// Namespace scope owned by one tab. Each tab has its own filter and view
/// model, so two tabs of the same resource can watch different namespaces.
pub struct NamespaceFilter {
    scope: NamespaceScope,
    select: Entity<NamespaceSelect>,
    synced_namespaces: Vec<String>,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the select event
    select_sub: Subscription,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the cluster observer
    cluster_sub: Subscription,
}

impl EventEmitter<NamespaceFilterEvent> for NamespaceFilter {}

impl NamespaceFilter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let namespaces = cx.global::<ClusterState>().namespaces().to_vec();
        let select = cx.new(|cx| {
            let mut select =
                SelectState::new(namespace_items(&namespaces), None, window, cx).searchable(true);
            select.set_selected_value(&String::new(), window, cx);
            select
        });
        let select_sub = cx.subscribe(
            &select,
            |this, _, event: &SelectEvent<SearchableVec<NamespaceItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.set_scope(scope_from_value(value.as_deref()), cx);
            },
        );
        let cluster_sub = cx.observe_global_in::<ClusterState>(window, |this, window, cx| {
            this.sync(window, cx);
        });
        Self {
            scope: NamespaceScope::All,
            select,
            synced_namespaces: namespaces,
            select_sub,
            cluster_sub,
        }
    }

    fn set_scope(&mut self, scope: NamespaceScope, cx: &mut Context<Self>) {
        if self.scope == scope {
            return;
        }
        self.scope = scope.clone();
        cx.emit(NamespaceFilterEvent::Changed(scope));
        cx.notify();
    }

    /// Refresh the namespace list and selection without discarding the user's
    /// in-progress search filter unless the list actually changed.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let namespaces = cx.global::<ClusterState>().namespaces().to_vec();
        // An empty list means the watcher has not loaded (or is switching
        // clusters) yet; keep the last list instead of blanking the selector.
        let namespaces_changed = !namespaces.is_empty() && namespaces != self.synced_namespaces;
        let scope_changed = match &self.scope {
            NamespaceScope::One(namespace)
                if !namespaces.is_empty() && !namespaces.contains(namespace) =>
            {
                self.set_scope(NamespaceScope::All, cx);
                true
            }
            _ => false,
        };

        if namespaces_changed {
            self.synced_namespaces = namespaces.clone();
            self.select.update(cx, |select, cx| {
                select.set_items(namespace_items(&namespaces), window, cx);
            });
        }

        let value = self
            .scope
            .namespace()
            .map(str::to_string)
            .unwrap_or_default();
        let value_changed = self.select.read(cx).selected_value() != Some(&value);
        if value_changed {
            self.select.update(cx, |select, cx| {
                select.set_selected_value(&value, window, cx);
            });
        }

        if namespaces_changed || scope_changed || value_changed {
            cx.notify();
        }
    }
}

fn scope_from_value(value: Option<&str>) -> NamespaceScope {
    match value {
        Some(namespace) if !namespace.is_empty() => NamespaceScope::One(namespace.to_string()),
        _ => NamespaceScope::All,
    }
}

/// Render the tab-local namespace selector.
pub fn render(filter: &Entity<NamespaceFilter>, cx: &App) -> impl gpui_kit::IntoElement + use<> {
    let filter = filter.read(cx);
    Select::new(&filter.select)
        .id(SharedString::from(format!(
            "namespace-filter-{}",
            filter.select.entity_id()
        )))
        .small()
        .w(px(180.))
        .menu_max_h(px(320.))
        .placeholder("All namespaces")
        .search_placeholder("Search namespaces")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_all_sentinel_clears_the_namespace_filter() {
        assert_eq!(scope_from_value(None), NamespaceScope::All);
        assert_eq!(scope_from_value(Some("")), NamespaceScope::All);
    }

    #[test]
    fn a_named_selection_filters_to_that_namespace() {
        assert_eq!(
            scope_from_value(Some("kube-system")),
            NamespaceScope::One("kube-system".to_string())
        );
    }
}
