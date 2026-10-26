//! Deployment detail view.

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
        ResourceKind::Deployments,
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
                            "Updated",
                            text(status.and_then(|status| status.get("updatedReplicas"))),
                        ),
                        (
                            "Available",
                            text(status.and_then(|status| status.get("availableReplicas"))),
                        ),
                        (
                            "Unavailable",
                            text(status.and_then(|status| status.get("unavailableReplicas"))),
                        ),
                        (
                            "Observed generation",
                            text(status.and_then(|status| status.get("observedGeneration"))),
                        ),
                        (
                            "Min ready seconds",
                            text(spec.and_then(|spec| spec.get("minReadySeconds"))),
                        ),
                        (
                            "Revision history limit",
                            text(spec.and_then(|spec| spec.get("revisionHistoryLimit"))),
                        ),
                        (
                            "Progress deadline",
                            text(spec.and_then(|spec| spec.get("progressDeadlineSeconds"))),
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
                    "Strategy",
                    "strategy",
                    spec.and_then(|spec| spec.get("strategy")),
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
