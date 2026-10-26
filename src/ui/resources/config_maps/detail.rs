//! ConfigMap detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    count, fields_section, map_section, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::ConfigMaps,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let value = &detail.value;
            vec![
                fields_section(
                    "Overview",
                    vec![
                        ("Immutable", text(value.get("immutable"))),
                        ("Data keys", count(value.get("data")).to_string()),
                        (
                            "Binary data keys",
                            count(value.get("binaryData")).to_string(),
                        ),
                    ],
                    cx,
                ),
                map_section("Data", value.get("data"), cx),
                map_section("Binary data", value.get("binaryData"), cx),
            ]
        },
    )
}
