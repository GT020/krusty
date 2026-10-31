//! Reusable condition block for pod, node, and generic details.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, App, FontWeight, Hsla, IntoElement, ParentElement as _, Styled as _, div,
};

use super::text::{SelectableTextStyle, selectable_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConditionTone {
    Success,
    Warning,
    Danger,
    Muted,
}

impl ConditionTone {
    fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Danger => cx.theme().danger,
            Self::Muted => cx.theme().muted_foreground,
        }
    }
}

/// Presentation-ready view of one condition. Resource views map their own
/// condition types into this struct.
pub(crate) struct ConditionView<'a> {
    pub kind: &'a str,
    pub status: &'a str,
    pub reason: &'a str,
    pub message: &'a str,
    pub timestamp: &'a str,
    pub tone: ConditionTone,
}

/// Render one condition row. `id` must be unique within the containing view;
/// child element ids are derived from it.
pub(crate) fn condition_row(id: &str, view: ConditionView<'_>, cx: &App) -> AnyElement {
    let status_color = view.tone.color(cx);
    let mut block = v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(selectable_text(
                    format!("{id}-type"),
                    view.kind.to_string(),
                    SelectableTextStyle::xs().weight(FontWeight::MEDIUM),
                ))
                .child(selectable_text(
                    format!("{id}-status"),
                    view.status.to_string(),
                    SelectableTextStyle::xs().color(status_color),
                ))
                .child(div().flex_1().min_w_0().text_right().child(selectable_text(
                    format!("{id}-changed"),
                    view.timestamp.to_string(),
                    SelectableTextStyle::xs().color(cx.theme().muted_foreground),
                ))),
        );
    if !view.reason.is_empty() && view.reason != "—" {
        block = block.child(div().w_full().min_w_0().child(selectable_text(
            format!("{id}-reason"),
            view.reason.to_string(),
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        )));
    }
    if !view.message.is_empty() && view.message != "—" {
        block = block.child(div().w_full().min_w_0().child(selectable_text(
            format!("{id}-message"),
            view.message.to_string(),
            SelectableTextStyle::xs(),
        )));
    }
    block.into_any_element()
}
