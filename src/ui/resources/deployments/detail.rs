//! Deployment detail view.

use gpui_kit::base::v_flex;
use gpui_kit::{AnyElement, Context, Entity, IntoElement, ParentElement as _, Styled as _};

use crate::models::ResourceKind;
use crate::models::dashboard::{DeploymentSection, DeploymentWidgetSource, WidgetSource};
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::dashboard::pinnable;
use crate::ui::resources::deployments::actions::DeploymentActionsWidget;
use crate::ui::resources::detail_common::{
    conditions_section, fields_section, json_section, path, pod_template_sections,
    render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    actions: Option<&DeploymentActionsWidget>,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::Deployments,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let section = |section: DeploymentSection| {
                WidgetSource::Deployment(DeploymentWidgetSource::Section {
                    namespace: detail.namespace.clone(),
                    name: detail.name.clone(),
                    section,
                })
            };
            let id = |part: &str| {
                format!(
                    "deployment-section-{}-{}-{part}",
                    detail.namespace, detail.name
                )
            };
            let mut sections = Vec::new();
            if let Some(actions) = actions {
                sections.push(actions.element(cx));
            }
            sections.push(pinnable(
                id("overview"),
                overview(detail, cx),
                section(DeploymentSection::Overview),
            ));
            sections.push(pinnable(
                id("selector"),
                selector(detail, cx),
                section(DeploymentSection::Selector),
            ));
            sections.push(pinnable(
                id("strategy"),
                strategy(detail, cx),
                section(DeploymentSection::Strategy),
            ));
            sections.push(pinnable(
                id("conditions"),
                conditions(detail, cx),
                section(DeploymentSection::Conditions),
            ));
            sections.push(pinnable(
                id("pod-template"),
                pod_template(detail, cx),
                section(DeploymentSection::PodTemplate),
            ));
            sections
        },
    )
}

pub(crate) fn overview<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    let value = &detail.value;
    let spec = path(value, &["spec"]);
    let status = path(value, &["status"]);
    fields_section(
        "Overview",
        vec![
            ("Replicas", text(spec.and_then(|spec| spec.get("replicas")))),
            (
                "Ready",
                text(status.and_then(|status| status.get("readyReplicas"))),
            ),
            (
                "Updated",
                text(status.and_then(|status| status.get("updatedReplicas"))),
            ),
            (
                "Available",
                text(status.and_then(|status| status.get("availableReplicas"))),
            ),
            (
                "Unavailable",
                text(status.and_then(|status| status.get("unavailableReplicas"))),
            ),
            (
                "Observed generation",
                text(status.and_then(|status| status.get("observedGeneration"))),
            ),
            (
                "Min ready seconds",
                text(spec.and_then(|spec| spec.get("minReadySeconds"))),
            ),
            (
                "Revision history limit",
                text(spec.and_then(|spec| spec.get("revisionHistoryLimit"))),
            ),
            (
                "Progress deadline",
                text(spec.and_then(|spec| spec.get("progressDeadlineSeconds"))),
            ),
        ],
        cx,
    )
}

pub(crate) fn selector<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    json_section(
        "Selector",
        "selector",
        path(&detail.value, &["spec", "selector"]),
        cx,
    )
}

pub(crate) fn strategy<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    json_section(
        "Strategy",
        "strategy",
        path(&detail.value, &["spec", "strategy"]),
        cx,
    )
}

pub(crate) fn conditions<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    conditions_section(path(&detail.value, &["status", "conditions"]), cx)
}

pub(crate) fn pod_template<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> AnyElement {
    let template = path(&detail.value, &["spec", "template", "spec"]);
    v_flex()
        .gap_6()
        .children(pod_template_sections(template, cx))
        .into_any_element()
}
