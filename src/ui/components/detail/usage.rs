//! Usage section built from the shared usage meter and chart leaves.

use gpui_kit::base::v_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, div};

use crate::models::resources::usage::UsageHistory;
use crate::ui::components::usage::{ChartPoint, ChartView, MeterView, usage_chart, usage_meter};

use super::text::{SelectableTextStyle, selectable_text};
use super::{format_bytes, format_cpu, format_sample_label};

pub(crate) fn usage_body(
    id_prefix: &str,
    usage: &UsageHistory,
    error: Option<&str>,
    capacity_basis: &str,
    compact: bool,
    cx: &App,
) -> AnyElement {
    let latest = usage.latest();
    let chart_height = if compact { 100.0 } else { 130.0 };
    let mut body = v_flex().gap_4();

    if let Some(latest) = latest {
        let cpu_used = latest.cpu_used_millicores.map(format_cpu);
        let cpu_capacity = latest.cpu_capacity_millicores.map(format_cpu);
        let memory_used = latest.memory_used_bytes.map(format_bytes);
        let memory_capacity = latest.memory_capacity_bytes.map(format_bytes);
        body = body.child(
            v_flex()
                .gap_3()
                .child(usage_meter(
                    &format!("{id_prefix}-cpu"),
                    MeterView {
                        label: "CPU",
                        percent: latest.cpu_percent(),
                        used: cpu_used.as_deref(),
                        capacity: cpu_capacity.as_deref(),
                        capacity_basis,
                        color: cx.theme().chart_1,
                    },
                    cx,
                ))
                .child(usage_meter(
                    &format!("{id_prefix}-memory"),
                    MeterView {
                        label: "Memory",
                        percent: latest.memory_percent(),
                        used: memory_used.as_deref(),
                        capacity: memory_capacity.as_deref(),
                        capacity_basis,
                        color: cx.theme().chart_2,
                    },
                    cx,
                )),
        );
    }

    let latest_at = latest.map(|sample| sample.observed_at);
    let cpu_points: Vec<ChartPoint> = usage
        .samples()
        .iter()
        .filter_map(|sample| {
            let percent = sample.cpu_percent()?;
            Some(ChartPoint {
                label: format_sample_label(sample.observed_at, latest_at).into(),
                value: percent as f64,
            })
        })
        .collect();
    let memory_points: Vec<ChartPoint> = usage
        .samples()
        .iter()
        .filter_map(|sample| {
            let percent = sample.memory_percent()?;
            Some(ChartPoint {
                label: format_sample_label(sample.observed_at, latest_at).into(),
                value: percent as f64,
            })
        })
        .collect();

    if cpu_points.len() >= 2 {
        body = body
            .child(usage_chart(
                &format!("{id_prefix}-cpu-chart"),
                ChartView {
                    title: "CPU usage",
                    points: &cpu_points,
                    color: cx.theme().chart_1,
                    height: chart_height,
                },
                cx,
            ))
            .child(usage_chart(
                &format!("{id_prefix}-memory-chart"),
                ChartView {
                    title: "Memory usage",
                    points: &memory_points,
                    color: cx.theme().chart_2,
                    height: chart_height,
                },
                cx,
            ))
            .child(div().w_full().min_w_0().child(selectable_text(
                format!("{id_prefix}-samples"),
                format!("Last {} samples · updated every 10s", usage.samples().len()),
                SelectableTextStyle::xs().color(cx.theme().muted_foreground),
            )));
    } else if latest.is_some() {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Collecting usage samples…"),
        );
    } else if let Some(error) = error {
        body = body.child(div().w_full().min_w_0().child(selectable_text(
            format!("{id_prefix}-error"),
            error.to_string(),
            SelectableTextStyle::xs().color(cx.theme().warning),
        )));
    } else {
        body = body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Waiting for the first metrics sample…"),
        );
    }

    body.into_any_element()
}
