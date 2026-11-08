//! Secret detail view.

use gpui_kit::{AnyElement, Context};

use crate::models::resources::detail::ResourceDetail;
use crate::ui::resources::detail_common::{count, fields_section, map_section, text};

pub fn sections<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> Vec<AnyElement> {
    let value = &detail.value;
    vec![
        fields_section(
            "Overview",
            vec![
                ("Type", text(value.get("type"))),
                ("Immutable", text(value.get("immutable"))),
                ("Data keys", count(value.get("data")).to_string()),
                (
                    "String data keys",
                    count(value.get("stringData")).to_string(),
                ),
            ],
            cx,
        ),
        map_section("Data", value.get("data"), cx),
        map_section("String data", value.get("stringData"), cx),
    ]
}
