//! Selectable text runs used by detail panes.

use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::base::SelectableText;
use gpui_kit::{
    AbsoluteLength, AnyElement, FontWeight, Hsla, IntoElement, SharedString, TextStyleRefinement,
    WhiteSpace, rems,
};

/// Visual treatment for [`SelectableText`] runs in detail panes.
#[derive(Clone, Copy)]
pub(crate) struct SelectableTextStyle {
    font_size: AbsoluteLength,
    color: Option<Hsla>,
    font_weight: Option<FontWeight>,
    monospace: bool,
}

impl SelectableTextStyle {
    pub(crate) fn xs() -> Self {
        Self {
            font_size: rems(0.75).into(),
            color: None,
            font_weight: None,
            monospace: false,
        }
    }

    pub(crate) fn sm() -> Self {
        Self {
            font_size: rems(0.875).into(),
            ..Self::xs()
        }
    }

    pub(crate) fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub(crate) fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub(crate) fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }
}

pub(crate) fn selectable_text(
    id: String,
    text: impl Into<SharedString>,
    style: SelectableTextStyle,
) -> AnyElement {
    // Detail text is rendered in reading order, so a process-wide sequence keeps
    // runs in the order they appear on screen for drag selection across fields.
    static NEXT_DOCUMENT_ORDER: AtomicU64 = AtomicU64::new(0);
    let refinement = TextStyleRefinement {
        color: style.color,
        font_family: style.monospace.then(|| "monospace".into()),
        font_size: Some(style.font_size),
        font_weight: style.font_weight,
        white_space: Some(WhiteSpace::Normal),
        ..Default::default()
    };
    SelectableText::new(id, text)
        .document_order(NEXT_DOCUMENT_ORDER.fetch_add(1, Ordering::Relaxed))
        .text_style(refinement)
        .into_any_element()
}
