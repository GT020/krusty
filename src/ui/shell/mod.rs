//! Workspace shell: routes the selected resource kind to its list view.

pub mod nav;
pub mod sidebar;
pub mod status_bar;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, ResourceKind};
use crate::ui::resources::list::ResourceListView;
use crate::ui::resources::node::NodesView;
use crate::ui::resources::pod::PodsView;
use crate::ui::settings::custom_resources::CustomResourcesView;
use crate::ui::settings::render_settings;
use crate::ui::settings::theme::{self, AppearanceState};
use crate::ui::shell::nav::NavTarget;
use crate::ui::shell::sidebar::render_sidebar;
use crate::ui::shell::status_bar::render_status_bar;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::select::{
    SearchableVec, Select, SelectEvent, SelectItem, SelectState,
};
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div, px,
};
use std::collections::BTreeMap;

pub struct Workspace {
    selected: NavTarget,
    panels: BTreeMap<ResourceKind, Entity<ResourceListView>>,
    pods: Option<Entity<PodsView>>,
    nodes: Option<Entity<NodesView>>,
    custom_resources: Entity<CustomResourcesView>,
    namespace_select: Entity<SelectState<SearchableVec<NamespaceItem>>>,
    /// Last-synced values mirrored into `namespace_select`; used to avoid
    /// rebuilding the list on every render, which would discard the user's
    /// in-progress search filter.
    synced_namespaces: Vec<String>,
    synced_scope: NamespaceScope,
    _cluster_subscription: gpui_kit::Subscription,
    _appearance_subscription: gpui_kit::Subscription,
    _activation_subscription: gpui_kit::Subscription,
    _preference_subscription: gpui_kit::Subscription,
    _custom_resources_subscription: gpui_kit::Subscription,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let started = std::time::Instant::now();
        tracing::info!("ui: workspace construction start");
        let cluster_subscription =
            cx.observe_global::<crate::cluster::ClusterState>(|_, cx| cx.notify());
        let appearance_subscription = cx.observe_window_appearance(window, |_, window, cx| {
            theme::sync_system_appearance(window, cx);
        });
        let activation_subscription = cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                theme::sync_system_appearance(window, cx);
            }
        });
        let preference_subscription = cx.observe_global::<AppearanceState>(|_, cx| cx.notify());
        let custom_resources = cx.new(|_| CustomResourcesView::new());
        let custom_resources_subscription =
            cx.observe_global::<crate::ui::settings::custom_resources::CustomResourcesState>(
                |this, cx| {
                    this.panels
                        .retain(|kind, _| !matches!(kind, ResourceKind::Custom(_)));
                    cx.notify();
                },
            );
        let panels = BTreeMap::new();
        let pods = Some(cx.new(|cx| PodsView::new(window, cx)));

        let namespace_select = cx.new(|cx| {
            let state = cx.global::<ClusterState>();
            let items = namespace_items(state.namespaces());
            let selected = state.scope().namespace().map(str::to_string);
            let mut select = SelectState::new(items, None, window, cx).searchable(true);
            if let Some(selected) = selected {
                select.set_selected_value(&selected, window, cx);
            }
            select
        });
        let synced_namespaces = cx.global::<ClusterState>().namespaces().to_vec();
        let synced_scope = cx.global::<ClusterState>().scope().clone();
        cx.subscribe(
            &namespace_select,
            |_, _, event: &SelectEvent<SearchableVec<NamespaceItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                let scope = match value {
                    Some(namespace) if !namespace.is_empty() => {
                        NamespaceScope::One(namespace.clone())
                    }
                    _ => NamespaceScope::All,
                };
                crate::cluster::set_namespace_scope(cx, scope);
            },
        )
        .detach();

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace construction complete"
        );
        Self {
            selected: NavTarget::Resource(ResourceKind::Pods),
            panels,
            pods,
            nodes: None,
            custom_resources,
            namespace_select,
            synced_namespaces,
            synced_scope,
            _cluster_subscription: cluster_subscription,
            _appearance_subscription: appearance_subscription,
            _activation_subscription: activation_subscription,
            _preference_subscription: preference_subscription,
            _custom_resources_subscription: custom_resources_subscription,
        }
    }

    pub fn select(&mut self, target: NavTarget, window: &mut Window, cx: &mut Context<Self>) {
        if target == self.selected {
            return;
        }
        tracing::info!(from = ?self.selected, to = ?target, "ui: select navigation target");
        if let NavTarget::Resource(kind) = self.selected {
            match kind {
                ResourceKind::Pods => {
                    if let Some(pods) = self.pods.clone() {
                        pods.update(cx, |view, cx| view.set_active(false, cx));
                    }
                }
                ResourceKind::Nodes => {
                    if let Some(nodes) = self.nodes.clone() {
                        nodes.update(cx, |view, cx| view.set_active(false, cx));
                    }
                }
                _ => {
                    if let Some(previous) = self.panels.get(&kind).cloned() {
                        previous.update(cx, |view, cx| view.set_active(false, cx));
                    }
                }
            }
        }
        self.selected = target;
        if let NavTarget::Resource(kind) = target {
            match kind {
                ResourceKind::Pods => {
                    let pods = self
                        .pods
                        .get_or_insert_with(|| cx.new(|cx| PodsView::new(window, cx)))
                        .clone();
                    pods.update(cx, |view, cx| view.set_active(true, cx));
                }
                ResourceKind::Nodes => {
                    let nodes = self
                        .nodes
                        .get_or_insert_with(|| cx.new(|cx| NodesView::new(window, cx)))
                        .clone();
                    nodes.update(cx, |view, cx| view.set_active(true, cx));
                }
                _ => {
                    let panel = self
                        .panels
                        .entry(kind)
                        .or_insert_with(|| cx.new(|cx| ResourceListView::new(kind, window, cx)))
                        .clone();
                    panel.update(cx, |view, cx| view.set_active(true, cx));
                }
            }
        }
        cx.notify();
    }

    fn content(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        match self.selected {
            NavTarget::Resource(ResourceKind::Pods) => self
                .pods
                .clone()
                .expect("selected pod panel exists")
                .into_any_element(),
            NavTarget::Resource(ResourceKind::Nodes) => self
                .nodes
                .clone()
                .expect("selected node panel exists")
                .into_any_element(),
            NavTarget::Resource(kind) => self
                .panels
                .get(&kind)
                .cloned()
                .expect("selected panel exists")
                .into_any_element(),
            NavTarget::Settings => {
                render_settings(cx, self.custom_resources.clone()).into_any_element()
            }
        }
    }
}

