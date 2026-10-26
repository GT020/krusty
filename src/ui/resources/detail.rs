//! Generic detail pane used by resource kinds without a dedicated view.

use std::collections::BTreeMap;

use gpui_kit::base::v_flex;
use gpui_kit::{AnyElement, Context, Entity, IntoElement, ParentElement as _, Styled as _, div};

use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::components::detail::{
    detail_header, detail_shell, error_body, field, key_values, loading_body, raw_manifest, section,
};
use serde_json::Value;

#[allow(clippy::too_many_arguments)]
pub fn render_detail<T: 'static>(
    kind: ResourceKind,
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
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
        ResourceDetailState::Ready(detail) => detail_body(&detail, cx),
        ResourceDetailState::Closed => return div().into_any_element(),
    };

    detail_shell(header, body, compact, cx)
}

fn detail_body<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    let mut body = v_flex().gap_6().p_4().child(overview(detail, cx));

    let mut top_level: Vec<&String> = detail
        .value
        .as_object()
        .map(|object| object.keys().collect())
        .unwrap_or_default();
    top_level.sort();
    for key in top_level {
        if matches!(key.as_str(), "apiVersion" | "kind" | "metadata") {
            continue;
        }
        let Some(value) = detail.value.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        body = body.child(json_section(&humanize(key), key, value, cx));
    }

    let metadata = detail.value.get("metadata");
    let labels = string_map(metadata.and_then(|metadata| metadata.get("labels")));
    let annotations = string_map(metadata.and_then(|metadata| metadata.get("annotations")));
    body.child(key_values("Labels", &labels, cx))
        .child(key_values("Annotations", &annotations, cx))
        .child(raw_manifest(&detail.raw_json, cx))
        .into_any_element()
}

fn overview<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    let metadata = detail.value.get("metadata");
    let mut body = v_flex()
        .child(field("Kind", detail.kind.label(), cx))
        .child(field(
            "API version",
            &scalar_text(detail.value.get("apiVersion")),
            cx,
        ))
        .child(field("Name", &detail.name, cx));
    let namespace = if detail.namespace.is_empty() {
        "Cluster-scoped"
    } else {
        &detail.namespace
    };
    body = body.child(field("Namespace", namespace, cx));
    if let Some(metadata) = metadata {
        for (label, key) in [
            ("UID", "uid"),
            ("Resource version", "resourceVersion"),
            ("Generation", "generation"),
            ("Created", "creationTimestamp"),
            ("Deleted", "deletionTimestamp"),
        ] {
            let value = metadata.get(key);
            if value.is_none_or(Value::is_null) {
                continue;
            }
            body = body.child(field(label, &scalar_text(value), cx));
        }
        if let Some(finalizers) = metadata.get("finalizers").and_then(Value::as_array)
            && !finalizers.is_empty()
        {
            body = body.child(field(
                "Finalizers",
                &finalizers
                    .iter()
                    .map(|value| scalar_text(Some(value)))
                    .collect::<Vec<_>>()
                    .join(", "),
                cx,
            ));
        }
    }
    section("Overview", body, cx)
}

fn json_section<T: 'static>(
    title: &str,
    prefix: &str,
    value: &Value,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut fields = Vec::new();
    flatten(prefix, value, &mut fields);
    let mut body = v_flex();
    for (path, value) in fields {
        body = body.child(field(&path, &value, cx));
    }
    section(title, body, cx)
}

fn flatten(prefix: &str, value: &Value, out: &mut Vec<(String, String)>) {
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
        _ => out.push((prefix.to_string(), scalar_text(Some(value)))),
    }
}

fn scalar_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => "—".to_string(),
        Some(value) => value.to_string(),
    }
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), scalar_text(Some(value))))
                .collect()
        })
        .unwrap_or_default()
}

fn namespace_label(namespace: &str) -> &str {
    if namespace.is_empty() {
        "Cluster-scoped"
    } else {
        namespace
    }
}

fn humanize(key: &str) -> String {
    let mut output = String::with_capacity(key.len() + 4);
    let mut previous_lowercase = false;
    for character in key.chars() {
        if character.is_ascii_uppercase() && previous_lowercase {
            output.push(' ');
        }
        previous_lowercase = character.is_ascii_lowercase();
        if output.is_empty() {
            output.extend(character.to_uppercase());
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flattens_nested_objects_and_arrays() {
        let value = json!({
            "spec": {
                "replicas": 3,
                "template": { "spec": { "containers": [{ "name": "api" }] } }
            }
        });
        let mut fields = Vec::new();
        flatten("spec", &value["spec"], &mut fields);
        assert!(fields.contains(&("spec.replicas".to_string(), "3".to_string())));
        assert!(fields.contains(&(
            "spec.template.spec.containers[0].name".to_string(),
            "api".to_string()
        )));
    }

    #[test]
    fn humanizes_camel_case_keys() {
        assert_eq!(humanize("binaryData"), "Binary Data");
        assert_eq!(humanize("spec"), "Spec");
        assert_eq!(humanize("podCIDRs"), "Pod CIDRs");
    }
}
