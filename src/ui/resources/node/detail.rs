//! Node detail pane: describe-like sections plus live usage charts.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement as _, Styled as _, div,
};

use crate::models::resources::node::detail::{NodeDetail, NodeDetailState, UsageHistory};
use crate::ui::components::detail::{
    age, detail_header, detail_shell, empty_row, error_body, field, key_values, loading_body,
    raw_manifest, section, usage_section, value_pair,
};
use crate::ui::resources::node::NodesView;

pub fn render_detail(
    state: NodeDetailState,
    usage: UsageHistory,
    usage_error: Option<String>,
    view: Entity<NodesView>,
    compact: bool,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let (title, subtitle) = match &state {
        NodeDetailState::Loading { node } => (node.clone(), String::new()),
        NodeDetailState::Error { node, .. } => (node.clone(), String::new()),
        NodeDetailState::Ready(detail) => (
            detail.name.clone(),
            format!(
                "{} · {} · {}",
                detail.status_label(),
                detail.roles_label(),
                age(detail.created_at)
            ),
        ),
        NodeDetailState::Closed => return div().into_any_element(),
    };
    let header = detail_header(
        &title,
        &subtitle,
        compact,
        "Back to nodes",
        "node-detail",
        view,
        NodesView::close_detail,
        cx,
    );

    let body: AnyElement = match state {
        NodeDetailState::Loading { .. } => loading_body(
            "Loading node…",
            "Reading the node, its conditions, and metrics.",
            cx,
        ),
        NodeDetailState::Error { node: _, message } => {
            error_body("Could not load this node", &message, cx)
        }
        NodeDetailState::Ready(detail) => v_flex()
            .gap_6()
            .p_4()
            .child(overview(&detail, cx))
            .child(usage_section(
                &usage,
                usage_error.as_deref(),
                "allocatable",
                cx,
            ))
            .child(capacity(&detail, cx))
            .child(conditions(&detail, cx))
            .child(system(&detail, cx))
            .child(addresses(&detail, cx))
            .child(taints(&detail, cx))
            .child(key_values("Labels", &detail.labels, cx))
            .child(key_values("Annotations", &detail.annotations, cx))
            .child(images(&detail, cx))
            .child(raw_manifest(&detail.raw_json, cx))
            .into_any_element(),
        NodeDetailState::Closed => return div().into_any_element(),
    };

    detail_shell(header, body, compact, cx)
}

fn overview(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    body = body
        .child(field("Name", &detail.name, cx))
        .child(field("Status", detail.status_label(), cx))
        .child(field("Roles", &detail.roles_label(), cx))
        .child(field("Age", &age(detail.created_at), cx))
        .child(field("UID", &detail.uid, cx))
        .child(field("Resource version", &detail.resource_version, cx))
        .child(field(
            "Unschedulable",
            if detail.unschedulable { "Yes" } else { "No" },
            cx,
        ))
        .child(field("Provider ID", dash(&detail.provider_id), cx))
        .child(field("Pod CIDR", dash(&detail.pod_cidr), cx));
    if detail.pod_cidrs.len() > 1 {
        body = body.child(field("Pod CIDRs", &detail.pod_cidrs.join(", "), cx));
    }
    section("Overview", body, cx)
}

fn capacity(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut keys: Vec<String> = detail
        .capacity
        .keys()
        .chain(detail.allocatable.keys())
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();

    let mut body = v_flex();
    if keys.is_empty() {
        body = body.child(empty_row("No capacity reported", cx));
    }
    for key in keys {
        let capacity = detail.capacity.get(&key).map(String::as_str).unwrap_or("—");
        let allocatable = detail
            .allocatable
            .get(&key)
            .map(String::as_str)
            .unwrap_or("—");
        body = body.child(
            v_flex()
                .w_full()
                .min_w_0()
                .gap_1()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().text_xs().font_weight(FontWeight::MEDIUM).child(key))
                .child(
                    h_flex()
                        .gap_4()
                        .items_start()
                        .child(value_pair("Capacity", capacity, cx))
                        .child(value_pair("Allocatable", allocatable, cx)),
                ),
        );
    }
    section("Capacity and allocatable", body, cx)
}

fn conditions(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    if detail.conditions.is_empty() {
        body = body.child(empty_row("No conditions reported", cx));
    }
    for condition in &detail.conditions {
        let status_color = if condition.status == "True" {
            cx.theme().success
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
                                "Heartbeat {} · Changed {}",
                                age(condition.last_heartbeat),
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

fn system(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let info = &detail.node_info;
    let body = v_flex()
        .child(field("Operating system", dash(&info.operating_system), cx))
        .child(field("Architecture", dash(&info.architecture), cx))
        .child(field("Kernel version", dash(&info.kernel_version), cx))
        .child(field("OS image", dash(&info.os_image), cx))
        .child(field(
            "Container runtime",
            dash(&info.container_runtime_version),
            cx,
        ))
        .child(field("Kubelet version", dash(&info.kubelet_version), cx))
        .child(field(
            "Kube-proxy version",
            dash(&info.kube_proxy_version),
            cx,
        ))
        .child(field("Machine ID", dash(&info.machine_id), cx))
        .child(field("System UUID", dash(&info.system_uuid), cx))
        .child(field("Boot ID", dash(&info.boot_id), cx));
    section("System", body, cx)
}

fn addresses(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    if detail.addresses.is_empty() {
        body = body.child(empty_row("No addresses reported", cx));
    }
    for address in &detail.addresses {
        body = body.child(field(&address.type_, &address.address, cx));
    }
    section("Addresses", body, cx)
}

fn taints(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    if detail.taints.is_empty() {
        body = body.child(empty_row("No taints", cx));
    }
    for taint in &detail.taints {
        let label = if taint.effect.is_empty() {
            taint.key.clone()
        } else {
            format!("{} · {}", taint.key, taint.effect)
        };
        body = body.child(field(&label, dash(&taint.value), cx));
    }
    section("Taints", body, cx)
}

fn images(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    if detail.images.is_empty() {
        body = body.child(empty_row("No images reported", cx));
    }
    for image in &detail.images {
        let label = format!(
            "Image · {}",
            crate::ui::components::detail::format_bytes(image.size_bytes.max(0) as u64)
        );
        body = body.child(field(&label, &image.names.join(", "), cx));
    }
    section("Images", body, cx)
}

fn dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}
