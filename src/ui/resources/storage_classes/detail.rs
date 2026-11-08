//! StorageClass detail view.

use gpui_kit::{AnyElement, Context};

use crate::models::resources::detail::ResourceDetail;
use crate::ui::resources::detail_common::{fields_section, json_section, map_section, text};

pub fn sections<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> Vec<AnyElement> {
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
}
