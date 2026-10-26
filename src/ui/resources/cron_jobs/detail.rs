//! CronJob detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    count, fields_section, json_section, path, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::CronJobs,
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
                        ("Schedule", text(spec.and_then(|spec| spec.get("schedule")))),
                        ("Suspend", text(spec.and_then(|spec| spec.get("suspend")))),
                        (
                            "Time zone",
                            text(spec.and_then(|spec| spec.get("timeZone"))),
                        ),
                        (
                            "Concurrency policy",
                            text(spec.and_then(|spec| spec.get("concurrencyPolicy"))),
                        ),
                        (
                            "Starting deadline",
                            text(spec.and_then(|spec| spec.get("startingDeadlineSeconds"))),
                        ),
                        (
                            "Successful history limit",
                            text(spec.and_then(|spec| spec.get("successfulJobsHistoryLimit"))),
                        ),
                        (
                            "Failed history limit",
                            text(spec.and_then(|spec| spec.get("failedJobsHistoryLimit"))),
                        ),
                        (
                            "Last schedule time",
                            text(status.and_then(|status| status.get("lastScheduleTime"))),
                        ),
                        (
                            "Last successful time",
                            text(status.and_then(|status| status.get("lastSuccessfulTime"))),
                        ),
                        (
                            "Active jobs",
                            count(status.and_then(|status| status.get("active"))).to_string(),
                        ),
                    ],
                    cx,
                ),
                json_section(
                    "Job template",
                    "jobTemplate",
                    spec.and_then(|spec| spec.get("jobTemplate")),
                    cx,
                ),
                json_section(
                    "Active jobs",
                    "active",
                    status.and_then(|status| status.get("active")),
                    cx,
                ),
            ]
        },
    )
}
