//! Shared building blocks for resource detail panes.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::base::{SelectableText, TextSelection, h_flex, v_flex};
use gpui_kit::component::chart::AreaChart;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AbsoluteLength, AnyElement, ClipboardItem, Context, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Styled as _,
    TextStyleRefinement, WhiteSpace, Window, div, px, rems,
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
    root.id("detail-shell")
        .context_menu(text_selection_context_menu)
        .into_any_element()
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
                .child(div().w_full().min_w_0().child(selectable_text(
                    format!("{id_prefix}-title"),
                    title.to_string(),
                    SelectableTextStyle::sm().weight(FontWeight::MEDIUM),
                )))
                .child(div().w_full().min_w_0().child(selectable_text(
                    format!("{id_prefix}-subtitle"),
                    subtitle.to_string(),
                    SelectableTextStyle::xs(),
                ))),
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
        .child(div().w_full().min_w_0().child(selectable_text(
            "detail-error-message".to_string(),
            message.to_string(),
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        )))
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

/// Visual treatment for [`SelectableText`] runs in detail panes.
#[derive(Clone, Copy)]
pub(crate) struct SelectableTextStyle {
    font_size: AbsoluteLength,
    color: Option<Hsla>,
    font_weight: Option<FontWeight>,
    monospace: bool,
}

impl SelectableTextStyle {
    pub(crate) fn xs() -> Self {
        Self {
            font_size: rems(0.75).into(),
            color: None,
            font_weight: None,
            monospace: false,
        }
    }

    pub(crate) fn sm() -> Self {
        Self {
            font_size: rems(0.875).into(),
            ..Self::xs()
        }
    }

    pub(crate) fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub(crate) fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub(crate) fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }
}

pub(crate) fn selectable_text(
    id: String,
    text: impl Into<SharedString>,
    style: SelectableTextStyle,
) -> AnyElement {
    // Detail text is rendered in reading order, so a process-wide sequence keeps
    // runs in the order they appear on screen for drag selection across fields.
    static NEXT_DOCUMENT_ORDER: AtomicU64 = AtomicU64::new(0);
    let refinement = TextStyleRefinement {
        color: style.color,
        font_family: style.monospace.then(|| "monospace".into()),
        font_size: Some(style.font_size),
        font_weight: style.font_weight,
        white_space: Some(WhiteSpace::Normal),
        ..Default::default()
    };
    SelectableText::new(id, text)
        .document_order(NEXT_DOCUMENT_ORDER.fetch_add(1, Ordering::Relaxed))
        .text_style(refinement)
        .into_any_element()
}

/// Context menu for the current window text selection.
///
/// Returns an empty menu when nothing is selected, so right-clicking plain
/// detail text does not open a menu without a target.
pub(crate) fn text_selection_context_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    if TextSelection::selected_text(window, cx).trim().is_empty() {
        return menu;
    }
    menu.item(
        PopupMenuItem::new("Copy")
            .icon(IconName::Copy)
            .on_click(|_, window, cx| {
                let text = TextSelection::selected_text(window, cx).trim().to_string();
                if !text.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }),
    )
}

pub(crate) fn field<T: 'static>(label: &str, value: &str, cx: &mut Context<T>) -> AnyElement {
    field_with_id(format!("detail-field-{label}"), label, value, cx)
}

pub(crate) fn field_with_id<T: 'static>(
    id: String,
    label: &str,
    value: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    field_row(id, label, value, None, cx)
}

pub(crate) fn field_colored<T: 'static>(
    label: &str,
    value: &str,
    color: Hsla,
    cx: &mut Context<T>,
) -> AnyElement {
    field_row(
        format!("detail-field-{label}"),
        label,
        value,
        Some(color),
        cx,
    )
}

fn field_row<T: 'static>(
    id: String,
    label: &str,
    value: &str,
    color: Option<Hsla>,
    cx: &mut Context<T>,
) -> AnyElement {
    let label = selectable_text(
        format!("{id}-label"),
        label.to_string(),
        SelectableTextStyle::xs().color(cx.theme().muted_foreground),
    );
    let value_style = match color {
        Some(color) => SelectableTextStyle::xs().color(color),
        None => SelectableTextStyle::xs(),
    };
    let value = selectable_text(format!("{id}-value"), value.to_string(), value_style);
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .py_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(div().w_full().min_w_0().child(label))
        .child(div().w_full().min_w_0().child(value))
        .into_any_element()
}

pub(crate) fn value_pair<T: 'static>(label: &str, value: &str, cx: &mut Context<T>) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(div().w_full().min_w_0().child(selectable_text(
            format!("detail-value-{label}-label"),
            label.to_string(),
            SelectableTextStyle::xs().color(cx.theme().muted_foreground),
        )))
        .child(div().w_full().min_w_0().child(selectable_text(
            format!("detail-value-{label}-value"),
            value.to_string(),
            SelectableTextStyle::xs(),
        )))
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
        body = body.child(field_with_id(
            format!("detail-{title}-{key}"),
            key,
            value,
            cx,
        ));
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
            .child(
                div().w_full().min_w_0().child(selectable_text(
                    "detail-manifest-text".to_string(),
                    json.to_string(),
                    SelectableTextStyle::xs()
                        .monospace()
                        .color(cx.theme().muted_foreground),
                )),
            ),
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
