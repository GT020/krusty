//! StorageClass detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    fields_section, json_section, map_section, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::StorageClasses,
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
                        ("Provisioner", text(value.get("provisioner"))),
                        ("Reclaim policy", text(value.get("reclaimPolicy"))),
                        ("Volume binding mode", text(value.get("volumeBindingMode"))),
                        (
                            "Allow volume expansion",
                            text(value.get("allowVolumeExpansion")),
                        ),
                        ("Mount options", text(value.get("mountOptions"))),
                    ],
                    cx,
                ),
                map_section("Parameters", value.get("parameters"), cx),
                json_section(
                    "Allowed topologies",
                    "allowedTopologies",
                    value.get("allowedTopologies"),
                    cx,
                ),
            ]
        },
    )
}
