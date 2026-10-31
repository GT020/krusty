//! One usage chart with a titled series.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::chart::AreaChart;
use gpui_kit::{
    AnyElement, App, Hsla, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};

#[derive(Clone, Debug)]
pub(crate) struct ChartPoint {
    pub label: SharedString,
    pub value: f64,
}

pub(crate) struct ChartView<'a> {
    pub title: &'a str,
    pub points: &'a [ChartPoint],
    pub color: Hsla,
    pub height: f32,
}

pub(crate) fn usage_chart(id: &str, view: ChartView<'_>, cx: &App) -> AnyElement {
    let points = view.points.to_vec();
    let chart_id = SharedString::from(id.to_string());
    v_flex()
        .gap_3()
        .child(chart_label(view.title, view.color, cx))
        .child(
            div().h(px(view.height)).w_full().child(
                AreaChart::new(points)
                    .id(chart_id)
                    .x(|point| point.label.clone())
                    .y(|point| point.value)
                    .stroke(view.color)
                    .fill(view.color.opacity(0.12))
                    .natural()
                    .x_axis(true)
                    .x_tick_count(4)
                    .y_axis(true)
                    .y_tick_format(|value| format!("{value:.0}%"))
                    .y_domain(0.0, 100.0)
                    .grid(true),
            ),
        )
        .into_any_element()
}

fn chart_label(title: &str, color: impl Into<Hsla>, cx: &App) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(div().size(px(6.)).rounded_full().bg(color.into()))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .into_any_element()
}
