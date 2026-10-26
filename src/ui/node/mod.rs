//! Node workspace: list plus a detail pane that opens on row selection.

mod detail;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_resizable, resizable_panel,
    table::DataTable,
};
use gpui_kit::{
    AppContext as _, Context, FontWeight, IntoElement, ParentElement as _, Pixels, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};

use crate::models::SnapshotPhase;
use crate::view_models::node::NodesViewModel;

const LIST_MIN_WIDTH: Pixels = px(320.);
const LIST_MAX_WIDTH: Pixels = px(720.);
const DETAIL_MIN_WIDTH: Pixels = px(360.);
const SPLIT_MIN_VIEWPORT_WIDTH: f32 = 1080.;

fn list_default_width(viewport_width: f32) -> Pixels {
    if viewport_width >= 1500. {
        px(640.)
    } else if viewport_width >= 1280. {
        px(540.)
    } else {
        px(460.)
    }
}

pub struct NodesView {
    vm: gpui_kit::Entity<NodesViewModel>,
    _vm_sub: Subscription,
}

impl NodesView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let vm = cx.new(|cx| NodesViewModel::new(window, cx));
        let sub = cx.observe(&vm, |_, _, cx| cx.notify());
        Self { vm, _vm_sub: sub }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| {
            vm.set_active(active, cx);
            if active {
                let table = vm.table().clone();
                table.update(cx, |table, cx| table.scroll_to_row(0, cx));
            }
        });
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.close_detail(cx));
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
        let scope = self.vm.read(cx).scope().label();
        let phase = self.vm.read(cx).phase(cx);
        let count = self.vm.read(cx).item_count(cx);

        let status = match &phase {
            SnapshotPhase::Loading => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Loading…")
                .into_any_element(),
            SnapshotPhase::Ready => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{count} objects · {scope}"))
                .into_any_element(),
            SnapshotPhase::Error(message) => div()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(message.clone())
                .into_any_element(),
        };

        let handle = cx.entity().clone();
        h_flex()
            .justify_between()
            .py_2()
            .px_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(SharedString::from("Nodes")),
                    )
                    .child(status),
            )
            .child(
                Button::new("refresh-nodes")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .on_click(move |_, _, cx| {
                        handle.update(cx, |this, cx| {
                            this.vm.update(cx, |vm, cx| vm.refresh(cx));
                        });
                    }),
            )
    }

    fn list(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let table = self.vm.read(cx).table().clone();
        v_flex()
            .size_full()
            .min_w_0()
            .child(self.header(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px_4()
                    .py_3()
                    .child(DataTable::new(&table)),
            )
            .into_any_element()
    }
}

impl Render for NodesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let detail_state = self.vm.read(cx).detail(cx);
        let list = self.list(cx);

        if !detail_state.is_open() {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(list)
                .into_any_element();
        }

        let viewport_width = window.viewport_size().width.as_f32();
        let split = viewport_width >= SPLIT_MIN_VIEWPORT_WIDTH;
        let usage = self.vm.read(cx).usage(cx);
        let usage_error = self.vm.read(cx).usage_error(cx);
        let detail = detail::render_detail(
            detail_state,
            usage,
            usage_error,
            cx.entity().clone(),
            !split,
            cx,
        );

        if !split {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(detail)
                .into_any_element();
        }

        h_resizable("nodes-workspace")
            .child(
                resizable_panel()
                    .size(list_default_width(viewport_width))
                    .size_range(LIST_MIN_WIDTH..LIST_MAX_WIDTH)
                    .fixed()
                    .child(list),
            )
            .child(
                resizable_panel()
                    .size_range(DETAIL_MIN_WIDTH..Pixels::MAX)
                    .child(detail),
            )
            .into_any_element()
    }
}
