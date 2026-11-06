//! Live-watch indicator for resource list headers.

use gpui_kit::base::h_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, div, px};

use crate::models::{ResourceKind, WatchMode};

pub(crate) fn render(mode: WatchMode, _kind: ResourceKind, cx: &App) -> AnyElement {
    let (color, label) = match mode {
        WatchMode::Watch => (cx.theme().success, "Live"),
    };

    h_flex()
        .gap_1()
        .items_center()
        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(color))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .into_any_element()
}
