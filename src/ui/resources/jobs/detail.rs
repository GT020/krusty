//! Job detail view.

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
        ResourceKind::Jobs,
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
                            "Completions",
                            text(spec.and_then(|spec| spec.get("completions"))),
                        ),
                        (
                            "Parallelism",
                            text(spec.and_then(|spec| spec.get("parallelism"))),
                        ),
                        (
                            "Backoff limit",
                            text(spec.and_then(|spec| spec.get("backoffLimit"))),
                        ),
                        (
                            "Active",
                            text(status.and_then(|status| status.get("active"))),
                        ),
                        (
                            "Succeeded",
                            text(status.and_then(|status| status.get("succeeded"))),
                        ),
                        (
                            "Failed",
                            text(status.and_then(|status| status.get("failed"))),
                        ),
                        (
                            "Start time",
                            text(status.and_then(|status| status.get("startTime"))),
                        ),
                        (
                            "Completion time",
                            text(status.and_then(|status| status.get("completionTime"))),
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
                    "Pod failure policy",
                    "podFailurePolicy",
                    spec.and_then(|spec| spec.get("podFailurePolicy")),
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
