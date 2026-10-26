//! Workspace shell: routes the selected resource kind to its list view.

pub mod nav;
pub mod sidebar;
pub mod status_bar;

use crate::cluster::ClusterState;
use crate::models::{NamespaceScope, ResourceKind};
use crate::ui::shell::nav::NavTarget;
use crate::ui::resources::node::NodesView;
use crate::ui::resources::pod::PodsView;
use crate::ui::resources::list::ResourceListView;
use crate::ui::settings::render_settings;
use crate::ui::shell::sidebar::render_sidebar;
use crate::ui::shell::status_bar::render_status_bar;
use crate::ui::settings::theme::{self, AppearanceState};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div,
};
use std::collections::BTreeMap;

pub struct Workspace {
    selected: NavTarget,
    panels: BTreeMap<ResourceKind, Entity<ResourceListView>>,
    pods: Option<Entity<PodsView>>,
    nodes: Option<Entity<NodesView>>,
    _cluster_subscription: gpui_kit::Subscription,
    _appearance_subscription: gpui_kit::Subscription,
    _activation_subscription: gpui_kit::Subscription,
    _preference_subscription: gpui_kit::Subscription,
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
        let panels = BTreeMap::new();
        let pods = Some(cx.new(|cx| PodsView::new(window, cx)));
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace construction complete"
        );
        Self {
            selected: NavTarget::Resource(ResourceKind::Pods),
            panels,
            pods,
            nodes: None,
            _cluster_subscription: cluster_subscription,
            _appearance_subscription: appearance_subscription,
            _activation_subscription: activation_subscription,
            _preference_subscription: preference_subscription,
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
            NavTarget::Settings => render_settings(cx).into_any_element(),
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

fn render_namespace_selector(cx: &Context<Workspace>) -> impl IntoElement {
    let state = cx.global::<ClusterState>();
    let scope_label = state.scope().label();
    Button::new("namespace")
        .ghost()
        .small()
        .label(scope_label)
        .icon(IconName::ChevronsUpDown)
        .dropdown_menu(move |mut menu, _, cx| {
            let state = cx.global::<ClusterState>();
            let namespaces = state.namespaces().to_vec();
            let scope = state.scope().clone();
            menu = menu.item(
                PopupMenuItem::new("All namespaces")
                    .checked(scope.is_all())
                    .on_click(move |_, _, cx| {
                        crate::cluster::set_namespace_scope(cx, NamespaceScope::All);
                    }),
            );
            for namespace in &namespaces {
                let id = namespace.clone();
                let selected = scope.namespace() == Some(namespace.as_str());
                menu = menu.item(
                    PopupMenuItem::new(namespace.clone())
                        .checked(selected)
                        .on_click(move |_, _, cx| {
                            crate::cluster::set_namespace_scope(
                                cx,
                                NamespaceScope::One(id.clone()),
                            );
                        }),
                );
            }
            menu
        })
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = std::time::Instant::now();
        tracing::trace!(selected = ?self.selected, panels = self.panels.len(), "ui: workspace render start");
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
                        .child(render_namespace_selector(cx)),
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
