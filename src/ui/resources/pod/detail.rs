//! Pod detail pane: describe-like sections plus live usage charts.

use std::collections::BTreeMap;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement as _, Styled as _, div,
};

use crate::models::resources::pod::detail::{
    PodContainer, PodDetail, PodDetailState, UsageHistory,
};
use crate::ui::components::detail::{
    age, detail_header, detail_shell, empty_row, error_body, field, key_values, loading_body,
    raw_manifest, section, usage_section,
};
use crate::ui::resources::pod::PodsView;

pub fn render_detail(
    state: PodDetailState,
    usage: UsageHistory,
    usage_error: Option<String>,
    view: Entity<PodsView>,
    compact: bool,
    cx: &mut Context<PodsView>,
) -> AnyElement {
    let (title, subtitle) = match &state {
        PodDetailState::Loading { name, .. } => (name.clone(), String::new()),
        PodDetailState::Error { name, .. } => (name.clone(), String::new()),
        PodDetailState::Ready(detail) => (
            detail.name.clone(),
            format!(
                "{} · {}/{} ready · {} restarts · {}",
                detail.status_label(),
                detail.ready_containers(),
                detail.containers.len(),
                detail.restart_total(),
                age(detail.created_at)
            ),
        ),
        PodDetailState::Closed => return div().into_any_element(),
    };
    let header = detail_header(
        &title,
        &subtitle,
        compact,
        "Back to pods",
        "pod-detail",
        view,
        PodsView::close_detail,
        cx,
    );

    let body: AnyElement = match state {
        PodDetailState::Loading { .. } => loading_body(
            "Loading pod…",
            "Reading the pod, its containers, and metrics.",
            cx,
        ),
        PodDetailState::Error { message, .. } => {
            error_body("Could not load this pod", &message, cx)
        }
        PodDetailState::Ready(detail) => {
            let mut body = v_flex()
                .gap_6()
                .p_4()
                .child(overview(&detail, cx))
                .child(usage_section(&usage, usage_error.as_deref(), "", cx))
                .child(containers("Containers", &detail.containers, cx))
                .child(conditions(&detail, cx));
            if !detail.init_containers.is_empty() {
                body = body.child(containers("Init containers", &detail.init_containers, cx));
            }
            body.child(volumes(&detail, cx))
                .child(tolerations(&detail, cx))
                .child(owners(&detail, cx))
                .child(key_values("Labels", &detail.labels, cx))
                .child(key_values("Annotations", &detail.annotations, cx))
                .child(raw_manifest(&detail.raw_json, cx))
                .into_any_element()
        }
        PodDetailState::Closed => return div().into_any_element(),
    };

    detail_shell(header, body, compact, cx)
}

fn overview(detail: &PodDetail, cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    body = body
        .child(field("Name", &detail.name, cx))
        .child(field("Namespace", &detail.namespace, cx))
        .child(field("Status", detail.status_label(), cx))
        .child(field("Node", dash(&detail.node_name), cx))
        .child(field("QoS class", dash(&detail.qos_class), cx))
        .child(field("Restart policy", dash(&detail.restart_policy), cx))
        .child(field("Service account", dash(&detail.service_account), cx))
        .child(field("Pod IP", dash(&detail.pod_ip), cx));
    if detail.pod_ips.len() > 1 {
        body = body.child(field("Pod IPs", &detail.pod_ips.join(", "), cx));
    }
    body = body
        .child(field("Host IP", dash(&detail.host_ip), cx))
        .child(field(
            "Host network",
            if detail.host_network { "Yes" } else { "No" },
            cx,
        ))
        .child(field("Priority class", dash(&detail.priority_class), cx))
        .child(field(
            "Priority",
            &detail
                .priority
                .map(|priority| priority.to_string())
                .unwrap_or_else(|| "—".to_string()),
            cx,
        ))
        .child(field("DNS policy", dash(&detail.dns_policy), cx))
        .child(field("Hostname", dash(&detail.hostname), cx))
        .child(field("Subdomain", dash(&detail.subdomain), cx))
        .child(field(
            "Nominated node",
            dash(&detail.nominated_node_name),
            cx,
        ))
        .child(field("Started", &age(detail.start_time), cx))
        .child(field("Age", &age(detail.created_at), cx))
        .child(field("UID", &detail.uid, cx))
        .child(field("Resource version", &detail.resource_version, cx));
    section("Overview", body, cx)
}

