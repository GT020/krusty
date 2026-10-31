//! Label/value rows and paired values.

use gpui_kit::base::v_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, Hsla, IntoElement, ParentElement as _, Styled as _, div};

use super::text::{SelectableTextStyle, selectable_text};

pub(crate) fn field(label: &str, value: &str, cx: &App) -> AnyElement {
    field_with_id(format!("detail-field-{label}"), label, value, cx)
}

pub(crate) fn field_with_id(id: String, label: &str, value: &str, cx: &App) -> AnyElement {
    field_row(id, label, value, None, cx)
}

pub(crate) fn field_colored(label: &str, value: &str, color: Hsla, cx: &App) -> AnyElement {
    field_row(
        format!("detail-field-{label}"),
        label,
        value,
        Some(color),
        cx,
    )
}

fn field_row(id: String, label: &str, value: &str, color: Option<Hsla>, cx: &App) -> AnyElement {
    let label = selectable_text(
        format!("{id}-label"),
        label.to_string(),
        SelectableTextStyle::xs().color(cx.theme().muted_foreground),
    );
    let value_style = match color {
        Some(color) => SelectableTextStyle::xs().color(color),
        None => SelectableTextStyle::xs(),
    };
    let value = selectable_text(format!("{id}-value"), value.to_string(), value_style);
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .py_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(div().w_full().min_w_0().child(label))
        .child(div().w_full().min_w_0().child(value))
        .into_any_element()
}

pub(crate) fn value_pair(label: &str, value: &str, cx: &App) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(div().w_full().min_w_0().child(selectable_text(
            format!("detail-value-{label}-label"),
            label.to_string(),
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        )))
        .child(div().w_full().min_w_0().child(selectable_text(
            format!("detail-value-{label}-value"),
            value.to_string(),
            SelectableTextStyle::xs(),
        )))
        .into_any_element()
}
