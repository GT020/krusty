//! Custom dashboard: pins per-resource widgets and composes their chrome.

mod frame;
mod widget;

pub use widget::{ResourceWidget, mount_widget, widget_title};

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};

use crate::cluster::ClusterState;
use crate::models::dashboard::{
    DashboardFile, DashboardId, DeploymentWidgetSource, NodeWidgetSource, PinnedWidgetSpec,
    PodWidgetSource, WidgetDescriptor, WidgetId, WidgetSize, WidgetSource,
};
use crate::repository::dashboard_store;

use self::frame::widget_frame;

struct MountedEntry {
    id: WidgetId,
    widget: Option<ResourceWidget>,
}

pub struct DashboardView {
    file: DashboardFile,
    entries: Vec<MountedEntry>,
    mounted_dashboard: Option<DashboardId>,
    active: bool,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the context observer
    cluster_sub: Subscription,
}

impl DashboardView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let file = dashboard_store::load();
        let cluster_sub = cx.observe_global_in::<ClusterState>(window, |this, window, cx| {
            this.sync_context(window, cx);
            cx.notify();
        });
        let mut this = Self {
            file,
            entries: Vec::new(),
            mounted_dashboard: None,
            active: false,
            cluster_sub,
        };
        this.sync_context(window, cx);
        this
    }

    pub fn dashboard_choices(&self) -> Vec<(DashboardId, String)> {
        self.file.dashboard_choices()
    }

    pub fn active_dashboard(&self) -> DashboardId {
        self.file.active
    }

    pub fn set_active_dashboard(
        &mut self,
        id: DashboardId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.file.set_active(id) {
            self.remount(window, cx);
            self.save();
            cx.notify();
        }
    }

    pub fn add_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.file.next_dashboard_id;
        let id = self.file.add_dashboard(format!("Dashboard {next}"));
        self.file.set_active(id);
        self.remount(window, cx);
        self.save();
        cx.notify();
    }

    pub fn remove_active_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.file.active;
        if self.file.remove_dashboard(active) {
            self.remount(window, cx);
            self.save();
            cx.notify();
        }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        for entry in &self.entries {
            if let Some(widget) = &entry.widget {
                widget.set_active(active, cx);
            }
        }
    }

    pub fn add_to(
        &mut self,
        dashboard: DashboardId,
        descriptor: WidgetDescriptor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.file.contains(dashboard, &descriptor) {
            return Ok(());
        }
        let size = default_size(&descriptor.source);
        let Some(id) = self.file.add_widget(dashboard, descriptor.clone(), size) else {
            return Err("Dashboard no longer exists.".to_string());
        };
        if dashboard == self.file.active {
            let widget = self.mount(id, &descriptor, window, cx);
            if let Some(widget) = &widget {
                widget.set_active(self.active, cx);
            }
            self.entries.push(MountedEntry { id, widget });
        }
        self.save();
        cx.notify();
        Ok(())
    }

    pub fn remove(&mut self, id: WidgetId, cx: &mut Context<Self>) {
        if let Some(index) = self.entries.iter().position(|entry| entry.id == id) {
            let entry = self.entries.remove(index);
            if let Some(widget) = entry.widget {
                widget.shutdown(cx);
            }
        }
        if self.file.remove_widget(self.file.active, id) {
            self.save();
            cx.notify();
        }
    }

    pub fn set_size(&mut self, id: WidgetId, size: WidgetSize, cx: &mut Context<Self>) {
        if self.file.set_widget_size(self.file.active, id, size) {
            self.save();
            cx.notify();
        }
    }

    fn sync_context(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mounted_dashboard != Some(self.file.active) {
            self.remount(window, cx);
            return;
        }
        let active = cx.global::<ClusterState>().active_id().map(str::to_string);
        let specs: Vec<PinnedWidgetSpec> = self.file.active_dashboard().widgets.clone();
        for spec in specs {
            let matches = active.as_deref() == Some(spec.descriptor.context_id.as_str());
            let existing = self.entries.iter().position(|entry| entry.id == spec.id);
            match (matches, existing) {
                (true, None) => {
                    let widget = self.mount(spec.id, &spec.descriptor, window, cx);
                    if let Some(widget) = &widget {
                        widget.set_active(self.active, cx);
                    }
                    self.entries.push(MountedEntry {
                        id: spec.id,
                        widget,
                    });
                }
                (false, Some(index)) => {
                    let entry = self.entries.remove(index);
                    if let Some(widget) = entry.widget {
                        widget.shutdown(cx);
                    }
                }
                _ => {}
            }
        }
        self.entries.sort_by_key(|entry| entry.id);
    }

    fn remount(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for entry in self.entries.drain(..) {
            if let Some(widget) = entry.widget {
                widget.shutdown(cx);
            }
        }
        let active_context = cx.global::<ClusterState>().active_id().map(str::to_string);
        let specs: Vec<PinnedWidgetSpec> = self.file.active_dashboard().widgets.clone();
        for spec in specs {
            if active_context.as_deref() != Some(spec.descriptor.context_id.as_str()) {
                continue;
            }
            let widget = self.mount(spec.id, &spec.descriptor, window, cx);
            if let Some(widget) = &widget {
                widget.set_active(self.active, cx);
            }
            self.entries.push(MountedEntry {
                id: spec.id,
                widget,
            });
        }
        self.mounted_dashboard = Some(self.file.active);
        self.entries.sort_by_key(|entry| entry.id);
    }

    fn mount(
        &mut self,
        id: WidgetId,
        descriptor: &WidgetDescriptor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ResourceWidget> {
        match mount_widget(id, descriptor, window, cx) {
            Ok(widget) => Some(widget),
            Err(error) => {
                tracing::warn!(%error, widget = id.get(), "dashboard: could not mount widget");
                None
            }
        }
    }

    fn save(&self) {
        if let Err(error) = dashboard_store::save(&self.file) {
            tracing::warn!(%error, "dashboard: could not save");
        }
    }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = self.file.dashboard_choices();
        let selected = choices
            .iter()
            .position(|(id, _)| *id == self.file.active)
            .unwrap_or(0);
        let mut tabs = TabBar::new(("dashboard-tabs", cx.entity().entity_id()))
            .small()
            .menu(false)
            .selected_index(selected)
            .on_click(cx.listener(|this, index, window, cx| {
                if let Some((id, _)) = this.file.dashboard_choices().get(*index).cloned() {
                    this.set_active_dashboard(id, window, cx);
                }
            }));
        for (_, name) in &choices {
            tabs = tabs.child(Tab::new().label(name.clone()));
        }

        let add = Button::new("dashboard-add")
            .ghost()
            .small()
            .icon(IconName::Plus)
            .tooltip("New dashboard")
            .on_click(cx.listener(|this, _, window, cx| {
                this.add_dashboard(window, cx);
            }));
        let remove = (choices.len() > 1).then(|| {
            Button::new("dashboard-remove")
                .ghost()
                .small()
                .icon(IconName::Trash)
                .tooltip("Delete this dashboard")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.remove_active_dashboard(window, cx);
                }))
        });

        let widgets = self.file.active_dashboard().widgets.clone();
        let content = if widgets.is_empty() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::LayoutDashboard)
                        .size_8()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child("No widgets pinned yet"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Right-click a widget and choose a dashboard to pin it."),
                )
                .into_any_element()
        } else {
            let mut cards = Vec::new();
            for spec in &widgets {
                let entry = self.entries.iter().find(|entry| entry.id == spec.id);
                let content = entry
                    .and_then(|entry| entry.widget.as_ref())
                    .map(|widget| widget.element(cx));
                let content = match content {
                    Some(content) => content,
                    None => unavailable_widget(&spec.descriptor.context_id, cx),
                };
                cards.push(widget_frame(
                    cx.entity(),
                    spec.id,
                    widget_title(&spec.descriptor),
                    format!("context {}", spec.descriptor.context_id),
                    spec.size,
                    content,
                    cx,
                ));
            }
            div()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_4()
                        .p_4()
                        .items_start()
                        .children(cards),
                )
                .into_any_element()
        };

        v_flex()
            .size_full()
            .min_w_0()
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .py_2()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(tabs)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} widgets", widgets.len())),
                            )
                            .child(add)
                            .when_some(remove, |this, button| this.child(button)),
                    ),
            )
            .child(content)
            .into_any_element()
    }
}

