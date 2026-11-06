//! Per-tab namespace filter: the selector state and its sync with the cluster.

use crate::cluster::ClusterState;
use crate::models::NamespaceScope;
use gpui_kit::base::h_flex;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::combobox::{Combobox, ComboboxEvent, ComboboxState};
use gpui_kit::component::select::{SearchableVec, SelectDelegate, SelectItem};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, IndexPath};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, WeakEntity, Window, div, px,
};

/// One row in the namespace selector. `name` is empty for the "All namespaces"
/// row, which has no checkbox.
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

fn namespace_items(namespaces: &[String]) -> SearchableVec<NamespaceItem> {
    let mut items = Vec::with_capacity(namespaces.len() + 1);
    items.push(NamespaceItem::all());
    items.extend(namespaces.iter().cloned().map(NamespaceItem::named));
    SearchableVec::new(items)
}

/// Combobox delegate that draws a checkbox before every namespace name. The
/// checkbox toggles the namespace in place and keeps the menu open; clicking
/// the name itself selects just that namespace and closes the menu.
#[derive(Clone)]
pub(crate) struct NamespaceItems {
    items: SearchableVec<NamespaceItem>,
    filter: WeakEntity<NamespaceFilter>,
}

impl NamespaceItems {
    fn new(items: SearchableVec<NamespaceItem>, filter: WeakEntity<NamespaceFilter>) -> Self {
        Self { items, filter }
    }
}

impl SelectDelegate for NamespaceItems {
    type Item = NamespaceItem;

    fn items_count(&self, section: usize) -> usize {
        self.items.items_count(section)
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.items.item(ix)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.items.position(value)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, cx: &mut App) -> Task<()> {
        self.items.perform_search(query, window, cx)
    }

    fn is_item_checked(
        &self,
        _ix: IndexPath,
        item: &Self::Item,
        current_selection: &[(IndexPath, Self::Item)],
        _cx: &App,
    ) -> bool {
        if item.is_all() {
            current_selection.is_empty()
                || current_selection
                    .iter()
                    .all(|(_, selected)| selected.is_all())
        } else {
            current_selection
                .iter()
                .any(|(_, selected)| selected.value() == item.value())
        }
    }

    fn render_item(
        &self,
        _ix: IndexPath,
        item: &Self::Item,
        checked: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        if item.is_all() {
            return Some(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(14.)).flex_shrink_0())
                    .child(div().flex_1().min_w_0().truncate().child(item.title()))
                    .when(checked, |this| {
                        this.child(Icon::new(IconName::Check).text_color(cx.theme().primary))
                    })
                    .into_any_element(),
            );
        }

        let name = item.name.clone();
        let filter = self.filter.clone();
        Some(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id(SharedString::from(format!("namespace-check-{name}")))
                        .flex()
                        .size(px(14.))
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .border_1()
                        .cursor_pointer()
                        .when(checked, |this| {
                            this.border_color(cx.theme().primary)
                                .bg(cx.theme().primary)
                                .child(
                                    Icon::new(IconName::Check)
                                        .size(px(10.))
                                        .text_color(cx.theme().primary_foreground),
                                )
                        })
                        .when(!checked, |this| this.border_color(cx.theme().input))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            let Some(filter) = filter.upgrade() else {
                                return;
                            };
                            filter.update(cx, |filter, cx| {
                                filter.toggle_namespace(name.clone(), window, cx);
                            });
                        }),
                )
                .child(div().flex_1().min_w_0().truncate().child(item.title()))
                .into_any_element(),
        )
    }
}

pub type NamespaceSelect = ComboboxState<NamespaceItems>;

