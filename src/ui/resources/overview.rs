//! Cluster overview dashboard: health cards, node usage, and unhealthy pods.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    group_box::{GroupBox, GroupBoxVariants as _},
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};

use crate::cluster::ClusterState;
use crate::models::Item;
use crate::models::resources::node::detail::NodeUsage;
use crate::models::resources::overview::{ClusterOverviewStats, PodHealth, is_unhealthy_pod};
use crate::view_models::ClusterOverviewViewModel;

pub struct OverviewView {
    vm: Entity<ClusterOverviewViewModel>,
    _vm_sub: Subscription,
    _cluster_sub: Subscription,
}

impl OverviewView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let vm = cx.new(|cx| ClusterOverviewViewModel::new(window, cx));
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        let cluster_subscription = cx.observe_global::<ClusterState>(|_, cx| cx.notify());
        Self {
            vm,
            _vm_sub: subscription,
            _cluster_sub: cluster_subscription,
        }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }

    fn header(&self, stats: &ClusterOverviewStats, cx: &mut Context<Self>) -> AnyElement {
        let state = cx.global::<ClusterState>();
        let context = state
            .context()
            .map(str::to_string)
            .unwrap_or_else(|| "No cluster".to_string());
        let namespaces = state.namespaces().len();
        let view = cx.entity().clone();
        let refresh = Button::new("overview-refresh")
            .ghost()
            .small()
            .icon(Icon::new(gpui_kit::assets::IconName::RefreshCw))
            .tooltip("Refresh overview")
            .on_click(move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.vm.update(cx, |vm, cx| vm.refresh(cx));
                });
            });

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
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Cluster Overview"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} · {} namespaces · {} nodes · {} pods",
                                context, namespaces, stats.nodes_total, stats.pods_total
                            )),
                    ),
            )
            .child(refresh)
            .into_any_element()
    }
}

impl Render for OverviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = self.vm.read(cx);
        let stats = vm.stats().clone();
        let nodes = vm.nodes().to_vec();
        let pods = vm.pods().to_vec();
        let metrics = vm.metrics().clone();
        let metrics_error = vm.metrics_error().map(str::to_string);
        let header = self.header(&stats, cx);

        let cards = h_flex()
            .gap_3()
            .items_stretch()
            .child(metric_card(
                "overview-nodes",
                "Nodes",
                format!("{}/{}", stats.nodes_ready, stats.nodes_total),
                if stats.nodes_not_ready() == 0 {
                    "All nodes ready".to_string()
                } else {
                    format!("{} not ready", stats.nodes_not_ready())
                },
                if stats.nodes_not_ready() == 0 {
                    cx.theme().success
                } else {
                    cx.theme().danger
                },
                cx,
            ))
            .child(metric_card(
                "overview-pods",
                "Pods",
                stats.pods_total.to_string(),
                format!("{} running", stats.pods_running),
                cx.theme().foreground,
                cx,
            ))
            .child(metric_card(
                "overview-unhealthy",
                "Pods needing attention",
                stats.pods_unhealthy().to_string(),
                format!(
                    "{} pending · {} failed · {} crashing",
                    stats.pods_pending, stats.pods_failed, stats.pods_crashloopbackoff
                ),
                if stats.pods_unhealthy() == 0 {
                    cx.theme().success
                } else {
                    cx.theme().danger
                },
                cx,
            ))
            .child(metric_card(
                "overview-restarts",
                "Container restarts",
                stats.pod_restarts.to_string(),
                "Across all pods".to_string(),
                if stats.pod_restarts == 0 {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().warning
                },
                cx,
            ));

        let body = v_flex()
            .gap_4()
            .p_4()
            .child(cards)
            .child(node_section(
                &nodes,
                &stats,
                &metrics,
                metrics_error.as_deref(),
                cx,
            ))
            .child(pod_section(&stats, &pods, cx));

        v_flex()
            .size_full()
            .min_w_0()
            .child(header)
            .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(body))
    }
}

