//! Mounted dashboard widgets, grouped by resource.

use gpui_kit::{AnyElement, App, Window};

use crate::models::NamespaceScope;
use crate::models::dashboard::{
    DeploymentWidgetSource, NodeWidgetSource, PodWidgetSource, ResourceSectionSource,
    WidgetDescriptor, WidgetId, WidgetSource,
};
use crate::ui::resources::deployments::actions::{DeploymentActionsArgs, DeploymentActionsWidget};
use crate::ui::resources::deployments::section::DeploymentSectionWidget;
use crate::ui::resources::node::section::NodeSectionWidget;
use crate::ui::resources::node::usage::{NodeUsageArgs, NodeUsageWidget};
use crate::ui::resources::pod::exec::{PodExecArgs, PodExecWidget};
use crate::ui::resources::pod::list::{PodListArgs, PodListWidget};
use crate::ui::resources::pod::logs::{PodLogsArgs, PodLogsWidget};
use crate::ui::resources::pod::section::PodSectionWidget;
use crate::ui::resources::pod::usage::{PodUsageArgs, PodUsageWidget};
use crate::ui::resources::section::ResourceSectionWidget;

pub enum PodWidget {
    Usage(PodUsageWidget),
    Logs(PodLogsWidget),
    Exec(PodExecWidget),
    List(PodListWidget),
    Section(PodSectionWidget),
}

pub enum NodeWidget {
    Usage(NodeUsageWidget),
    Section(NodeSectionWidget),
}

pub enum DeploymentWidget {
    Actions(DeploymentActionsWidget),
    Section(DeploymentSectionWidget),
}

pub enum ResourceWidget {
    Pods(PodWidget),
    Nodes(NodeWidget),
    Deployments(DeploymentWidget),
    Section(ResourceSectionWidget),
}

impl ResourceWidget {
    pub fn element(&self, cx: &App) -> AnyElement {
        match self {
            Self::Pods(widget) => match widget {
                PodWidget::Usage(widget) => widget.element(cx),
                PodWidget::Logs(widget) => widget.element(cx),
                PodWidget::Exec(widget) => widget.element(cx),
                PodWidget::List(widget) => widget.element(cx),
                PodWidget::Section(widget) => widget.element(cx),
            },
            Self::Nodes(NodeWidget::Usage(widget)) => widget.element(cx),
            Self::Nodes(NodeWidget::Section(widget)) => widget.element(cx),
            Self::Deployments(DeploymentWidget::Actions(widget)) => widget.element(cx),
            Self::Deployments(DeploymentWidget::Section(widget)) => widget.element(cx),
            Self::Section(widget) => widget.element(cx),
        }
    }

    pub fn set_active(&self, active: bool, cx: &mut App) {
        match self {
            Self::Pods(widget) => match widget {
                PodWidget::Usage(widget) => widget.set_active(active, cx),
                PodWidget::Logs(widget) => widget.set_active(active, cx),
                PodWidget::Exec(widget) => widget.set_active(active, cx),
                PodWidget::List(widget) => widget.set_active(active, cx),
                PodWidget::Section(widget) => widget.set_active(active, cx),
            },
            Self::Nodes(NodeWidget::Usage(widget)) => widget.set_active(active, cx),
            Self::Nodes(NodeWidget::Section(widget)) => widget.set_active(active, cx),
            Self::Deployments(DeploymentWidget::Actions(_)) => {
                let _ = active;
                let _ = cx;
            }
            Self::Deployments(DeploymentWidget::Section(widget)) => {
                widget.set_active(active, cx);
            }
            Self::Section(widget) => widget.set_active(active, cx),
        }
    }

    pub fn shutdown(&self, cx: &mut App) {
        match self {
            Self::Pods(widget) => match widget {
                PodWidget::Usage(widget) => widget.shutdown(cx),
                PodWidget::Logs(widget) => widget.shutdown(cx),
                PodWidget::Exec(widget) => widget.shutdown(cx),
                PodWidget::List(_) => {}
                PodWidget::Section(widget) => widget.shutdown(cx),
            },
            Self::Nodes(NodeWidget::Usage(widget)) => widget.shutdown(cx),
            Self::Nodes(NodeWidget::Section(widget)) => widget.shutdown(cx),
            Self::Deployments(DeploymentWidget::Actions(widget)) => widget.shutdown(cx),
            Self::Deployments(DeploymentWidget::Section(widget)) => widget.shutdown(cx),
            Self::Section(widget) => widget.shutdown(cx),
        }
    }
}

