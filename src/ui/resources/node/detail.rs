//! Node detail pane: describe-like sections plus live usage charts.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement as _, Styled as _, div,
};

use crate::models::dashboard::{NodeSection, NodeWidgetSource, WidgetSource};
use crate::models::resources::node::detail::{NodeCondition, NodeDetail, NodeDetailState};
use crate::ui::components::detail::{
    ConditionTone, ConditionView, SelectableTextStyle, age, condition_row, detail_header,
    detail_shell, empty_row, error_body, field, field_colored, field_with_id, key_values,
    loading_body, raw_manifest, section, selectable_text, value_pair,
};
use crate::ui::dashboard::pinnable;
use crate::ui::resources::node::NodesView;
use crate::ui::resources::node::usage::NodeUsageWidget;

pub fn render_detail(
    state: NodeDetailState,
    usage: Option<&NodeUsageWidget>,
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
        NodeDetailState::Ready(detail) => {
            let section = |section: NodeSection| {
                WidgetSource::Node(NodeWidgetSource::Section {
                    name: detail.name.clone(),
                    section,
                })
            };
            let id = |part: &str| format!("node-section-{}-{part}", detail.name);

            let mut body = v_flex().gap_6().p_4().child(pinnable(
                id("overview"),
                overview(&detail, cx),
                section(NodeSection::Overview),
            ));
            if let Some(usage) = usage {
                body = body.child(usage.element(cx));
            }
            body.child(pinnable(
                id("capacity"),
                capacity(&detail, cx),
                section(NodeSection::Capacity),
            ))
            .child(pinnable(
                id("conditions"),
                conditions(&detail, cx),
                section(NodeSection::Conditions),
            ))
            .child(pinnable(
                id("system"),
                system(&detail, cx),
                section(NodeSection::System),
            ))
            .child(pinnable(
                id("addresses"),
                addresses(&detail, cx),
                section(NodeSection::Addresses),
            ))
            .child(pinnable(
                id("taints"),
                taints(&detail, cx),
                section(NodeSection::Taints),
            ))
            .child(pinnable(
                id("labels"),
                key_values("Labels", &detail.labels, cx),
                section(NodeSection::Labels),
            ))
            .child(pinnable(
                id("annotations"),
                key_values("Annotations", &detail.annotations, cx),
                section(NodeSection::Annotations),
            ))
            .child(pinnable(
                id("images"),
                images(&detail, cx),
                section(NodeSection::Images),
            ))
            .child(pinnable(
                id("manifest"),
                raw_manifest(&detail.raw_json, cx),
                section(NodeSection::Manifest),
            ))
            .into_any_element()
        }
        NodeDetailState::Closed => return div().into_any_element(),
    };

    detail_shell(header, body, compact, cx)
}

pub(crate) fn overview<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
    let mut body = v_flex();
    body = body
        .child(field("Name", &detail.name, cx))
        .child(field_colored(
            "Status",
            detail.status_label(),
            if detail.is_ready() {
                cx.theme().success
            } else {
                cx.theme().danger
            },
            cx,
        ))
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

pub(crate) fn capacity<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
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
                .child(div().w_full().min_w_0().child(selectable_text(
                    format!("node-capacity-{key}-key"),
                    key.clone(),
                    SelectableTextStyle::xs().weight(FontWeight::MEDIUM),
                )))
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

pub(crate) fn conditions<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
    let mut body = v_flex();
    if detail.conditions.is_empty() {
        body = body.child(empty_row("No conditions reported", cx));
    }
    for condition in &detail.conditions {
        let tone = condition_tone(condition);
        let heartbeat = format!(
            "Heartbeat {} · Changed {}",
            age(condition.last_heartbeat),
            age(condition.last_transition)
        );
        body = body.child(condition_row(
            &format!("node-condition-{}", condition.type_),
            ConditionView {
                kind: &condition.type_,
                status: &condition.status,
                reason: &condition.reason,
                message: &condition.message,
                timestamp: &heartbeat,
                tone,
            },
            cx,
        ));
    }
    section("Conditions", body, cx)
}

fn condition_tone(condition: &NodeCondition) -> ConditionTone {
    let negative = matches!(
        condition.type_.as_str(),
        "MemoryPressure" | "DiskPressure" | "PIDPressure" | "NetworkUnavailable"
    );
    match condition.status.as_str() {
        "True" if negative => ConditionTone::Danger,
        "True" => ConditionTone::Success,
        "False" if condition.type_ == "Ready" => ConditionTone::Danger,
        "False" if negative => ConditionTone::Success,
        "Unknown" => ConditionTone::Warning,
        _ => ConditionTone::Muted,
    }
}

pub(crate) fn system<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
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

pub(crate) fn addresses<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
    let mut body = v_flex();
    if detail.addresses.is_empty() {
        body = body.child(empty_row("No addresses reported", cx));
    }
    for address in &detail.addresses {
        body = body.child(field_with_id(
            format!("node-address-{}", address.type_),
            &address.type_,
            &address.address,
            cx,
        ));
    }
    section("Addresses", body, cx)
}

pub(crate) fn taints<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
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
        body = body.child(field_with_id(
            format!("node-taint-{}-{}", taint.key, taint.effect),
            &label,
            dash(&taint.value),
            cx,
        ));
    }
    section("Taints", body, cx)
}

pub(crate) fn images<T: 'static>(detail: &NodeDetail, cx: &mut Context<T>) -> AnyElement {
    let mut body = v_flex();
    if detail.images.is_empty() {
        body = body.child(empty_row("No images reported", cx));
    }
    for image in &detail.images {
        let label = format!(
            "Image · {}",
            crate::ui::components::detail::format_bytes(image.size_bytes.max(0) as u64)
        );
        let names = image.names.join(", ");
        body = body.child(field_with_id(
            format!("node-image-{}", image.names.join(",")),
            &label,
            &names,
            cx,
        ));
    }
    section("Images", body, cx)
}

fn dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}
