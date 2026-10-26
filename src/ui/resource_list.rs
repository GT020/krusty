//! Generic resource list view: header + virtualized table.

use crate::models::{ResourceKind, SnapshotPhase};
use crate::view_models::ResourceViewModel;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    Context, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};

pub struct ResourceListView {
    title: SharedString,
    vm: ResourceViewModel,
    _vm_sub: Subscription,
}

impl ResourceListView {
    pub fn new(kind: ResourceKind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let vm = ResourceViewModel::new(kind, window, cx);
        let sub = vm.observe(cx, |_, cx| cx.notify());
        Self {
            title: kind.label().into(),
            vm,
            _vm_sub: sub,
        }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.set_active(active, cx);
        if active {
            self.vm.scroll_to_top(cx);
        }
    }
}

impl Render for ResourceListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scope = self.vm.scope(cx).label();
        let phase = self.vm.phase(cx);
        let count = self.vm.item_count(cx);

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

        let table_el = self.vm.render_table(cx);
        let handle = cx.entity().clone();

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
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
                                    .child(self.title.clone()),
                            )
                            .child(status),
                    )
                    .child(
                        Button::new("refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.vm.refresh(cx);
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px_4()
                    .py_3()
                    .child(table_el),
            )
    }
}