fn unavailable_widget(context_id: &str, cx: &App) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p_4()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Pinned from another cluster context"),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(context_id.to_string())),
        )
        .into_any_element()
}

fn default_size(source: &WidgetSource) -> WidgetSize {
    match source {
        WidgetSource::Pod(PodWidgetSource::Usage { .. }) => WidgetSize::Medium,
        WidgetSource::Pod(PodWidgetSource::Logs { .. }) => WidgetSize::Large,
        WidgetSource::Pod(PodWidgetSource::Exec { .. }) => WidgetSize::Large,
        WidgetSource::Pod(PodWidgetSource::List { .. }) => WidgetSize::Large,
        WidgetSource::Pod(PodWidgetSource::Section { .. }) => WidgetSize::Medium,
        WidgetSource::Node(NodeWidgetSource::Usage { .. }) => WidgetSize::Medium,
        WidgetSource::Node(NodeWidgetSource::Section { .. }) => WidgetSize::Medium,
        WidgetSource::Deployment(DeploymentWidgetSource::Actions { .. }) => WidgetSize::Small,
        WidgetSource::Deployment(DeploymentWidgetSource::Section { .. }) => WidgetSize::Medium,
        WidgetSource::Section(_) => WidgetSize::Medium,
    }
}

/// Global handle so any classic surface can pin a widget.
#[derive(Clone)]
pub struct DashboardHandle {
    pub view: Entity<DashboardView>,
}

