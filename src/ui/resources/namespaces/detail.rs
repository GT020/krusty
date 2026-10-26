//! Namespace detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    fields_section, json_section, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::Namespaces,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
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
        },
    )
}