fn metric_card(
    id: &'static str,
    label: &str,
    value: String,
    detail: String,
    value_color: Hsla,
    cx: &App,
) -> AnyElement {
    v_flex()
        .id(id)
        .flex_1()
        .min_w_0()
        .gap_1()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .p_3()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(26.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(value_color)
                .child(value),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .into_any_element()
}

fn node_section(
    nodes: &[Arc<Item>],
    stats: &ClusterOverviewStats,
    metrics: &BTreeMap<String, NodeUsage>,
    metrics_error: Option<&str>,
    cx: &App,
) -> AnyElement {
    let mut body = v_flex();
    body = body.child(node_header_row(cx));
    if nodes.is_empty() {
        body = body.child(empty_row("No nodes reported.", cx));
    }
    for node in nodes {
        let ready = node.status == "ready";
        let usage = metrics.get(&node.name);
        let cpu = usage
            .and_then(|usage| usage.cpu_used_millicores)
            .map(cpu_label)
            .unwrap_or_else(|| "—".to_string());
        let memory = usage
            .and_then(|usage| usage.memory_used_bytes)
            .map(memory_label)
            .unwrap_or_else(|| "—".to_string());
        body = body.child(
            h_flex()
                .gap_3()
                .items_center()
                .px_2()
                .py_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(node.name.clone()),
                )
                .child(
                    div()
                        .w(px(86.))
                        .text_color(if ready {
                            cx.theme().success
                        } else {
                            cx.theme().danger
                        })
                        .child(if ready { "Ready" } else { "NotReady" }),
                )
                .child(
                    div()
                        .w(px(58.))
                        .text_color(cx.theme().muted_foreground)
                        .child(stats.pods_on(&node.name).to_string()),
                )
                .child(
                    div()
                        .w(px(96.))
                        .text_color(cx.theme().muted_foreground)
                        .child(cpu),
                )
                .child(
                    div()
                        .w(px(104.))
                        .text_color(cx.theme().muted_foreground)
                        .child(memory),
                )
                .child(
                    div()
                        .w(px(116.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(cx.theme().muted_foreground)
                        .child(if node.version.is_empty() {
                            "—".to_string()
                        } else {
                            node.version.clone()
                        }),
                )
                .child(
                    div()
                        .w(px(58.))
                        .text_right()
                        .text_color(cx.theme().muted_foreground)
                        .child(node.age()),
                ),
        );
    }
    if let Some(error) = metrics_error {
        body = body.child(
            div()
                .pt_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(error.to_string()),
        );
    }
    body = body.child(
        div()
            .pt_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("CPU and memory usage come from metrics-server."),
    );

    GroupBox::new()
        .outline()
        .title("Nodes")
        .child(body)
        .into_any_element()
}

fn node_header_row(cx: &App) -> AnyElement {
    h_flex()
        .gap_3()
        .items_center()
        .px_2()
        .pb_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(div().flex_1().min_w_0().child("Node"))
        .child(div().w(px(86.)).child("Status"))
        .child(div().w(px(58.)).child("Pods"))
        .child(div().w(px(96.)).child("CPU used"))
        .child(div().w(px(104.)).child("Memory used"))
        .child(div().w(px(116.)).child("Kubelet"))
        .child(div().w(px(58.)).text_right().child("Age"))
        .into_any_element()
}

fn pod_section(stats: &ClusterOverviewStats, pods: &[Arc<Item>], cx: &App) -> AnyElement {
    let mut health = h_flex().gap_2().items_center();
    for (label, count, color) in [
        ("Running", stats.pods_running, cx.theme().success),
        ("Pending", stats.pods_pending, cx.theme().warning),
        (
            "Succeeded",
            stats.pods_succeeded,
            cx.theme().muted_foreground,
        ),
        ("Failed", stats.pods_failed, cx.theme().danger),
        (
            "CrashLoopBackOff",
            stats.pods_crashloopbackoff,
            cx.theme().danger,
        ),
        ("Unknown", stats.pods_unknown, cx.theme().warning),
    ] {
        health = health.child(health_chip(label, count, color, cx));
    }

    let mut unhealthy = pods
        .iter()
        .filter(|pod| is_unhealthy_pod(pod))
        .collect::<Vec<_>>();
    unhealthy.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut body = v_flex().gap_3().child(health);
    if unhealthy.is_empty() {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("No pending, failed, or crashing pods."),
        );
    } else {
        body = body.child(
            div()
                .pt_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} pods need attention", unhealthy.len())),
        );
        body = body.child(
            h_flex()
                .gap_3()
                .px_2()
                .pb_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(div().w(px(150.)).child("Namespace"))
                .child(div().flex_1().min_w_0().child("Pod"))
                .child(div().w(px(140.)).child("Status"))
                .child(div().w(px(72.)).text_right().child("Restarts"))
                .child(div().w(px(56.)).text_right().child("Age")),
        );
        for pod in unhealthy.iter().take(8) {
            let status = PodHealth::from_status(&pod.status);
            body = body.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .w(px(150.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(cx.theme().muted_foreground)
                            .child(pod.namespace.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(pod.name.clone()),
                    )
                    .child(
                        div()
                            .w(px(140.))
                            .text_color(status_color(status, cx))
                            .child(status.label()),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .text_right()
                            .text_color(if pod.restarts > 0 {
                                cx.theme().warning
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(pod.restarts.to_string()),
                    )
                    .child(
                        div()
                            .w(px(56.))
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(pod.age()),
                    ),
            );
        }
        if unhealthy.len() > 8 {
            body = body.child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("+{} more", unhealthy.len() - 8)),
            );
        }
    }

    GroupBox::new()
        .outline()
        .title("Pods")
        .child(body)
        .into_any_element()
}

fn health_chip(label: &str, count: usize, color: Hsla, cx: &App) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .px_2()
        .py_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(count.to_string()),
        )
        .into_any_element()
}

fn empty_row(message: &str, cx: &App) -> AnyElement {
    div()
        .py_2()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}

fn status_color(status: PodHealth, cx: &App) -> Hsla {
    match status {
        PodHealth::Running | PodHealth::Succeeded => cx.theme().success,
        PodHealth::Pending | PodHealth::Unknown => cx.theme().warning,
        PodHealth::Failed | PodHealth::CrashLoopBackOff => cx.theme().danger,
    }
}

fn cpu_label(millicores: u64) -> String {
    if millicores >= 1_000 {
        format!("{:.1} cores", millicores as f64 / 1_000.0)
    } else {
        format!("{millicores}m")
    }
}

fn memory_label(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else {
        format!("{:.0} MiB", bytes / MIB)
    }
}
