//! Shared building blocks for resource detail panes.

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

use crate::models::resources::usage::UsageHistory;

pub(crate) fn detail_shell<T: 'static>(
    header: AnyElement,
    body: AnyElement,
    compact: bool,
    cx: &mut Context<T>,
) -> AnyElement {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn detail_header<T: 'static>(
    title: &str,
    subtitle: &str,
    compact: bool,
    back_tooltip: &str,
    id_prefix: &str,
    view: Entity<T>,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    let back_handle = view.clone();
    let close_handle = view;
    let close_button = if compact {
        Button::new(format!("{id_prefix}-back"))
            .ghost()
            .small()
            .icon(IconName::ArrowLeft)
            .label("Back")
            .tooltip(back_tooltip.to_string())
            .on_click(move |_, _, cx| {
                back_handle.update(cx, close);
            })
    } else {
        Button::new(format!("{id_prefix}-close"))
            .ghost()
            .small()
            .icon(IconName::CircleX)
            .tooltip("Close details")
            .on_click(move |_, _, cx| {
                close_handle.update(cx, close);
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
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .whitespace_normal()
                        .child(subtitle.to_string()),
                ),
        )
        .child(close_button)
        .into_any_element()
}

pub(crate) fn loading_body<T: 'static>(
    title: &str,
    description: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    v_flex()
        .p_4()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(description.to_string()),
        )
        .into_any_element()
}

pub(crate) fn error_body<T: 'static>(
    title: &str,
    message: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    v_flex()
        .p_4()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().danger)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_xs()
                .whitespace_normal()
                .text_color(cx.theme().muted_foreground)
                .child(message.to_string()),
        )
        .into_any_element()
}

pub(crate) fn section<T: 'static>(
    title: &str,
    body: impl IntoElement,
    cx: &mut Context<T>,
) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(div().w_full().min_w_0().child(body))
        .into_any_element()
}

pub(crate) fn field<T: 'static>(label: &str, value: &str, cx: &mut Context<T>) -> AnyElement {
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

pub(crate) fn value_pair<T: 'static>(label: &str, value: &str, cx: &mut Context<T>) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(value.to_string()),
        )
        .into_any_element()
}

pub(crate) fn empty_row<T: 'static>(message: &str, cx: &mut Context<T>) -> AnyElement {
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

pub(crate) fn key_values<T: 'static>(
    title: &str,
    values: &BTreeMap<String, String>,
    cx: &mut Context<T>,
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

pub(crate) fn raw_manifest<T: 'static>(json: &str, cx: &mut Context<T>) -> AnyElement {
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
            .child(wrap_text(json)),
        cx,
    )
}

pub(crate) fn usage_section<T: 'static>(
    usage: &UsageHistory,
    error: Option<&str>,
    capacity_basis: &str,
    cx: &mut Context<T>,
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
                    latest.cpu_used_millicores.map(format_cpu),
                    latest.cpu_capacity_millicores.map(format_cpu),
                    capacity_basis,
                    cx.theme().chart_1,
                    cx,
                ))
                .child(usage_bar(
                    "Memory",
                    latest.memory_percent(),
                    latest.memory_used_bytes.map(format_bytes),
                    latest.memory_capacity_bytes.map(format_bytes),
                    capacity_basis,
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
                            .id("detail-cpu-usage")
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
                            .id("detail-memory-usage")
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
                .whitespace_normal()
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

fn chart_label<T: 'static>(
    label: &str,
    color: impl Into<gpui_kit::Hsla>,
    cx: &mut Context<T>,
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

fn usage_bar<T: 'static>(
    label: &str,
    percent: Option<f32>,
    used: Option<String>,
    capacity: Option<String>,
    capacity_basis: &str,
    color: impl Into<gpui_kit::Hsla>,
    cx: &mut Context<T>,
) -> AnyElement {
    let percent = percent.unwrap_or(0.0);
    let used = used.unwrap_or_else(|| "—".to_string());
    let capacity = capacity.unwrap_or_else(|| "—".to_string());
    let basis = if capacity_basis.is_empty() {
        String::new()
    } else {
        format!(" {capacity_basis}")
    };
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
                        .child(format!("{used} / {capacity}{basis} · {percent:.0}%")),
                ),
        )
        .child(
            Progress::new(format!("detail-usage-{label}"))
                .value(percent)
                .color(color),
        )
        .into_any_element()
}

pub(crate) fn wrap_text(value: &str) -> String {
    const BREAK_EVERY: usize = 24;
    let mut out = String::with_capacity(value.len() + value.len() / BREAK_EVERY);
    let mut run = 0;
    for character in value.chars() {
        out.push(character);
        if character.is_whitespace() {
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

pub(crate) fn age(created_at: Option<i64>) -> String {
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

pub(crate) fn format_cpu(millicores: u64) -> String {
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

pub(crate) fn format_bytes(bytes: u64) -> String {
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

fn format_sample_label(
    sample: &crate::models::resources::usage::UsageSample,
    latest: Option<&crate::models::resources::usage::UsageSample>,
) -> String {
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
