//! PersistentVolumeClaim detail view.

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
        ResourceKind::PersistentVolumeClaims,
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
                        ("Phase", text(status.and_then(|status| status.get("phase")))),
                        ("Volume", text(spec.and_then(|spec| spec.get("volumeName")))),
                        (
                            "Capacity",
                            text(path(value, &["status", "capacity", "storage"])),
                        ),
                        (
                            "Requested storage",
                            text(path(value, &["spec", "resources", "requests", "storage"])),
                        ),
                        (
                            "Access modes",
                            text(spec.and_then(|spec| spec.get("accessModes"))),
                        ),
                        (
                            "Storage class",
                            text(spec.and_then(|spec| spec.get("storageClassName"))),
                        ),
                        (
                            "Volume mode",
                            text(spec.and_then(|spec| spec.get("volumeMode"))),
                        ),
                    ],
                    cx,
                ),
                json_section(
                    "Selector",
                    "selector",
                    spec.and_then(|spec| spec.get("selector")),
                    cx,
                ),
                json_section(
                    "Data source",
                    "dataSource",
                    spec.and_then(|spec| spec.get("dataSource")),
                    cx,
                ),
                json_section(
                    "Data source reference",
                    "dataSourceRef",
                    spec.and_then(|spec| spec.get("dataSourceRef")),
                    cx,
                ),
                conditions_section(status.and_then(|status| status.get("conditions")), cx),
            ]
        },
    )
}
