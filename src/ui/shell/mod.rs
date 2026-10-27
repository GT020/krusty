//! Workspace shell: routes the selected resource kind to its list view.

pub mod nav;
pub mod sidebar;
pub mod status_bar;

use crate::cluster::ClusterState;
use crate::models::ResourceKind;
use crate::ui::resources::list::ResourceListView;
use crate::ui::resources::node::NodesView;
use crate::ui::resources::pod::PodsView;
use crate::ui::settings::custom_resources::CustomResourcesView;
use crate::ui::settings::render_settings;
use crate::ui::settings::theme::{self, AppearanceState};
use crate::ui::shell::nav::NavTarget;
use crate::ui::shell::sidebar::render_sidebar;
use crate::ui::shell::status_bar::render_status_bar;
use crate::view_models::shell::{NamespaceSelect, ShellViewModel};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::select::Select;
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
    shell: Entity<ShellViewModel>,
    panels: BTreeMap<ResourceKind, Entity<ResourceListView>>,
    pods: Option<Entity<PodsView>>,
    nodes: Option<Entity<NodesView>>,
    custom_resources: Entity<CustomResourcesView>,
    _shell_subscription: gpui_kit::Subscription,
    _appearance_subscription: gpui_kit::Subscription,
    _activation_subscription: gpui_kit::Subscription,
    _preference_subscription: gpui_kit::Subscription,
    _custom_resources_subscription: gpui_kit::Subscription,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let started = std::time::Instant::now();
        tracing::info!("ui: workspace construction start");
        let shell = cx.new(|cx| ShellViewModel::new(window, cx));
        let shell_subscription = cx.observe(&shell, |_, _, cx| cx.notify());
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

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace construction complete"
        );
        Self {
            shell,
            panels,
            pods,
            nodes: None,
            custom_resources,
            _shell_subscription: shell_subscription,
            _appearance_subscription: appearance_subscription,
            _activation_subscription: activation_subscription,
            _preference_subscription: preference_subscription,
            _custom_resources_subscription: custom_resources_subscription,
        }
    }

    pub fn select(&mut self, target: NavTarget, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.shell.read(cx).selected();
        if !self.shell.update(cx, |shell, cx| shell.select(target, cx)) {
            return;
        }
        tracing::info!(from = ?previous, to = ?target, "ui: select navigation target");
        if let NavTarget::Resource(kind) = previous {
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

    fn content(&self, selected: NavTarget, cx: &Context<Self>) -> gpui_kit::AnyElement {
        match selected {
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

fn render_context_selector(
    cx: &Context<Workspace>,
    shell: &Entity<ShellViewModel>,
) -> impl IntoElement {
    let label = shell.read(cx).context_label(cx);
    let shell = shell.clone();
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
                let shell = shell.clone();
                menu = menu.item(
                    PopupMenuItem::new(context.label().to_string())
                        .checked(selected)
                        .on_click(move |_, _, cx| {
                            if let Err(error) =
                                shell.update(cx, |shell, cx| shell.switch_context(&id, cx))
                            {
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

fn render_namespace_selector(select: &Entity<NamespaceSelect>) -> impl IntoElement {
    Select::new(select)
        .id("namespace")
        .small()
        .w(px(180.))
        .menu_max_h(px(320.))
        .placeholder("All namespaces")
        .search_placeholder("Search namespaces")
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = std::time::Instant::now();
        let selected = self.shell.read(cx).selected();
        let namespace_select = self.shell.read(cx).namespace_select().clone();
        tracing::trace!(selected = ?selected, panels = self.panels.len(), "ui: workspace render start");
        let sidebar = render_sidebar(selected, cx.entity(), cx);

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
                        .child(render_context_selector(cx, &self.shell))
                        .child(render_namespace_selector(&namespace_select)),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(self.content(selected, cx)),
            )
            .child(render_status_bar(cx));
        tracing::trace!(
            selected = ?selected,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace render end"
        );
        root
    }
}
