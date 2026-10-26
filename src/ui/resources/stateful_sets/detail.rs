//! StatefulSet detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    conditions_section, fields_section, json_section, path, pod_template_sections,
    render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::StatefulSets,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let value = &detail.value;
            let spec = path(value, &["spec"]);
            let status = path(value, &["status"]);
            let mut sections = vec![
                fields_section(
                    "Overview",
                    vec![
                        ("Replicas", text(spec.and_then(|spec| spec.get("replicas")))),
                        (
                            "Ready",
                            text(status.and_then(|status| status.get("readyReplicas"))),
                        ),
                        (
                            "Current",
                            text(status.and_then(|status| status.get("currentReplicas"))),
                        ),
                        (
                            "Updated",
                            text(status.and_then(|status| status.get("updatedReplicas"))),
                        ),
                        (
                            "Available",
                            text(status.and_then(|status| status.get("availableReplicas"))),
                        ),
                        (
                            "Current revision",
                            text(status.and_then(|status| status.get("currentRevision"))),
                        ),
                        (
                            "Update revision",
                            text(status.and_then(|status| status.get("updateRevision"))),
                        ),
                        (
                            "Collision count",
                            text(status.and_then(|status| status.get("collisionCount"))),
                        ),
                        (
                            "Observed generation",
                            text(status.and_then(|status| status.get("observedGeneration"))),
                        ),
                        (
                            "Service name",
                            text(spec.and_then(|spec| spec.get("serviceName"))),
                        ),
                        (
                            "Pod management policy",
                            text(spec.and_then(|spec| spec.get("podManagementPolicy"))),
                        ),
                        (
                            "Min ready seconds",
                            text(spec.and_then(|spec| spec.get("minReadySeconds"))),
                        ),
                        (
                            "Revision history limit",
                            text(spec.and_then(|spec| spec.get("revisionHistoryLimit"))),
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
                    "Update strategy",
                    "updateStrategy",
                    spec.and_then(|spec| spec.get("updateStrategy")),
                    cx,
                ),
                json_section(
                    "PVC retention policy",
                    "persistentVolumeClaimRetentionPolicy",
                    spec.and_then(|spec| spec.get("persistentVolumeClaimRetentionPolicy")),
                    cx,
                ),
                conditions_section(status.and_then(|status| status.get("conditions")), cx),
            ];
            sections.extend(pod_template_sections(
                path(value, &["spec", "template", "spec"]),
                cx,
            ));
            sections
        },
    )
}
