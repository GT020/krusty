//! Key/value section for labels, annotations, and maps.

use std::collections::BTreeMap;

use gpui_kit::base::v_flex;
use gpui_kit::{AnyElement, App, ParentElement as _};

use super::empty::empty_row;
use super::fields::field_with_id;
use super::frame::section_with_app;

pub(crate) fn key_values(title: &str, values: &BTreeMap<String, String>, cx: &App) -> AnyElement {
    let mut body = v_flex();
    if values.is_empty() {
        body = body.child(empty_row("None", cx));
    }
    for (key, value) in values {
        body = body.child(field_with_id(
            format!("detail-{title}-{key}"),
            key,
            value,
            cx,
        ));
    }
    section_with_app(title, body, cx)
}