fn render_context_selector(cx: &Context<Workspace>) -> impl IntoElement {
    let state = cx.global::<ClusterState>();
    let label = state.context().unwrap_or("No context").to_string();
    Button::new("context")
        .ghost()
        .small()
        .label(label)
        .icon(IconName::ChevronsUpDown)
        .dropdown_menu(move |mut menu, _, cx| {
            let state = cx.global::<ClusterState>();
            let contexts = state.contexts().to_vec();
            let active = state.active_id().map(str::to_string);
            if contexts.is_empty() {
                return menu.item(PopupMenuItem::new("No contexts found").disabled(true));
            }
            for context in &contexts {
                let id = context.id().to_string();
                let selected = active.as_deref() == Some(context.id());
                menu = menu.item(
                    PopupMenuItem::new(context.label().to_string())
                        .checked(selected)
                        .on_click(move |_, _, cx| {
                            if let Err(error) = crate::cluster::switch_context(cx, &id) {
                                tracing::error!(
                                    context = %id,
                                    %error,
                                    "ui: switch context failed"
                                );
                            }
                        }),
                );
            }
            menu
        })
}

/// One row in the namespace selector. `name` is empty for the "All namespaces"
/// sentinel; `value()` distinguishes the scope.
#[derive(Clone, Debug)]
struct NamespaceItem {
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

    fn title(&self) -> gpui_kit::SharedString {
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

/// Sync the namespace select from the cluster global, but only when the
/// namespaces list or the scope actually changed. `set_items` rebuilds the
/// delegate and would drop the in-progress search filter, so we avoid calling
/// it on every render.
fn sync_namespace_select(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let state = cx.global::<ClusterState>();
    let namespaces = state.namespaces().to_vec();
    let scope = state.scope().clone();

    let namespaces_changed = namespaces != workspace.synced_namespaces;
    let scope_changed = scope != workspace.synced_scope;
    if !namespaces_changed && !scope_changed {
        return;
    }

    workspace.synced_namespaces = namespaces.clone();
    workspace.synced_scope = scope.clone();

    workspace.namespace_select.update(cx, |select, cx| {
        if namespaces_changed {
            select.set_items(namespace_items(&namespaces), window, cx);
        }
        let value = scope.namespace().map(str::to_string).unwrap_or_default();
        if select.selected_value() != Some(&value) {
            select.set_selected_value(&value, window, cx);
        }
    });
}

fn render_namespace_selector(
    select: &Entity<SelectState<SearchableVec<NamespaceItem>>>,
) -> impl IntoElement {
    Select::new(select)
        .id("namespace")
        .small()
        .w(px(180.))
        .menu_max_h(px(320.))
        .placeholder("All namespaces")
        .search_placeholder("Search namespaces")
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = std::time::Instant::now();
        tracing::trace!(selected = ?self.selected, panels = self.panels.len(), "ui: workspace render start");
        sync_namespace_select(self, window, cx);
        let sidebar = render_sidebar(self.selected, cx.entity(), cx);

        let root = v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("krusty"),
                        )
                        .child(render_context_selector(cx))
                        .child(render_namespace_selector(&self.namespace_select)),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(self.content(cx)),
            )
            .child(render_status_bar(cx));
        tracing::trace!(
            selected = ?self.selected,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace render end"
        );
        root
    }
}