pub fn mount_widget(
    id: WidgetId,
    descriptor: &WidgetDescriptor,
    window: &mut Window,
    cx: &mut App,
) -> Result<ResourceWidget, String> {
    Ok(match &descriptor.source {
        WidgetSource::Pod(PodWidgetSource::Usage { namespace, name }) => {
            ResourceWidget::Pods(PodWidget::Usage(PodUsageWidget::mount(
                PodUsageArgs {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    id_prefix: format!("{id}-pod-usage").into(),
                    compact: true,
                    show_header: false,
                },
                cx,
            )))
        }
        WidgetSource::Pod(PodWidgetSource::Logs {
            namespace,
            name,
            containers,
            container,
            follow,
        }) => ResourceWidget::Pods(PodWidget::Logs(PodLogsWidget::mount(
            PodLogsArgs {
                namespace: namespace.clone(),
                name: name.clone(),
                containers: containers.clone(),
                container: container.clone(),
                follow: *follow,
                id_prefix: format!("{id}-pod-logs").into(),
            },
            cx,
        ))),
        WidgetSource::Pod(PodWidgetSource::Exec {
            namespace,
            name,
            containers,
            container,
            shell,
        }) => ResourceWidget::Pods(PodWidget::Exec(PodExecWidget::mount(
            PodExecArgs {
                namespace: namespace.clone(),
                name: name.clone(),
                containers: containers.clone(),
                container: container
                    .clone()
                    .or_else(|| containers.first().cloned())
                    .unwrap_or_default(),
                shell: shell.clone(),
                id_prefix: format!("{id}-pod-exec").into(),
                auto_connect: false,
                focus_on_mount: false,
            },
            window,
            cx,
        ))),
        WidgetSource::Pod(PodWidgetSource::List { namespace }) => {
            let scope = namespace
                .clone()
                .map(NamespaceScope::One)
                .unwrap_or(NamespaceScope::All);
            let title = namespace
                .clone()
                .map(|namespace| format!("Pods · {namespace}"))
                .unwrap_or_else(|| "Pods · all namespaces".to_string());
            ResourceWidget::Pods(PodWidget::List(PodListWidget::mount(
                PodListArgs {
                    scope,
                    id_prefix: format!("{id}-pod-list").into(),
                    title: title.into(),
                    show_header: false,
                },
                window,
                cx,
            )))
        }
        WidgetSource::Pod(PodWidgetSource::Section {
            namespace,
            name,
            section,
        }) => ResourceWidget::Pods(PodWidget::Section(PodSectionWidget::mount(
            format!("{id}-pod-section"),
            namespace.clone(),
            name.clone(),
            *section,
            cx,
        ))),
        WidgetSource::Node(NodeWidgetSource::Usage { name }) => {
            ResourceWidget::Nodes(NodeWidget::Usage(NodeUsageWidget::mount(
                NodeUsageArgs {
                    node: name.clone(),
                    id_prefix: format!("{id}-node-usage").into(),
                    compact: true,
                    show_header: false,
                },
                cx,
            )))
        }
        WidgetSource::Node(NodeWidgetSource::Section { name, section }) => {
            ResourceWidget::Nodes(NodeWidget::Section(NodeSectionWidget::mount(
                format!("{id}-node-section"),
                name.clone(),
                *section,
                cx,
            )))
        }
        WidgetSource::Deployment(DeploymentWidgetSource::Actions { namespace, name }) => {
            ResourceWidget::Deployments(DeploymentWidget::Actions(DeploymentActionsWidget::mount(
                DeploymentActionsArgs {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    id_prefix: format!("{id}-deployment-actions").into(),
                },
                cx,
            )))
        }
        WidgetSource::Deployment(DeploymentWidgetSource::Section {
            namespace,
            name,
            section,
        }) => {
            ResourceWidget::Deployments(DeploymentWidget::Section(DeploymentSectionWidget::mount(
                format!("{id}-deployment-section"),
                namespace.clone(),
                name.clone(),
                *section,
                cx,
            )))
        }
        WidgetSource::Section(source) => ResourceWidget::Section(ResourceSectionWidget::mount(
            format!("{id}-section"),
            ResourceSectionSource {
                kind: source.kind,
                namespace: source.namespace.clone(),
                name: source.name.clone(),
                section: source.section.clone(),
            },
            cx,
        )),
    })
}

pub fn widget_title(descriptor: &WidgetDescriptor) -> String {
    match &descriptor.source {
        WidgetSource::Pod(PodWidgetSource::Usage { namespace, name }) => {
            format!("Pod usage · {namespace}/{name}")
        }
        WidgetSource::Pod(PodWidgetSource::Logs {
            namespace, name, ..
        }) => {
            format!("Logs · {namespace}/{name}")
        }
        WidgetSource::Pod(PodWidgetSource::Exec {
            namespace, name, ..
        }) => {
            format!("Exec · {namespace}/{name}")
        }
        WidgetSource::Pod(PodWidgetSource::List { namespace }) => namespace
            .as_ref()
            .map(|namespace| format!("Pods · {namespace}"))
            .unwrap_or_else(|| "Pods · all namespaces".to_string()),
        WidgetSource::Pod(PodWidgetSource::Section {
            namespace,
            name,
            section,
        }) => format!("{} · {namespace}/{name}", section.title()),
        WidgetSource::Node(NodeWidgetSource::Usage { name }) => {
            format!("Node usage · {name}")
        }
        WidgetSource::Node(NodeWidgetSource::Section { name, section }) => {
            format!("{} · {name}", section.title())
        }
        WidgetSource::Deployment(DeploymentWidgetSource::Actions { namespace, name }) => {
            format!("Deployment actions · {namespace}/{name}")
        }
        WidgetSource::Deployment(DeploymentWidgetSource::Section {
            namespace,
            name,
            section,
        }) => format!("{} · {namespace}/{name}", section.title()),
        WidgetSource::Section(source) => format!(
            "{} · {}/{}",
            source.section.title(),
            source.namespace,
            source.name
        ),
    }
}
