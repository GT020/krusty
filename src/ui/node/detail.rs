//! Node detail pane: describe-like sections plus live usage charts.

use std::collections::BTreeMap;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::chart::AreaChart;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement as _, SharedString,
    Styled as _, div, px,
};

use crate::models::node::detail::{NodeDetail, NodeDetailState, NodeUsage, UsageHistory};
use crate::ui::node::NodesView;

pub fn render_detail(
    state: NodeDetailState,
    usage: UsageHistory,
    usage_error: Option<String>,
    view: Entity<NodesView>,
    compact: bool,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let header = match &state {
        NodeDetailState::Loading { node } => detail_header(node, None, view.clone(), compact, cx),
        NodeDetailState::Ready(detail) => detail_header(
            &detail.name,
            Some(detail.as_ref()),
            view.clone(),
            compact,
            cx,
        ),
        NodeDetailState::Error { node, .. } => detail_header(node, None, view.clone(), compact, cx),
        NodeDetailState::Closed => return div().into_any_element(),
    };

    let body: AnyElement = match state {
        NodeDetailState::Loading { .. } => v_flex()
            .p_4()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Loading node…"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Reading the node, its conditions, and metrics."),
            )
            .into_any_element(),
        NodeDetailState::Error { node: _, message } => v_flex()
            .p_4()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().danger)
                    .child("Could not load this node"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(message),
            )
            .into_any_element(),
        NodeDetailState::Ready(detail) => v_flex()
            .gap_6()
            .p_4()
            .child(overview(&detail, cx))
            .child(usage_section(&usage, usage_error.as_deref(), cx))
            .child(capacity(&detail, cx))
            .child(conditions(&detail, cx))
            .child(system(&detail, cx))
            .child(addresses(&detail, cx))
            .child(taints(&detail, cx))
            .child(key_values("Labels", &detail.labels, cx))
            .child(key_values("Annotations", &detail.annotations, cx))
            .child(images(&detail, cx))
            .child(raw_manifest(&detail, cx))
            .into_any_element(),
        NodeDetailState::Closed => return div().into_any_element(),
    };

    let mut root = v_flex()
        .size_full()
        .min_w_0()
        .bg(cx.theme().background)
        .child(header)
        .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(body));
    if !compact {
        root = root.border_l_1().border_color(cx.theme().border);
    }
    root.into_any_element()
}

fn detail_header(
    node: &str,
    detail: Option<&NodeDetail>,
    view: Entity<NodesView>,
    compact: bool,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let handle = view;
    let close = if compact {
        Button::new("node-detail-back")
            .ghost()
            .small()
            .icon(IconName::ArrowLeft)
            .label("Back")
            .tooltip("Back to nodes")
            .on_click(move |_, _, cx| {
                handle.update(cx, |view, cx| view.close_detail(cx));
            })
    } else {
        Button::new("close-node-detail")
            .ghost()
            .small()
            .icon(IconName::CircleX)
            .tooltip("Close node details")
            .on_click(move |_, _, cx| {
                handle.update(cx, |view, cx| view.close_detail(cx));
            })
    };
    h_flex()
        .justify_between()
        .items_start()
        .gap_4()
        .py_3()
        .px_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(node.to_string()),
                )
                .child(div().text_xs().child(match detail {
                    Some(detail) => format!(
                        "{} · {} · {}",
                        detail.status_label(),
                        detail.roles_label(),
                        age(detail.created_at)
                    ),
                    None => String::new(),
                })),
        )
        .child(close)
        .into_any_element()
}

fn section(title: &str, body: impl IntoElement, cx: &mut Context<NodesView>) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().info)
                .child(title.to_string()),
        )
        .child(div().w_full().min_w_0().child(body))
        .into_any_element()
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

fn usage_section(
    usage: &UsageHistory,
    error: Option<&str>,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let latest = usage.latest();
    let mut body = v_flex().gap_4();

    if let Some(latest) = latest {
        body = body.child(
            v_flex()
                .gap_3()
                .child(usage_bar(
                    "CPU",
                    latest.cpu_percent(),
                    latest
                        .cpu_used_millicores
                        .map(format_cpu)
                        .unwrap_or_else(|| "—".to_string()),
                    latest
                        .cpu_capacity_millicores
                        .map(format_cpu)
                        .unwrap_or_else(|| "—".to_string()),
                    cx.theme().chart_1,
                    cx,
                ))
                .child(usage_bar(
                    "Memory",
                    latest.memory_percent(),
                    latest
                        .memory_used_bytes
                        .map(format_bytes)
                        .unwrap_or_else(|| "—".to_string()),
                    latest
                        .memory_capacity_bytes
                        .map(format_bytes)
                        .unwrap_or_else(|| "—".to_string()),
                    cx.theme().chart_2,
                    cx,
                )),
        );
    }

    let points: Vec<UsagePoint> = usage
        .samples()
        .iter()
        .filter_map(|sample| {
            let cpu = sample.cpu_percent()?;
            let memory = sample.memory_percent()?;
            Some(UsagePoint {
                label: SharedString::from(format_sample_label(sample, latest)),
                cpu: cpu as f64,
                memory: memory as f64,
            })
        })
        .collect();

    if points.len() >= 2 {
        let last = usage.samples().len();
        body = body.child(
            v_flex()
                .gap_3()
                .child(chart_label("CPU usage", cx.theme().chart_1, cx))
                .child(
                    div().h(px(130.)).w_full().child(
                        AreaChart::new(points.clone())
                            .id("node-cpu-usage")
                            .x(|point| point.label.clone())
                            .y(|point| point.cpu)
                            .stroke(cx.theme().chart_1)
                            .fill(cx.theme().chart_1.opacity(0.12))
                            .natural()
                            .x_axis(true)
                            .x_tick_count(4)
                            .y_axis(true)
                            .y_tick_format(|value| format!("{value:.0}%"))
                            .y_domain(0.0, 100.0)
                            .grid(true),
                    ),
                )
                .child(chart_label("Memory usage", cx.theme().chart_2, cx))
                .child(
                    div().h(px(130.)).w_full().child(
                        AreaChart::new(points)
                            .id("node-memory-usage")
                            .x(|point| point.label.clone())
                            .y(|point| point.memory)
                            .stroke(cx.theme().chart_2)
                            .fill(cx.theme().chart_2.opacity(0.12))
                            .natural()
                            .x_axis(true)
                            .x_tick_count(4)
                            .y_axis(true)
                            .y_tick_format(|value| format!("{value:.0}%"))
                            .y_domain(0.0, 100.0)
                            .grid(true),
                    ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Last {last} samples · updated every 10s")),
                ),
        );
    } else if latest.is_some() {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Collecting usage samples…"),
        );
    } else if let Some(error) = error {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().warning)
                .child(error.to_string()),
        );
    } else {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Waiting for the first metrics sample…"),
        );
    }

    section("Usage", body, cx)
}