impl Global for DashboardHandle {}

pub fn set_handle(cx: &mut App, view: Entity<DashboardView>) {
    cx.set_global(DashboardHandle { view });
}

pub fn pin_to(
    cx: &mut App,
    window: &mut Window,
    source: WidgetSource,
    dashboard: DashboardId,
) -> Result<(), String> {
    let descriptor = WidgetDescriptor {
        context_id: cx
            .global::<ClusterState>()
            .active_id()
            .unwrap_or_default()
            .to_string(),
        source,
    };
    let view = cx.global::<DashboardHandle>().view.clone();
    view.update(cx, |view, cx| {
        view.add_to(dashboard, descriptor, window, cx)
    })
}

pub fn pin_to_new(cx: &mut App, window: &mut Window, source: WidgetSource) -> Result<(), String> {
    let descriptor = WidgetDescriptor {
        context_id: cx
            .global::<ClusterState>()
            .active_id()
            .unwrap_or_default()
            .to_string(),
        source,
    };
    let view = cx.global::<DashboardHandle>().view.clone();
    view.update(cx, |view, cx| {
        view.add_dashboard(window, cx);
        let active = view.active_dashboard();
        view.add_to(active, descriptor, window, cx)
    })
}

/// Append dashboard choices to a widget's right-click menu.
pub fn pin_menu(menu: PopupMenu, source: WidgetSource, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let Some(handle) = cx.try_global::<DashboardHandle>() else {
        return menu.item(PopupMenuItem::new("Dashboard not ready").disabled(true));
    };
    let choices = handle.view.read(cx).dashboard_choices();
    let mut menu = menu;
    for (id, name) in choices {
        let source = source.clone();
        menu = menu.item(PopupMenuItem::new(format!("Pin to {name}")).on_click(
            move |_, window, cx| {
                let _ = pin_to(cx, window, source.clone(), id);
            },
        ));
    }
    menu.item(
        PopupMenuItem::new("Pin to new dashboard…").on_click(move |_, window, cx| {
            let _ = pin_to_new(cx, window, source.clone());
        }),
    )
}

/// Wrap an inline widget so right-clicking it offers the pin menu.
pub fn pinnable(
    id: impl Into<SharedString>,
    content: AnyElement,
    source: WidgetSource,
) -> AnyElement {
    div()
        .id(id.into())
        .context_menu(move |menu, _, cx| pin_menu(menu, source.clone(), cx))
        .child(content)
        .into_any_element()
}

/// Wrap a widget that owns its full layout box. Unlike [`pinnable`], this keeps
/// `size_full`/`flex_1` sizing so tables, logs, terminals, and sections do not
/// collapse inside a flex parent.
pub fn pinnable_sized(
    id: impl Into<SharedString>,
    content: AnyElement,
    source: WidgetSource,
) -> AnyElement {
    div()
        .id(id.into())
        .size_full()
        .min_w_0()
        .min_h_0()
        .context_menu(move |menu, _, cx| pin_menu(menu, source.clone(), cx))
        .child(content)
        .into_any_element()
}
