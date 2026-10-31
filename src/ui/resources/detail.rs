//! Generic detail pane used by resource kinds without a dedicated view.

use gpui_kit::base::v_flex;
use gpui_kit::{AnyElement, Context, Entity, IntoElement, ParentElement as _, Styled as _, div};

use crate::models::ResourceKind;
use crate::models::dashboard::ResourceSection;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::components::detail::{
    detail_header, detail_shell, error_body, field_with_id, humanize, key_values, loading_body,
    raw_manifest, section,
};
use crate::ui::dashboard::pinnable;
use crate::ui::resources::detail_common::{json_section, namespace_label, string_map, text};
use crate::ui::resources::section::section_source;
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
    let section =
        |part: ResourceSection| section_source(detail.kind, &detail.namespace, &detail.name, part);
    let id = |part: &str| {
        format!(
            "section-{}-{}-{}-{part}",
            detail.kind.label(),
            detail.namespace,
            detail.name
        )
    };
    let mut body = v_flex().gap_6().p_4().child(pinnable(
        id("overview"),
        overview(detail, cx),
        section(ResourceSection::Overview),
    ));

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
        let title = humanize(key);
        body = body.child(pinnable(
            id(key),
            json_section(&title, key, Some(value), cx),
            section(ResourceSection::path(title, &[key.as_str()])),
        ));
    }

    let metadata = detail.value.get("metadata");
    let labels = string_map(metadata.and_then(|metadata| metadata.get("labels")));
    let annotations = string_map(metadata.and_then(|metadata| metadata.get("annotations")));
    body.child(pinnable(
        id("labels"),
        key_values("Labels", &labels, cx),
        section(ResourceSection::Labels),
    ))
    .child(pinnable(
        id("annotations"),
        key_values("Annotations", &annotations, cx),
        section(ResourceSection::Annotations),
    ))
    .child(pinnable(
        id("manifest"),
        raw_manifest(&detail.raw_json, cx),
        section(ResourceSection::Manifest),
    ))
    .into_any_element()
}

pub(crate) fn overview<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    overview_with_prefix(detail, "detail-overview", cx)
}

pub(crate) fn overview_with_prefix<T: 'static>(
    detail: &ResourceDetail,
    prefix: &str,
    cx: &mut Context<T>,
) -> AnyElement {
    let metadata = detail.value.get("metadata");
    let field = |label: &str, value: &str| {
        field_with_id(format!("{prefix}-field-{label}"), label, value, cx)
    };
    let mut body = v_flex()
        .child(field("Kind", detail.kind.label()))
        .child(field("API version", &text(detail.value.get("apiVersion"))))
        .child(field("Name", &detail.name));
    body = body.child(field("Namespace", namespace_label(&detail.namespace)));
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
            body = body.child(field(label, &text(value)));
        }
        if let Some(finalizers) = metadata.get("finalizers").and_then(Value::as_array)
            && !finalizers.is_empty()
        {
            body = body.child(field(
                "Finalizers",
                &finalizers
                    .iter()
                    .map(|value| text(Some(value)))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }
    section("Overview", body, cx)
}
