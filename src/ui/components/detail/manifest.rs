//! Raw manifest block for detail panes.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, ParentElement as _, Styled as _, div};

use super::frame::section_with_app;
use super::text::{SelectableTextStyle, selectable_text};

pub(crate) fn raw_manifest(json: &str, cx: &App) -> AnyElement {
    section_with_app(
        "Manifest",
        div()
            .w_full()
            .min_w_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .child(
                div().w_full().min_w_0().child(selectable_text(
                    "detail-manifest-text".to_string(),
                    json.to_string(),
                    SelectableTextStyle::xs()
                        .monospace()
                        .color(cx.theme().muted_foreground),
                )),
            ),
        cx,
    )
}
