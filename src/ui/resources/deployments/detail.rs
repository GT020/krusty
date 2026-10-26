//! Deployment detail view.

use serde_json::Value;

use gpui_kit::assets::IconName;
use gpui_kit::base::h_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{AnyElement, Context, Entity, IntoElement, ParentElement as _, Styled as _, div};

use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::resources::detail_common::{
    conditions_section, count, fields_section, json_section, map_section, path,
    render_resource_detail, text,
};
use crate::view_models::resources::deployments::detail::{
    ActionFeedback, ActionStatus, DeploymentsDetailViewModel,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    actions: Entity<DeploymentsDetailViewModel>,
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
            let mut sections = Vec::new();
            if let Some(feedback) = actions.read(cx).feedback().cloned() {
                sections.push(feedback_banner(&feedback, cx));
            }
            sections.push(action_bar(actions.clone(), detail, cx));
            sections.push(fields_section(
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
            ));
            sections.push(json_section(
                "Selector",
                "selector",
                spec.and_then(|spec| spec.get("selector")),
                cx,
            ));
            sections.push(json_section(
                "Strategy",
                "strategy",
                spec.and_then(|spec| spec.get("strategy")),
                cx,
            ));
            sections.push(conditions_section(
                status.and_then(|status| status.get("conditions")),
                cx,
            ));
            sections.extend(pod_template_sections(
                path(value, &["spec", "template", "spec"]),
                cx,
            ));
            sections
        },
    )
}

fn pod_template_sections<T: 'static>(
    pod_spec: Option<&Value>,
    cx: &mut Context<T>,
) -> Vec<AnyElement> {
    let Some(pod_spec) = pod_spec else {
        return Vec::new();
    };
    let mut sections = vec![fields_section(
        "Pod template",
        vec![
            ("Node", text(pod_spec.get("nodeName"))),
            ("Service account", text(pod_spec.get("serviceAccountName"))),
            ("Restart policy", text(pod_spec.get("restartPolicy"))),
            ("DNS policy", text(pod_spec.get("dnsPolicy"))),
            ("Host network", text(pod_spec.get("hostNetwork"))),
            ("Priority class", text(pod_spec.get("priorityClassName"))),
        ],
        cx,
    )];
    if count(pod_spec.get("volumes")) > 0 {
        sections.push(json_section(
            "Volumes",
            "volumes",
            pod_spec.get("volumes"),
            cx,
        ));
    }
    if count(pod_spec.get("tolerations")) > 0 {
        sections.push(json_section(
            "Tolerations",
            "tolerations",
            pod_spec.get("tolerations"),
            cx,
        ));
    }
    if let Some(selector) = pod_spec.get("nodeSelector") {
        sections.push(map_section("Node selector", Some(selector), cx));
    }
    sections
}

fn action_bar<T: 'static>(
    actions: Entity<DeploymentsDetailViewModel>,
    detail: &ResourceDetail,
    cx: &mut Context<T>,
) -> AnyElement {
    let namespace = detail.namespace.clone();
    let name = detail.name.clone();
    let replicas = detail
        .value
        .pointer("/spec/replicas")
        .and_then(Value::as_i64)
        .unwrap_or(1) as i32;
    let manifest = detail.value.clone();

    let scale = actions.clone();
    let scale_namespace = namespace.clone();
    let scale_name = name.clone();
    let restart = actions.clone();
    let restart_namespace = namespace.clone();
    let restart_name = name.clone();
    let edit = actions.clone();
    let edit_namespace = namespace.clone();
    let edit_name = name.clone();
    let delete = actions.clone();
    let delete_namespace = namespace;
    let delete_name = name;

    h_flex()
        .gap_1()
        .items_center()
        .child(
            Button::new("deployment-scale")
                .ghost()
                .small()
                .icon(IconName::Scaling)
                .tooltip("Scale deployment")
                .on_click(move |_, window, cx| {
                    scale.update(cx, |this, cx| {
                        this.scale(
                            scale_namespace.clone(),
                            scale_name.clone(),
                            replicas,
                            window,
                            cx,
                        )
                    });
                }),
        )
        .child(
            Button::new("deployment-restart")
                .ghost()
                .small()
                .icon(IconName::RotateCcw)
                .tooltip("Restart deployment")
                .on_click(move |_, _, cx| {
                    restart.update(cx, |this, cx| {
                        this.restart(restart_namespace.clone(), restart_name.clone(), cx)
                    });
                }),
        )
        .child(
            Button::new("deployment-edit")
                .ghost()
                .small()
                .icon(IconName::Pencil)
                .tooltip("Edit deployment")
                .on_click(move |_, window, cx| {
                    edit.update(cx, |this, cx| {
                        this.edit(
                            edit_namespace.clone(),
                            edit_name.clone(),
                            &manifest,
                            window,
                            cx,
                        )
                    });
                }),
        )
        .child(
            Button::new("deployment-delete")
                .ghost()
                .small()
                .text_color(cx.theme().danger)
                .icon(IconName::Trash)
                .tooltip("Delete deployment")
                .on_click(move |_, window, cx| {
                    delete.update(cx, |this, cx| {
                        this.delete(delete_namespace.clone(), delete_name.clone(), window, cx)
                    });
                }),
        )
        .into_any_element()
}

fn feedback_banner<T: 'static>(feedback: &ActionFeedback, cx: &mut Context<T>) -> AnyElement {
    let color = match feedback.status() {
        ActionStatus::Pending => cx.theme().muted_foreground,
        ActionStatus::Success => cx.theme().success,
        ActionStatus::Error => cx.theme().danger,
    };
    div()
        .w_full()
        .min_w_0()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(color)
        .px_3()
        .py_2()
        .text_xs()
        .whitespace_normal()
        .text_color(color)
        .child(feedback.message().to_string())
        .into_any_element()
}
