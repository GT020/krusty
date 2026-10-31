//! Live pod logs panel: container selection, follow toggle, and buffered output.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{ContextMenuExt, DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
};

use crate::ui::components::detail::{
    SelectableTextStyle, selectable_text, text_selection_context_menu,
};
use crate::view_models::resources::pod::logs::PodLogsViewModel;

pub struct PodLogsView {
    vm: Entity<PodLogsViewModel>,
    scroll: ScrollHandle,
    _vm_sub: Subscription,
}

impl PodLogsView {
    pub fn new(vm: Entity<PodLogsViewModel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        Self {
            vm,
            scroll: ScrollHandle::new(),
            _vm_sub: subscription,
        }
    }

    fn toolbar(
        vm: Entity<PodLogsViewModel>,
        containers: Vec<String>,
        container: String,
        follow: bool,
        status: String,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu_vm = vm.clone();
        let menu_container = container.clone();
        let container_button = (!containers.is_empty()).then(|| {
            Button::new("pod-logs-container")
                .ghost()
                .small()
                .label(container)
                .icon(IconName::ChevronsUpDown)
                .dropdown_menu(move |mut menu, _, _| {
                    for name in &containers {
                        let selected = name == &menu_container;
                        let value = name.clone();
                        let vm = menu_vm.clone();
                        menu =
                            menu.item(PopupMenuItem::new(name.clone()).checked(selected).on_click(
                                move |_, _, cx| {
                                    let value = value.clone();
                                    vm.update(cx, |vm, cx| vm.set_container(value, cx));
                                },
                            ));
                    }
                    menu
                })
        });

        let follow_vm = vm.clone();
        let follow_button = Button::new("pod-logs-follow")
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
        let clear_button = Button::new("pod-logs-clear")
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
            "pod-logs-status".to_string(),
            status,
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        ));
        if let Some(error) = error {
            status_row = status_row.child(selectable_text(
                "pod-logs-error".to_string(),
                error,
                SelectableTextStyle::xs().color(cx.theme().danger),
            ));
        }

        let mut container_row = h_flex().gap_2().items_center();
        if let Some(container_button) = container_button {
            container_row = container_row
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Container"),
                )
                .child(container_button);
        }

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .items_center()
                    .child(container_row)
                    .child(actions),
            )
            .child(status_row)
            .into_any_element()
    }

    fn output(logs: String, scroll: &ScrollHandle, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("pod-logs-output")
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .track_scroll(scroll)
            .overflow_y_scrollbar()
            .child(div().w_full().min_w_0().child(selectable_text(
                "pod-logs-text".to_string(),
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
        let line_count = vm.lines().len();
        let logs = vm.lines().join("\n");

        if follow && line_count > 0 {
            self.scroll.scroll_to_bottom();
        }

        v_flex()
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
                cx,
            ))
            .child(Self::output(logs, &self.scroll, cx))
    }
}
