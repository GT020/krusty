//! One usage meter row: label, values, and progress.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::progress::Progress;
use gpui_kit::{AnyElement, App, Hsla, IntoElement, ParentElement as _, Styled as _, div};

use crate::ui::components::detail::{SelectableTextStyle, selectable_text};

pub(crate) struct MeterView<'a> {
    pub label: &'a str,
    pub percent: Option<f32>,
    pub used: Option<&'a str>,
    pub capacity: Option<&'a str>,
    pub capacity_basis: &'a str,
    pub color: Hsla,
}

pub(crate) fn usage_meter(id: &str, view: MeterView<'_>, cx: &App) -> AnyElement {
    let percent = view.percent.unwrap_or(0.0);
    let used = view.used.unwrap_or("—");
    let capacity = view.capacity.unwrap_or("—");
    let basis = if view.capacity_basis.is_empty() {
        String::new()
    } else {
        format!(" {}", view.capacity_basis)
    };
    v_flex()
        .gap_1()
        .child(
            h_flex()
                .justify_between()
                .gap_2()
                .child(div().flex_shrink_0().child(selectable_text(
                    format!("{id}-label"),
                    view.label.to_string(),
                    SelectableTextStyle::xs(),
                )))
                .child(div().flex_1().min_w_0().text_right().child(selectable_text(
                    format!("{id}-value"),
                    format!("{used} / {capacity}{basis} · {percent:.0}%"),
                    SelectableTextStyle::xs().color(cx.theme().muted_foreground),
                ))),
        )
        .child(
            Progress::new(format!("{id}-progress"))
                .value(percent)
                .color(view.color),
        )
        .into_any_element()
}
