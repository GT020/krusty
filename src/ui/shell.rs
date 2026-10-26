//! Workspace shell: routes the selected resource kind to its list view.

use crate::models::ResourceKind;
use crate::ui::resource_list::ResourceListView;
use crate::ui::sidebar::render_sidebar;
use crate::ui::status_bar::render_status_bar;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{ActiveTheme, TitleBar};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div,
};
use std::collections::BTreeMap;

pub struct Workspace {
    selected: ResourceKind,
    panels: BTreeMap<ResourceKind, Entity<ResourceListView>>,
    _cluster_subscription: gpui_kit::Subscription,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let started = std::time::Instant::now();
        tracing::info!("ui: workspace construction start");
        let cluster_subscription =
            cx.observe_global::<crate::cluster::ClusterState>(|_, cx| cx.notify());
        let mut panels = BTreeMap::new();
        panels.insert(
            ResourceKind::Pods,
            cx.new(|cx| ResourceListView::new(ResourceKind::Pods, window, cx)),
        );
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace construction complete"
        );
        Self {
            selected: ResourceKind::Pods,
            panels,
            _cluster_subscription: cluster_subscription,
        }
    }

    pub fn select_kind(&mut self, kind: ResourceKind, window: &mut Window, cx: &mut Context<Self>) {
        if kind == self.selected {
            return;
        }
        tracing::info!(from = ?self.selected, to = ?kind, "ui: select resource kind");
        if let Some(previous) = self.panels.get(&self.selected).cloned() {
            previous.update(cx, |view, cx| view.set_active(false, cx));
        }
        self.selected = kind;
        let panel = self
            .panels
            .entry(kind)
            .or_insert_with(|| cx.new(|cx| ResourceListView::new(kind, window, cx)))
            .clone();
        panel.update(cx, |view, cx| view.set_active(true, cx));
        cx.notify();
    }

    fn content(&self) -> gpui_kit::AnyElement {
        self.panels
            .get(&self.selected)
            .cloned()
            .expect("selected panel exists")
            .into_any_element()
    }
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
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("krusty"),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(self.content()),
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
