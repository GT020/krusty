//! DaemonSet detail view.

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
        ResourceKind::DaemonSets,
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
                        (
                            "Desired",
                            text(status.and_then(|status| status.get("desiredNumberScheduled"))),
                        ),
                        (
                            "Current",
                            text(status.and_then(|status| status.get("currentNumberScheduled"))),
                        ),
                        (
                            "Ready",
                            text(status.and_then(|status| status.get("numberReady"))),
                        ),
                        (
                            "Available",
                            text(status.and_then(|status| status.get("numberAvailable"))),
                        ),
                        (
                            "Updated",
                            text(status.and_then(|status| status.get("updatedNumberScheduled"))),
                        ),
                        (
                            "Misscheduled",
                            text(status.and_then(|status| status.get("numberMisscheduled"))),
                        ),
                        (
                            "Unavailable",
                            text(status.and_then(|status| status.get("numberUnavailable"))),
                        ),
                        (
                            "Observed generation",
                            text(status.and_then(|status| status.get("observedGeneration"))),
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
