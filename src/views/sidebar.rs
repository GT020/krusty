use super::Route;
use crate::k8s::client;
use crate::k8s::context::{
    get_current_context, set_current_context, set_current_namespace, use_is_connected,
    CURRENT_NAMESPACE,
};
use dioxus::prelude::*;
use dioxus::router::{Link, Outlet};

#[derive(Clone, PartialEq)]
pub enum ResourceKind {
    Pods,
    Deployments,
    Services,
    ConfigMaps,
    Secrets,
    PersistentVolumeClaims,
    StatefulSets,
    DaemonSets,
    Ingresses,
    Jobs,
    CronJobs,
    ServiceAccounts,
    Endpoints,
}

impl ResourceKind {
    pub fn label(&self) -> &str {
        match self {
            ResourceKind::Pods => "Pods",
            ResourceKind::Deployments => "Deployments",
            ResourceKind::Services => "Services",
            ResourceKind::ConfigMaps => "ConfigMaps",
            ResourceKind::Secrets => "Secrets",
            ResourceKind::PersistentVolumeClaims => "PersistentVolumeClaims",
            ResourceKind::StatefulSets => "StatefulSets",
            ResourceKind::DaemonSets => "DaemonSets",
            ResourceKind::Ingresses => "Ingresses",
            ResourceKind::Jobs => "Jobs",
            ResourceKind::CronJobs => "CronJobs",
            ResourceKind::ServiceAccounts => "ServiceAccounts",
            ResourceKind::Endpoints => "Endpoints",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            ResourceKind::Pods => "/pods",
            ResourceKind::Deployments => "/deployments",
            ResourceKind::Services => "/services",
            ResourceKind::ConfigMaps => "/configmaps",
            ResourceKind::Secrets => "/secrets",
            ResourceKind::PersistentVolumeClaims => "/persistentvolumeclaims",
            ResourceKind::StatefulSets => "/statefulsets",
            ResourceKind::DaemonSets => "/daemonsets",
            ResourceKind::Ingresses => "/ingresses",
            ResourceKind::Jobs => "/jobs",
            ResourceKind::CronJobs => "/cronjobs",
            ResourceKind::ServiceAccounts => "/serviceaccounts",
            ResourceKind::Endpoints => "/endpoints",
        }
    }
}

