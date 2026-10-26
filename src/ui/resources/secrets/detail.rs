//! Secret detail view.

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
        ResourceKind::Secrets,
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
        },
    )
}