/// Emitted when the user picks namespaces, or when a selected namespace
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
    /// Create a filter that starts on `scope`, used when restoring a saved tab.
    pub fn with_scope(scope: NamespaceScope, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let namespaces = cx.global::<ClusterState>().namespaces().to_vec();
        let selected_values = scope.selected_namespaces();
        let filter = cx.entity().downgrade();
        let items = NamespaceItems::new(namespace_items(&namespaces), filter);
        let select = cx.new(|cx| {
            let mut select = ComboboxState::new(items, Vec::new(), window, cx).searchable(true);
            select.set_selected_values(&selected_values, window, cx);
            select
        });
        let select_sub = cx.subscribe(
            &select,
            |this, _, event: &ComboboxEvent<NamespaceItems>, cx| match event {
                // The clear button emits Change without a following Confirm, so
                // handle an empty selection immediately.
                ComboboxEvent::Change(values) if values.is_empty() => {
                    this.apply_scope(NamespaceScope::All, cx);
                }
                ComboboxEvent::Confirm(values) => {
                    this.apply_scope(NamespaceScope::from_namespaces(values.clone()), cx);
                }
                _ => {}
            },
        );
        let cluster_sub = cx.observe_global_in::<ClusterState>(window, |this, window, cx| {
            this.sync(window, cx);
        });
        Self {
            scope,
            select,
            synced_namespaces: namespaces,
            select_sub,
            cluster_sub,
        }
    }

    /// Switch the filter from outside the selector, e.g. when a kubectl
    /// command targets a namespace.
    pub fn set_scope(
        &mut self,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scope == scope {
            return;
        }
        let values = scope.selected_namespaces();
        self.apply_scope(scope, cx);
        self.select.update(cx, |select, cx| {
            select.set_selected_values(&values, window, cx);
        });
    }

    fn toggle_namespace(&mut self, namespace: String, window: &mut Window, cx: &mut Context<Self>) {
        let mut namespaces = self.scope.selected_namespaces();
        if let Some(index) = namespaces
            .iter()
            .position(|selected| selected == &namespace)
        {
            namespaces.remove(index);
        } else {
            namespaces.push(namespace);
        }

        let scope = NamespaceScope::from_namespaces(namespaces);
        let values = scope.selected_namespaces();
        if self.scope != scope {
            self.apply_scope(scope, cx);
        }
        self.select.update(cx, |select, cx| {
            select.set_selected_values(&values, window, cx);
        });
    }

    fn apply_scope(&mut self, scope: NamespaceScope, cx: &mut Context<Self>) {
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
                self.apply_scope(NamespaceScope::All, cx);
                true
            }
            NamespaceScope::Many(selected) if !namespaces.is_empty() => {
                let retained: Vec<String> = selected
                    .iter()
                    .filter(|namespace| namespaces.contains(namespace))
                    .cloned()
                    .collect();
                if retained.len() == selected.len() {
                    false
                } else {
                    self.apply_scope(NamespaceScope::from_namespaces(retained), cx);
                    true
                }
            }
            _ => false,
        };

        if namespaces_changed {
            self.synced_namespaces = namespaces.clone();
            let items = NamespaceItems::new(namespace_items(&namespaces), cx.entity().downgrade());
            self.select.update(cx, |select, cx| {
                select.set_items(items, window, cx);
            });
        }

        let values = self.scope.selected_namespaces();
        let value_changed = self.select.read(cx).selected_values() != values;
        if namespaces_changed || value_changed {
            self.select.update(cx, |select, cx| {
                select.set_selected_values(&values, window, cx);
            });
        }

        if namespaces_changed || scope_changed || value_changed {
            cx.notify();
        }
    }
}

/// Render the tab-local namespace selector.
pub fn render(filter: &Entity<NamespaceFilter>, cx: &App) -> impl gpui_kit::IntoElement + use<> {
    let filter = filter.read(cx);
    Combobox::new(&filter.select)
        .small()
        .w(px(240.))
        .menu_max_h(px(320.))
        .placeholder("All namespaces")
        .search_placeholder("Search namespaces")
        .render_trigger(|trigger, _window, _cx| {
            let names: Vec<String> = trigger
                .selection()
                .iter()
                .filter(|(_, item)| !item.is_all())
                .map(|(_, item)| item.title().to_string())
                .collect();
            let label = if names.is_empty() {
                "All namespaces".to_string()
            } else {
                names.join(", ")
            };
            div().truncate().child(label)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_selection_selects_all_namespaces() {
        assert_eq!(
            NamespaceScope::from_namespaces(Vec::new()),
            NamespaceScope::All
        );
    }

    #[test]
    fn one_selection_is_a_single_namespace_scope() {
        assert_eq!(
            NamespaceScope::from_namespaces(["kube-system".to_string()]),
            NamespaceScope::One("kube-system".to_string())
        );
    }

    #[test]
    fn several_selections_become_a_multi_namespace_scope() {
        assert_eq!(
            NamespaceScope::from_namespaces(["kube-system".to_string(), "default".to_string()]),
            NamespaceScope::Many(vec!["default".to_string(), "kube-system".to_string()])
        );
    }
}