#[component]
pub fn Sidebar() -> Element {
    let is_connected = use_is_connected();
    let current_ns = CURRENT_NAMESPACE.signal().read().clone();
    let current_ctx = get_current_context();

    let namespaces =
        use_resource(|| async move { client::list_namespaces().await.unwrap_or_default() });

    let namespace_list: Vec<String> = namespaces()
        .as_ref()
        .map(|list| list.iter().map(|n| n.name.clone()).collect())
        .unwrap_or_default();

    let contexts_resource = use_resource(|| async move { client::list_contexts() });
    let context_list: Vec<String> = contexts_resource()
        .map(|mut ctxs| {
            if ctxs.is_empty() {
                ctxs.push("default".to_string());
            }
            ctxs
        })
        .unwrap_or_else(|| vec!["default".to_string()]);

    let ctx_for_effect = current_ctx.clone();
    let ctx_list_for_effect = context_list.clone();
    use_effect(move || {
        if ctx_for_effect.is_empty() && !ctx_list_for_effect.is_empty() {
            set_current_context(ctx_list_for_effect[0].clone());
        }
    });

    let display_context = if current_ctx.is_empty() {
        context_list
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    } else {
        current_ctx.clone()
    };

    rsx! {
        div {
            class: "flex h-screen",
            div {
                class: "w-64 bg-gray-900 text-white flex flex-col shrink-0 h-full overflow-hidden",
                div {
                    class: "p-4 border-b border-gray-700 shrink-0",
                    Link {
                        to: "/",
                        class: "text-lg font-bold text-white hover:text-gray-300",
                        "Krusty"
                    }
                }
                div {
                    class: "p-4 shrink-0 border-b border-gray-800",
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider", "Context" }
                    div { class: "relative",
                        select {
                            class: "w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-white",
                            style: "color: black; background-color: #374151;",
                            value: "{display_context}",
                            onchange: move |e| {
                                let new_ctx = e.value();
                                set_current_context(new_ctx.clone());
                            },
                            for ctx in context_list.iter() {
                                option {
                                    style: "background-color: #374151; color: black;",
                                    value: "{ctx}",
                                    "{ctx}"
                                }
                            }
                        }
                        div { class: "absolute inset-y-0 right-0 flex items-center pr-2 pointer-events-none",
                            svg {
                                class: "w-4 h-4 text-gray-400",
                                fill: "none",
                                stroke: "currentColor",
                                view_box: "0 0 24 24",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    stroke_width: "2",
                                    d: "M19 9l-7 7-7-7"
                                }
                            }
                        }
                    }
                }
                div {
                    class: "p-4 shrink-0",
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider", "Namespace" }
                    select {
                        class: "w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-black",
                        style: "color: black; background-color: #374151;",
                        value: "{current_ns}",
                        onchange: move |e| {
                            set_current_namespace(e.value());
                        },
                        option {
                            style: "background-color: #374151; color: black;",
                            value: "",
                            "All Namespaces"
                        }
                        for ns in namespace_list.iter() {
                            option {
                                style: "background-color: #374151; color: black;",
                                value: "{ns}",
                                "{ns}"
                            }
                        }
                    }
                }
                div {
                    class: "flex-1 p-4 overflow-y-auto",
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 uppercase tracking-wider", "Workloads" }
                    ul {
                        class: "space-y-1",
                        ResourceLink { kind: ResourceKind::Pods }
                        ResourceLink { kind: ResourceKind::Deployments }
                        ResourceLink { kind: ResourceKind::StatefulSets }
                        ResourceLink { kind: ResourceKind::DaemonSets }
                        ResourceLink { kind: ResourceKind::Jobs }
                        ResourceLink { kind: ResourceKind::CronJobs }
                    }
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 mt-4 uppercase tracking-wider", "Networking" }
                    ul {
                        class: "space-y-1",
                        ResourceLink { kind: ResourceKind::Services }
                        ResourceLink { kind: ResourceKind::Ingresses }
                        ResourceLink { kind: ResourceKind::Endpoints }
                    }
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 mt-4 uppercase tracking-wider", "Config" }
                    ul {
                        class: "space-y-1",
                        ResourceLink { kind: ResourceKind::ConfigMaps }
                        ResourceLink { kind: ResourceKind::Secrets }
                        ResourceLink { kind: ResourceKind::ServiceAccounts }
                    }
                    h3 { class: "text-xs font-semibold text-gray-400 mb-2 mt-4 uppercase tracking-wider", "Storage" }
                    ul {
                        class: "space-y-1",
                        ResourceLink { kind: ResourceKind::PersistentVolumeClaims }
                    }
                }
                div {
                    class: "p-4 border-t border-gray-700 shrink-0",
                    if is_connected {
                        div { class: "flex items-center text-green-400 text-sm",
                            div { class: "w-2 h-2 bg-green-400 rounded-full mr-2" }
                            "Connected"
                        }
                        div { class: "text-xs text-gray-500 mt-1 truncate", "Context: {display_context}" }
                    } else {
                        div { class: "flex items-center text-red-400 text-sm",
                            div { class: "w-2 h-2 bg-red-400 rounded-full mr-2" }
                            "Disconnected"
                        }
                    }
                }
            }
            div {
                class: "flex-1 bg-gray-100 overflow-auto",
                Outlet::<Route> {}
            }
        }
    }
}

#[component]
fn ResourceLink(kind: ResourceKind) -> Element {
    use dioxus::router::Link;
    let path = kind.path();
    let label = kind.label();

    rsx! {
        li {
            Link {
                to: path,
                class: "block px-3 py-2 rounded hover:bg-gray-700 transition-colors text-white text-sm",
                "{label}"
            }
        }
    }
}