fn containers(title: &str, containers: &[PodContainer], cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    if containers.is_empty() {
        body = body.child(empty_row("No containers reported", cx));
    }
    for container in containers {
        let status_color = if container.ready {
            cx.theme().success
        } else if container.state.starts_with("Terminated") {
            cx.theme().danger
        } else if container.state.starts_with("Waiting") {
            cx.theme().warning
        } else {
            cx.theme().muted_foreground
        };
        let heading = format!(
            "{} · {} · {} · {} restarts",
            container.name,
            container.state,
            if container.ready {
                "ready"
            } else {
                "not ready"
            },
            container.restart_count
        );
        let mut block = v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(status_color)
                    .whitespace_normal()
                    .child(heading),
            );
        if !container.state_detail.is_empty() {
            block = block.child(
                div()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(cx.theme().muted_foreground)
                    .child(container.state_detail.clone()),
            );
        }
        if !container.last_state.is_empty() {
            block = block.child(field("Last state", &container.last_state, cx));
        }
        block = block
            .child(field("Image", dash(&container.image), cx))
            .child(field(
                "Image pull policy",
                dash(&container.image_pull_policy),
                cx,
            ))
            .child(field("Container ID", dash(&container.container_id), cx))
            .child(field("Image ID", dash(&container.image_id), cx));
        if !container.command.is_empty() {
            block = block.child(field("Command", &container.command.join(" "), cx));
        }
        if !container.args.is_empty() {
            block = block.child(field("Args", &container.args.join(" "), cx));
        }
        if !container.ports.is_empty() {
            block = block.child(field("Ports", &container.ports.join(", "), cx));
        }
        if !container.env.is_empty() {
            block = block.child(field("Env", &container.env.join("\n"), cx));
        }
        if !container.mounts.is_empty() {
            block = block.child(field("Mounts", &container.mounts.join("\n"), cx));
        }
        block = block
            .child(field("Requests", &quantity_map(&container.requests), cx))
            .child(field("Limits", &quantity_map(&container.limits), cx));
        body = body.child(block);
    }
    section(title, body, cx)
}

fn quantity_map(values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        "—".to_string()
    } else {
        values
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn conditions(detail: &PodDetail, cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    if detail.conditions.is_empty() {
        body = body.child(empty_row("No conditions reported", cx));
    }
    for condition in &detail.conditions {
        let status_color = if condition.status == "True" {
            cx.theme().success
        } else if condition.status == "Unknown" {
            cx.theme().warning
        } else {
            cx.theme().muted_foreground
        };
        let mut block = v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child(condition.type_.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(status_color)
                            .child(condition.status.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Probed {} · Changed {}",
                                age(condition.last_probe),
                                age(condition.last_transition)
                            )),
                    ),
            );
        if !condition.reason.is_empty() {
            block = block.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(condition.reason.clone()),
            );
        }
        if !condition.message.is_empty() {
            block = block.child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .whitespace_normal()
                    .child(condition.message.clone()),
            );
        }
        body = body.child(block);
    }
    section("Conditions", body, cx)
}

fn volumes(detail: &PodDetail, cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    if detail.volumes.is_empty() {
        body = body.child(empty_row("No volumes", cx));
    }
    for volume in &detail.volumes {
        let label = if volume.type_.is_empty() {
            volume.name.clone()
        } else {
            format!("{} · {}", volume.type_, volume.name)
        };
        body = body.child(field(&label, dash(&volume.detail), cx));
    }
    section("Volumes", body, cx)
}

fn tolerations(detail: &PodDetail, cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    if detail.tolerations.is_empty() {
        body = body.child(empty_row("No tolerations", cx));
    }
    for toleration in &detail.tolerations {
        let label = if toleration.effect.is_empty() {
            toleration.key.clone()
        } else {
            format!("{} · {}", toleration.key, toleration.effect)
        };
        let mut value = format!("{} {}", toleration.operator, toleration.value);
        if let Some(seconds) = toleration.seconds {
            value.push_str(&format!(" · {seconds}s"));
        }
        body = body.child(field(&label, value.trim(), cx));
    }
    section("Tolerations", body, cx)
}

fn owners(detail: &PodDetail, cx: &mut Context<PodsView>) -> AnyElement {
    let mut body = v_flex();
    if detail.owners.is_empty() {
        body = body.child(empty_row("No owner references", cx));
    }
    for owner in &detail.owners {
        let label = if owner.controller {
            format!("{} · controller", owner.kind)
        } else {
            owner.kind.clone()
        };
        body = body.child(field(&label, &owner.name, cx));
    }
    section("Owners", body, cx)
}

fn dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}
