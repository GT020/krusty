//! Empty-state row for detail sections.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, div};

pub(crate) fn empty_row(message: &str, cx: &App) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .py_1()
        .text_xs()
        .whitespace_normal()
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}
