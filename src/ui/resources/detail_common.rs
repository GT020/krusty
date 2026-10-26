//! Shared frame and JSON helpers for resource-specific detail views.

use std::collections::BTreeMap;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement as _, Styled as _, div,
};
use serde_json::Value;

use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::components::detail::{
    age, detail_header, detail_shell, empty_row, error_body, field, key_values, loading_body,
    raw_manifest, section,
};

#[allow(clippy::too_many_arguments)]
pub fn render_resource_detail<T: 'static>(
    kind: ResourceKind,
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
    ready: impl FnOnce(&ResourceDetail, &mut Context<T>) -> Vec<AnyElement>,
) -> AnyElement {
    let (title, subtitle) = match &state {
        ResourceDetailState::Loading {
            namespace, name, ..
        } => (
            name.clone(),
            format!("{} · {}", kind.label(), namespace_label(namespace)),
        ),
        ResourceDetailState::Error {
            namespace, name, ..
        } => (
            name.clone(),
            format!("{} · {}", kind.label(), namespace_label(namespace)),
        ),
        ResourceDetailState::Ready(detail) => (
            detail.name.clone(),
            format!(
                "{} · {}",
                detail.kind.label(),
                namespace_label(&detail.namespace)
            ),
        ),
        ResourceDetailState::Closed => return div().into_any_element(),
    };
    let id_prefix = format!("resource-detail-{}", kind.label().to_lowercase());
    let header = detail_header(
        &title,
        &subtitle,
        compact,
        "Back to list",
        &id_prefix,
        view,
        close,
        cx,
    );

    let body: AnyElement = match state {
        ResourceDetailState::Loading { .. } => loading_body(
            "Loading resource…",
            "Reading the object from the API server.",
            cx,
        ),
        ResourceDetailState::Error { message, .. } => {
            error_body("Could not load this resource", &message, cx)
        }
        ResourceDetailState::Ready(detail) => {
            let mut body = v_flex().gap_6().p_4();
            for element in ready(&detail, cx) {
                body = body.child(element);
            }
            let metadata = detail.value.get("metadata");
            let labels = string_map(metadata.and_then(|metadata| metadata.get("labels")));
            let annotations = string_map(metadata.and_then(|metadata| metadata.get("annotations")));
            body.child(key_values("Labels", &labels, cx))
                .child(key_values("Annotations", &annotations, cx))
                .child(raw_manifest(&detail.raw_json, cx))
                .into_any_element()
        }
        ResourceDetailState::Closed => return div().into_any_element(),
    };

    detail_shell(header, body, compact, cx)
}

pub fn path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |value, key| value.get(*key))
}

pub fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Array(values)) => {
            if values.is_empty() {
                "—".to_string()
            } else {
                values
                    .iter()
                    .map(|value| text(Some(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        Some(Value::Object(_)) => "—".to_string(),
        Some(Value::Null) | None => "—".to_string(),
    }
}

pub fn count(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map(Vec::len).unwrap_or(0)
}

pub fn fields_section<T: 'static>(
    title: &str,
    fields: Vec<(&str, String)>,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut body = v_flex();
    for (label, value) in fields {
        body = body.child(field(label, &value, cx));
    }
    section(title, body, cx)
}

pub fn json_section<T: 'static>(
    title: &str,
    prefix: &str,
    value: Option<&Value>,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut fields = Vec::new();
    if let Some(value) = value {
        flatten(prefix, value, &mut fields);
    }
    let mut body = v_flex();
    if fields.is_empty() {
        body = body.child(empty_row("None", cx));
    }
    for (path, value) in fields {
        body = body.child(field(&path, &value, cx));
    }
    section(title, body, cx)
}

pub fn map_section<T: 'static>(
    title: &str,
    value: Option<&Value>,
    cx: &mut Context<T>,
) -> AnyElement {
    key_values(title, &string_map(value), cx)
}

pub fn conditions_section<T: 'static>(
    conditions: Option<&Value>,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut body = v_flex();
    let Some(items) = conditions.and_then(Value::as_array) else {
        body = body.child(empty_row("No conditions reported", cx));
        return section("Conditions", body, cx);
    };
    if items.is_empty() {
        body = body.child(empty_row("No conditions reported", cx));
    }
    for condition in items {
        let status = text(condition.get("status"));
        let status_color = match status.as_str() {
            "True" => cx.theme().success,
            "Unknown" => cx.theme().warning,
            _ => cx.theme().muted_foreground,
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
                            .child(text(condition.get("type"))),
                    )
                    .child(div().text_xs().text_color(status_color).child(status))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Changed {}",
                                age_from_timestamp(
                                    condition
                                        .get("lastTransitionTime")
                                        .or_else(|| condition.get("lastUpdateTime"))
                                        .or_else(|| condition.get("lastProbeTime"))
                                )
                            )),
                    ),
            );
        let reason = text(condition.get("reason"));
        if reason != "—" {
            block = block.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(reason),
            );
        }
        let message = text(condition.get("message"));
        if message != "—" {
            block = block.child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .whitespace_normal()
                    .child(message),
            );
        }
        body = body.child(block);
    }
    section("Conditions", body, cx)
}

pub fn pod_template_sections<T: 'static>(
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
    sections.push(json_section(
        "Containers",
        "containers",
        pod_spec.get("containers"),
        cx,
    ));
    if count(pod_spec.get("initContainers")) > 0 {
        sections.push(json_section(
            "Init containers",
            "initContainers",
            pod_spec.get("initContainers"),
            cx,
        ));
    }
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

pub fn flatten(prefix: &str, value: &Value, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                out.push((prefix.to_string(), "{}".to_string()));
                return;
            }
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(value) = map.get(key) {
                    flatten(&format!("{prefix}.{key}"), value, out);
                }
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                out.push((prefix.to_string(), "[]".to_string()));
                return;
            }
            for (index, item) in items.iter().enumerate() {
                flatten(&format!("{prefix}[{index}]"), item, out);
            }
        }
        _ => out.push((prefix.to_string(), text(Some(value)))),
    }
}

pub fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), text(Some(value))))
                .collect()
        })
        .unwrap_or_default()
}

fn age_from_timestamp(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<jiff::Timestamp>().ok())
        .map(|timestamp| age(Some(timestamp.as_second())))
        .unwrap_or_else(|| "—".to_string())
}

fn namespace_label(namespace: &str) -> &str {
    if namespace.is_empty() {
        "Cluster-scoped"
    } else {
        namespace
    }
}
