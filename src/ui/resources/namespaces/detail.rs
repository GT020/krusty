//! Namespace detail view.

use gpui_kit::{AnyElement, Context};

use crate::models::resources::detail::ResourceDetail;
use crate::ui::resources::detail_common::{fields_section, json_section, text};

pub fn sections<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> Vec<AnyElement> {
    let value = &detail.value;
    let status = value.get("status");
    vec![
        fields_section(
            "Overview",
            vec![("Phase", text(status.and_then(|status| status.get("phase"))))],
            cx,
        ),
        json_section("Spec", "spec", value.get("spec"), cx),
    ]
}
