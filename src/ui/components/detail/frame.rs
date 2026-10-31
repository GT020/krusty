//! Shared frame, header, and state blocks for detail panes.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, div,
};

use super::menu::text_selection_context_menu;
use super::text::{SelectableTextStyle, selectable_text};

pub(crate) fn detail_shell<T: 'static>(
    header: AnyElement,
    body: AnyElement,
    compact: bool,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut root = v_flex()
        .size_full()
        .min_w_0()
        .bg(cx.theme().background)
        .child(header)
        .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(body));
    if !compact {
        root = root.border_l_1().border_color(cx.theme().border);
    }
    root.id("detail-shell")
        .context_menu(text_selection_context_menu)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn detail_header<T: 'static>(
    title: &str,
    subtitle: &str,
    compact: bool,
    back_tooltip: &str,
    id_prefix: &str,
    view: Entity<T>,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    let back_handle = view.clone();
    let close_handle = view;
    let close_button = if compact {
        Button::new(format!("{id_prefix}-back"))
            .ghost()
            .small()
            .icon(IconName::ArrowLeft)
            .label("Back")
            .tooltip(back_tooltip.to_string())
            .on_click(move |_, _, cx| {
                back_handle.update(cx, close);
            })
    } else {
        Button::new(format!("{id_prefix}-close"))
            .ghost()
            .small()
            .icon(IconName::CircleX)
            .tooltip("Close details")
            .on_click(move |_, _, cx| {
                close_handle.update(cx, close);
            })
    };

    h_flex()
        .justify_between()
        .items_start()
        .gap_4()
        .py_3()
        .px_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().w_full().min_w_0().child(selectable_text(
                    format!("{id_prefix}-title"),
                    title.to_string(),
                    SelectableTextStyle::sm().weight(FontWeight::MEDIUM),
                )))
                .child(div().w_full().min_w_0().child(selectable_text(
                    format!("{id_prefix}-subtitle"),
                    subtitle.to_string(),
                    SelectableTextStyle::xs(),
                ))),
        )
        .child(close_button)
        .into_any_element()
}

pub(crate) fn loading_body(title: &str, description: &str, cx: &App) -> AnyElement {
    v_flex()
        .p_4()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(description.to_string()),
        )
        .into_any_element()
}

pub(crate) fn error_body(title: &str, message: &str, cx: &App) -> AnyElement {
    v_flex()
        .p_4()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().danger)
                .child(title.to_string()),
        )
        .child(div().w_full().min_w_0().child(selectable_text(
            "detail-error-message".to_string(),
            message.to_string(),
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        )))
        .into_any_element()
}

pub(crate) fn section<T: 'static>(
    title: &str,
    body: impl IntoElement,
    cx: &mut Context<T>,
) -> AnyElement {
    section_with_app(title, body, cx)
}

pub(crate) fn section_with_app(title: &str, body: impl IntoElement, cx: &App) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(div().w_full().min_w_0().child(body))
        .into_any_element()
}
