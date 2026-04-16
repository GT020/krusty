use crate::k8s::client;
use crate::k8s::context::{get_current_namespace, use_is_connected};
use dioxus::prelude::*;
use dioxus::router::Link;

#[derive(Clone, PartialEq)]
pub struct ResourceStats {
    pub pods: usize,
    pub deployments: usize,
    pub services: usize,
    pub configmaps: usize,
    pub secrets: usize,
    pub namespaces: usize,
    pub statefulsets: usize,
    pub daemonsets: usize,
    pub jobs: usize,
    pub cronjobs: usize,
    pub ingresses: usize,
    pub persistentvolumeclaims: usize,
}

#[component]
pub fn Home() -> Element {
    let is_connected = use_is_connected();

    let mut stats = use_signal(|| ResourceStats {
        pods: 0,
        deployments: 0,
        services: 0,
        configmaps: 0,
        secrets: 0,
        namespaces: 0,
        statefulsets: 0,
        daemonsets: 0,
        jobs: 0,
        cronjobs: 0,
        ingresses: 0,
        persistentvolumeclaims: 0,
    });

    use_effect(move || {
        let ns = get_current_namespace();
        let ns_for_api = ns.clone();

        spawn(async move {
            let namespace = if ns_for_api.is_empty() {
                None
            } else {
                Some(ns_for_api.as_str())
            };

            let (
                pods,
                deployments,
                services,
                configmaps,
                secrets,
                namespaces,
                statefulsets,
                daemonsets,
                jobs,
                cronjobs,
                ingresses,
                pvcs,
            ) = tokio::join!(
                client::list_pods(namespace),
                client::list_deployments(namespace),
                client::list_services(namespace),
                client::list_configmaps(namespace),
                client::list_secrets(namespace),
                client::list_namespaces(),
                client::list_statefulsets(namespace),
                client::list_daemonsets(namespace),
                client::list_jobs(namespace),
                client::list_cronjobs(namespace),
                client::list_ingresses(namespace),
                client::list_persistentvolumeclaims(namespace),
            );

            stats.write().pods = pods.map(|v| v.len()).unwrap_or(0);
            stats.write().deployments = deployments.map(|v| v.len()).unwrap_or(0);
            stats.write().services = services.map(|v| v.len()).unwrap_or(0);
            stats.write().configmaps = configmaps.map(|v| v.len()).unwrap_or(0);
            stats.write().secrets = secrets.map(|v| v.len()).unwrap_or(0);
            stats.write().namespaces = namespaces.map(|v| v.len()).unwrap_or(0);
            stats.write().statefulsets = statefulsets.map(|v| v.len()).unwrap_or(0);
            stats.write().daemonsets = daemonsets.map(|v| v.len()).unwrap_or(0);
            stats.write().jobs = jobs.map(|v| v.len()).unwrap_or(0);
            stats.write().cronjobs = cronjobs.map(|v| v.len()).unwrap_or(0);
            stats.write().ingresses = ingresses.map(|v| v.len()).unwrap_or(0);
            stats.write().persistentvolumeclaims = pvcs.map(|v| v.len()).unwrap_or(0);
        });
    });

    let current_stats = stats.read().clone();

    rsx! {
        div {
            class: "flex-1 p-6 bg-white overflow-auto",
            div { class: "mb-6 flex justify-between items-center",
                h1 { class: "text-2xl font-bold", "Cluster Overview" }
                button {
                    class: "px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700",
                    onclick: move |_| {
                        let current_stats = stats.read().clone();
                        *stats.write() = ResourceStats {
                            pods: current_stats.pods,
                            deployments: current_stats.deployments,
                            services: current_stats.services,
                            configmaps: current_stats.configmaps,
                            secrets: current_stats.secrets,
                            namespaces: current_stats.namespaces,
                            statefulsets: current_stats.statefulsets,
                            daemonsets: current_stats.daemonsets,
                            jobs: current_stats.jobs,
                            cronjobs: current_stats.cronjobs,
                            ingresses: current_stats.ingresses,
                            persistentvolumeclaims: current_stats.persistentvolumeclaims,
                        };
                    },
                    "Refresh"
                }
            }
            if !is_connected {
                div { class: "text-red-500 text-center py-8", "Not connected to cluster" }
            } else {
                div { class: "space-y-6",
                    div {
                        h2 { class: "text-lg font-semibold text-gray-700 mb-3", "Workloads" }
                        div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                            StatCard { label: "Pods", count: current_stats.pods, color: "blue", path: "/pods" }
                            StatCard { label: "Deployments", count: current_stats.deployments, color: "purple", path: "/deployments" }
                            StatCard { label: "StatefulSets", count: current_stats.statefulsets, color: "indigo", path: "/statefulsets" }
                            StatCard { label: "DaemonSets", count: current_stats.daemonsets, color: "cyan", path: "/daemonsets" }
                            StatCard { label: "Jobs", count: current_stats.jobs, color: "pink", path: "/jobs" }
                            StatCard { label: "CronJobs", count: current_stats.cronjobs, color: "rose", path: "/cronjobs" }
                        }
                    }
                    div {
                        h2 { class: "text-lg font-semibold text-gray-700 mb-3", "Networking" }
                        div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                            StatCard { label: "Services", count: current_stats.services, color: "orange", path: "/services" }
                            StatCard { label: "Ingresses", count: current_stats.ingresses, color: "amber", path: "/ingresses" }
                        }
                    }
                    div {
                        h2 { class: "text-lg font-semibold text-gray-700 mb-3", "Config & Storage" }
                        div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                            StatCard { label: "ConfigMaps", count: current_stats.configmaps, color: "yellow", path: "/configmaps" }
                            StatCard { label: "Secrets", count: current_stats.secrets, color: "red", path: "/secrets" }
                            StatCard { label: "PVCs", count: current_stats.persistentvolumeclaims, color: "emerald", path: "/persistentvolumeclaims" }
                        }
                    }
                    div {
                        h2 { class: "text-lg font-semibold text-gray-700 mb-3", "Cluster" }
                        div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                            StatCard { label: "Namespaces", count: current_stats.namespaces, color: "green", path: "/namespaces" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn StatCard(label: String, count: usize, color: String, path: String) -> Element {
    let color_class = match color.as_str() {
        "blue" => "bg-blue-500 hover:bg-blue-600",
        "green" => "bg-green-500 hover:bg-green-600",
        "purple" => "bg-purple-500 hover:bg-purple-600",
        "orange" => "bg-orange-500 hover:bg-orange-600",
        "yellow" => "bg-yellow-500 hover:bg-yellow-600",
        "red" => "bg-red-500 hover:bg-red-600",
        "indigo" => "bg-indigo-500 hover:bg-indigo-600",
        "cyan" => "bg-cyan-500 hover:bg-cyan-600",
        "pink" => "bg-pink-500 hover:bg-pink-600",
        "rose" => "bg-rose-500 hover:bg-rose-600",
        "amber" => "bg-amber-500 hover:bg-amber-600",
        "emerald" => "bg-emerald-500 hover:bg-emerald-600",
        _ => "bg-gray-500 hover:bg-gray-600",
    };

    rsx! {
        Link {
            to: path.as_str(),
            class: "{color_class} text-white p-4 rounded-lg shadow cursor-pointer transition-colors block",
            div { class: "text-2xl font-bold", "{count}" }
            div { class: "text-sm opacity-90", "{label}" }
        }
    }
}
