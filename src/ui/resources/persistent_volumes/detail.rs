//! PersistentVolume detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    conditions_section, fields_section, json_section, path, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::PersistentVolumes,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let value = &detail.value;
            let spec = path(value, &["spec"]);
            let status = path(value, &["status"]);
            vec![
                fields_section(
                    "Overview",
                    vec![
                        (
                            "Capacity",
                            text(path(value, &["spec", "capacity", "storage"])),
                        ),
                        (
                            "Access modes",
                            text(spec.and_then(|spec| spec.get("accessModes"))),
                        ),
                        (
                            "Reclaim policy",
                            text(spec.and_then(|spec| spec.get("persistentVolumeReclaimPolicy"))),
                        ),
                        (
                            "Storage class",
                            text(spec.and_then(|spec| spec.get("storageClassName"))),
                        ),
                        (
                            "Volume mode",
                            text(spec.and_then(|spec| spec.get("volumeMode"))),
                        ),
                        ("Phase", text(status.and_then(|status| status.get("phase")))),
                        (
                            "Mount options",
                            text(spec.and_then(|spec| spec.get("mountOptions"))),
                        ),
                    ],
                    cx,
                ),
                json_section(
                    "Claim reference",
                    "claimRef",
                    spec.and_then(|spec| spec.get("claimRef")),
                    cx,
                ),
                json_section("Spec", "spec", spec, cx),
                json_section(
                    "Node affinity",
                    "nodeAffinity",
                    spec.and_then(|spec| spec.get("nodeAffinity")),
                    cx,
                ),
                conditions_section(status.and_then(|status| status.get("conditions")), cx),
            ]
        },
    )
}