#[derive(Clone)]
struct UsagePoint {
    label: SharedString,
    cpu: f64,
    memory: f64,
}

fn chart_label(
    label: &str,
    color: impl Into<gpui_kit::Hsla>,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(div().size(px(6.)).rounded_full().bg(color.into()))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .into_any_element()
}

fn usage_bar(
    label: &str,
    percent: Option<f32>,
    used: String,
    capacity: String,
    color: impl Into<gpui_kit::Hsla>,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let percent = percent.unwrap_or(0.0);
    v_flex()
        .gap_1()
        .child(
            h_flex()
                .justify_between()
                .gap_2()
                .text_xs()
                .child(div().flex_shrink_0().child(label.to_string()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_right()
                        .whitespace_normal()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{used} / {capacity} · {percent:.0}%")),
                ),
        )
        .child(
            Progress::new(format!("node-usage-{label}"))
                .value(percent)
                .color(color),
        )
        .into_any_element()
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

fn value_pair(label: &str, value: &str, cx: &mut Context<NodesView>) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(wrap_text(label)),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(wrap_text(value)),
        )
        .into_any_element()
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
                    .child(wrap_text(&condition.reason)),
            );
        }
        if !condition.message.is_empty() {
            block = block.child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .whitespace_normal()
                    .child(wrap_text(&condition.message)),
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

fn key_values(
    title: &str,
    values: &BTreeMap<String, String>,
    cx: &mut Context<NodesView>,
) -> AnyElement {
    let mut body = v_flex();
    if values.is_empty() {
        body = body.child(empty_row("None", cx));
    }
    for (key, value) in values {
        body = body.child(field(key, value, cx));
    }
    section(title, body, cx)
}

fn images(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    let mut body = v_flex();
    if detail.images.is_empty() {
        body = body.child(empty_row("No images reported", cx));
    }
    for image in &detail.images {
        let label = format!("Image · {}", format_bytes(image.size_bytes.max(0) as u64));
        body = body.child(field(&label, &image.names.join(", "), cx));
    }
    section("Images", body, cx)
}

fn raw_manifest(detail: &NodeDetail, cx: &mut Context<NodesView>) -> AnyElement {
    section(
        "Manifest",
        div()
            .w_full()
            .min_w_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .font_family("monospace")
            .text_xs()
            .whitespace_normal()
            .text_color(cx.theme().muted_foreground)
            .child(wrap_text(&detail.raw_json)),
        cx,
    )
}

/// Add zero-width break opportunities inside long unbroken tokens so labels,
/// annotations, image names, and manifest values can wrap inside the pane.
fn wrap_text(value: &str) -> String {
    const BREAK_EVERY: usize = 24;
    let mut out = String::with_capacity(value.len() + value.len() / BREAK_EVERY);
    let mut run = 0;
    for character in value.chars() {
        out.push(character);
        if character.is_whitespace() || character == '\n' {
            run = 0;
        } else {
            run += 1;
            if run >= BREAK_EVERY {
                out.push('\u{200b}');
                run = 0;
            }
        }
    }
    out
}

fn field(label: &str, value: &str, cx: &mut Context<NodesView>) -> AnyElement {
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .py_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(wrap_text(label)),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(wrap_text(value)),
        )
        .into_any_element()
}

fn empty_row(message: &str, cx: &mut Context<NodesView>) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .py_1()
        .text_xs()
        .whitespace_normal()
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}

fn dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}

fn age(created_at: Option<i64>) -> String {
    let Some(created) = created_at else {
        return "—".to_string();
    };
    let secs = (jiff::Timestamp::now().as_second() - created).max(0) as u64;
    if secs >= 86_400 {
        format!("{}d", secs / 86_400)
    } else if secs >= 3_600 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}m", secs / 60)
    }
}

fn format_sample_label(sample: &NodeUsage, latest: Option<&NodeUsage>) -> String {
    let Some(latest) = latest else {
        return String::new();
    };
    let behind = (latest.observed_at - sample.observed_at).max(0);
    if behind == 0 {
        "now".to_string()
    } else {
        format!("-{behind}s")
    }
}

fn format_cpu(millicores: u64) -> String {
    if millicores >= 1000 {
        let cores = millicores as f64 / 1000.0;
        if (cores - cores.round()).abs() < f64::EPSILON {
            format!("{} cores", cores.round() as u64)
        } else {
            format!("{cores:.1} cores")
        }
    } else {
        format!("{millicores}m")
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= TIB {
        format!("{:.1} TiB", bytes / TIB)
    } else if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes.round() as u64)
    }
}
