//! Banner for pending, successful, and failed actions.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, App, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    div,
};

use crate::ui::components::detail::{SelectableTextStyle, selectable_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeedbackTone {
    Pending,
    Success,
    Error,
}

impl FeedbackTone {
    fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Pending => cx.theme().muted_foreground,
            Self::Success => cx.theme().success,
            Self::Error => cx.theme().danger,
        }
    }
}

pub(crate) struct FeedbackView<'a> {
    pub tone: FeedbackTone,
    pub message: &'a str,
}

pub(crate) fn feedback_banner(id: &str, view: FeedbackView<'_>, cx: &App) -> AnyElement {
    let color = view.tone.color(cx);
    div()
        .id(gpui_kit::SharedString::from(id.to_string()))
        .w_full()
        .min_w_0()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(color)
        .px_3()
        .py_2()
        .text_xs()
        .whitespace_normal()
        .child(selectable_text(
            format!("{id}-message"),
            view.message.to_string(),
            SelectableTextStyle::xs().color(color),
        ))
        .into_any_element()
}
