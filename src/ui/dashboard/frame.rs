//! Dashboard card chrome: title, context, size, unpin, and content.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, Context, Entity, IntoElement, ParentElement as _, Pixels, SharedString,
    Styled as _, div, px,
};

use crate::models::dashboard::{WidgetId, WidgetSize};
use crate::ui::dashboard::DashboardView;

pub fn widget_dimensions(size: WidgetSize) -> (Pixels, Pixels) {
    match size {
        WidgetSize::Small => (px(340.), px(220.)),
        WidgetSize::Medium => (px(520.), px(320.)),
        WidgetSize::Large => (px(760.), px(460.)),
        WidgetSize::Full => (px(1120.), px(320.)),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn widget_frame(
    view: Entity<DashboardView>,
    id: WidgetId,
    title: String,
    context: String,
    size: WidgetSize,
    content: AnyElement,
    cx: &mut Context<DashboardView>,
) -> AnyElement {
    let (width, height) = widget_dimensions(size);
    let remove_view = view.clone();
    let size_view = view;
    let current_size = size;

    let size_menu = Button::new(SharedString::from(format!("{id}-size")))
        .ghost()
        .xsmall()
        .label(size.label())
        .dropdown_menu(move |mut menu, _, _| {
            for option in WidgetSize::ALL {
                let menu_view = size_view.clone();
                menu = menu.item(
                    PopupMenuItem::new(option.label())
                        .checked(option == current_size)
                        .on_click(move |_, _, cx| {
                            menu_view.update(cx, |view, cx| view.set_size(id, option, cx));
                        }),
                );
            }
            menu
        });

    v_flex()
        .w(width)
        .h(height)
        .min_w_0()
        .min_h_0()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            h_flex()
                .justify_between()
                .gap_3()
                .items_center()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .child(context),
                        ),
                )
                .child(
                    h_flex().gap_1().items_center().child(size_menu).child(
                        Button::new(SharedString::from(format!("{id}-remove")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::CircleX)
                            .tooltip("Remove widget")
                            .on_click(move |_, _, cx| {
                                remove_view.update(cx, |view, cx| view.remove(id, cx));
                            }),
                    ),
                ),
        )
        .child(div().flex_1().min_h_0().overflow_hidden().child(content))
        .into_any_element()
}
