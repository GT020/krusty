//! Live pod logs panel and the reusable logs widget.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{ContextMenuExt, DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div,
};

use crate::models::dashboard::{PodWidgetSource, WidgetSource};
use crate::ui::components::detail::{
    SelectableTextStyle, selectable_text, text_selection_context_menu,
};
use crate::ui::dashboard::pinnable_sized;
use crate::view_models::resources::pod::logs::PodLogsViewModel;

pub struct PodLogsView {
    vm: Entity<PodLogsViewModel>,
    id_prefix: SharedString,
    scroll: ScrollHandle,
    _vm_sub: Subscription,
}

impl PodLogsView {
    pub fn new(
        vm: Entity<PodLogsViewModel>,
        id_prefix: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        Self {
            vm,
            id_prefix,
            scroll: ScrollHandle::new(),
            _vm_sub: subscription,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn toolbar(
        vm: Entity<PodLogsViewModel>,
        containers: Vec<String>,
        container: String,
        follow: bool,
        status: String,
        error: Option<String>,
        id_prefix: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu_vm = vm.clone();
        let menu_container = container.clone();
        let container_button = Button::new(format!("{id_prefix}-container"))
            .ghost()
            .small()
            .label(container)
            .icon(IconName::ChevronsUpDown)
            .dropdown_menu(move |mut menu, _, _| {
                for name in &containers {
                    let selected = name == &menu_container;
                    let value = name.clone();
                    let vm = menu_vm.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).checked(selected).on_click(
                        move |_, _, cx| {
                            let value = value.clone();
                            vm.update(cx, |vm, cx| vm.set_container(value, cx));
                        },
                    ));
                }
                menu
            });

        let follow_vm = vm.clone();
        let follow_button = Button::new(format!("{id_prefix}-follow"))
            .ghost()
            .small()
            .label(if follow { "Following" } else { "Paused" })
            .icon(if follow {
                IconName::Pause
            } else {
                IconName::Play
            })
            .on_click(move |_, _, cx| {
                follow_vm.update(cx, |vm, cx| vm.toggle_follow(cx));
            });

        let clear_vm = vm;
        let clear_button = Button::new(format!("{id_prefix}-clear"))
            .ghost()
            .small()
            .icon(IconName::Trash)
            .tooltip("Clear logs")
            .on_click(move |_, _, cx| {
                clear_vm.update(cx, |vm, cx| vm.clear(cx));
            });

        let actions = h_flex()
            .gap_1()
            .items_center()
            .child(follow_button)
            .child(clear_button);

        let mut status_row = h_flex().gap_2().items_center().child(selectable_text(
            format!("{id_prefix}-status"),
            status,
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        ));
        if let Some(error) = error {
            status_row = status_row.child(selectable_text(
                format!("{id_prefix}-error"),
                error,
                SelectableTextStyle::xs().color(cx.theme().danger),
            ));
        }

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Container"),
                            )
                            .child(container_button),
                    )
                    .child(actions),
            )
            .child(status_row)
            .into_any_element()
    }

    fn output(
        logs: String,
        scroll: &ScrollHandle,
        id_prefix: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(format!("{id_prefix}-output"))
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .track_scroll(scroll)
            .overflow_y_scrollbar()
            .child(div().w_full().min_w_0().child(selectable_text(
                format!("{id_prefix}-text"),
                logs,
                SelectableTextStyle::xs().monospace(),
            )))
            .context_menu(text_selection_context_menu)
            .into_any_element()
    }
}

impl Render for PodLogsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = self.vm.read(cx);
        let containers = vm.containers().to_vec();
        let container = vm.container().to_string();
        let follow = vm.follow();
        let status = vm.status().to_string();
        let error = vm.error().map(str::to_string);
        let line_count = vm.line_count();
        let logs = vm.text().to_string();

        if follow && line_count > 0 {
            self.scroll.scroll_to_bottom();
        }

        let source = WidgetSource::Pod(PodWidgetSource::Logs {
            namespace: vm.namespace().to_string(),
            name: vm.name().to_string(),
            containers: vm.containers().to_vec(),
            container: Some(vm.container().to_string()),
            follow: vm.follow(),
        });
        let root = v_flex()
            .size_full()
            .min_w_0()
            .gap_3()
            .p_3()
            .child(Self::toolbar(
                self.vm.clone(),
                containers,
                container,
                follow,
                status,
                error,
                &self.id_prefix,
                cx,
            ))
            .child(Self::output(logs, &self.scroll, &self.id_prefix, cx))
            .into_any_element();
        pinnable_sized(format!("pod-logs-menu-{}", self.id_prefix), root, source)
    }
}

/// Arguments for mounting a pod logs widget in classic mode or on the dashboard.
pub struct PodLogsArgs {
    pub namespace: String,
    pub name: String,
    pub containers: Vec<String>,
    pub container: Option<String>,
    pub follow: bool,
    pub id_prefix: SharedString,
}

/// Reusable logs component. Owns its own stream and scroll state.
pub struct PodLogsWidget {
    vm: Entity<PodLogsViewModel>,
    view: Entity<PodLogsView>,
    namespace: String,
    name: String,
}

impl PodLogsWidget {
    pub fn mount(args: PodLogsArgs, cx: &mut App) -> Self {
        let vm = cx.new(|cx| {
            PodLogsViewModel::new_with(
                args.namespace.clone(),
                args.name.clone(),
                args.containers.clone(),
                args.container.clone(),
                args.follow,
                cx,
            )
        });
        let view = cx.new(|cx| PodLogsView::new(vm.clone(), args.id_prefix, cx));
        Self {
            vm,
            view,
            namespace: args.namespace,
            name: args.name,
        }
    }

    pub fn element(&self, _cx: &App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    pub fn set_active(&self, active: bool, cx: &mut App) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }

    pub fn shutdown(&self, cx: &mut App) {
        self.vm.update(cx, |vm, _| vm.shutdown());
    }

    pub fn title(&self) -> String {
        format!("Logs · {}/{}", self.namespace, self.name)
    }
}
