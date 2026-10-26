//! Repository for Pods: live list/watch plus one-shot detail and metrics reads.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStatus, EnvVar, Pod, PodStatus, Toleration, Volume,
};
use kube::api::{Api, DynamicObject};
use kube::core::{ApiResource, GroupVersionKind};
use kube::{Client, ResourceExt};
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::pod::detail::{
    PodCondition, PodContainer, PodDetail, PodDetailUpdate, PodOwner, PodToleration, PodUsage,
    PodVolume,
};
use crate::models::{NamespaceScope, ResourceKind};
use gpui_kit::App;

use super::{Subscription, subscribe_kind};

const METRICS_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Subscribe to the live Pods stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App, scope: NamespaceScope) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::Pods, scope)
}

/// A live detail stream for one pod. Dropping it cancels the background task.
pub struct PodDetailSubscription {
    updates: mpsc::UnboundedReceiver<PodDetailUpdate>,
    cancel: watch::Sender<bool>,
}

impl PodDetailSubscription {
    pub async fn next(&mut self) -> Option<PodDetailUpdate> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for PodDetailSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

/// Start loading a pod's full description and live usage samples.
pub fn subscribe_detail(
    cx: &mut App,
    namespace: &str,
    name: &str,
) -> Result<PodDetailSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_pod_detail(
        client,
        namespace.to_string(),
        name.to_string(),
        updates_tx,
        cancel_rx,
    ));
    Ok(PodDetailSubscription { updates, cancel })
}

async fn watch_pod_detail(
    client: Client,
    namespace: String,
    name: String,
    updates: mpsc::UnboundedSender<PodDetailUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);
    let fetched = tokio::select! {
        _ = cancel.changed() => return,
        result = api.get(&name) => result,
    };
    let fetched = match fetched {
        Ok(pod) => pod,
        Err(error) => {
            let _ = updates.send(PodDetailUpdate::Detail {
                namespace,
                name,
                result: Err(format!("Could not load pod details: {error}")),
            });
            return;
        }
    };
    let detail = detail_from_pod(&fetched);
    let (cpu_capacity, memory_capacity) = capacity_from_pod(&fetched);
    if updates
        .send(PodDetailUpdate::Detail {
            namespace: namespace.clone(),
            name: name.clone(),
            result: Ok(Box::new(detail)),
        })
        .is_err()
    {
        return;
    }

    loop {
        let fetched = tokio::select! {
            _ = cancel.changed() => return,
            result = fetch_usage(&client, &namespace, &name, cpu_capacity, memory_capacity) => result,
        };
        if updates
            .send(PodDetailUpdate::Usage {
                namespace: namespace.clone(),
                name: name.clone(),
                result: fetched,
            })
            .is_err()
        {
            return;
        }
        tokio::select! {
            _ = cancel.changed() => return,
            _ = tokio::time::sleep(METRICS_POLL_INTERVAL) => {}
        }
    }
}

async fn fetch_usage(
    client: &Client,
    namespace: &str,
    name: &str,
    cpu_capacity_millicores: Option<u64>,
    memory_capacity_bytes: Option<u64>,
) -> Result<PodUsage, String> {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "PodMetrics");
    let resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &resource);
    let metrics = api.get(name).await.map_err(|error| {
        format!("Pod metrics are unavailable. Install metrics-server to see live usage. ({error})")
    })?;
    let containers = metrics
        .data
        .get("containers")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Pod metrics did not include container usage.".to_string())?;

    let mut cpu = 0.0;
    let mut memory = 0.0;
    for container in containers {
        let usage = container.get("usage");
        cpu += usage
            .and_then(|usage| usage.get("cpu"))
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .unwrap_or(0.0);
        memory += usage
            .and_then(|usage| usage.get("memory"))
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .unwrap_or(0.0);
    }

    Ok(PodUsage {
        observed_at: jiff::Timestamp::now().as_second(),
        cpu_used_millicores: Some((cpu * 1000.0).round().max(0.0) as u64),
        cpu_capacity_millicores,
        memory_used_bytes: Some(memory.max(0.0).round() as u64),
        memory_capacity_bytes,
    })
}

fn capacity_from_pod(pod: &Pod) -> (Option<u64>, Option<u64>) {
    let Some(spec) = pod.spec.as_ref() else {
        return (None, None);
    };
    let mut cpu = containers_quantity(&spec.containers, "cpu", true);
    let mut memory = containers_quantity(&spec.containers, "memory", false);
    if cpu.is_none() || memory.is_none() {
        let requests_cpu = containers_quantity(&spec.containers, "cpu", false);
        let requests_memory = containers_quantity(&spec.containers, "memory", false);
        cpu = cpu.or(requests_cpu);
        memory = memory.or(requests_memory);
    }
    (
        cpu.map(|value| (value * 1000.0).round() as u64),
        memory.map(|value| value.round() as u64),
    )
}

fn containers_quantity(containers: &[Container], resource: &str, limits: bool) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for container in containers {
        let Some(resources) = container.resources.as_ref() else {
            continue;
        };
        let map = if limits {
            resources.limits.as_ref()
        } else {
            resources.requests.as_ref()
        };
        if let Some(quantity) = map.and_then(|map| map.get(resource))
            && let Some(value) = parse_quantity(&quantity.0)
        {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

fn detail_from_pod(pod: &Pod) -> PodDetail {
    let metadata = &pod.metadata;
    let spec = pod.spec.as_ref();
    let status = pod.status.as_ref();
    let labels = metadata.labels.clone().unwrap_or_default();
    let annotations = metadata.annotations.clone().unwrap_or_default();
    let owners = metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| PodOwner {
                    kind: owner.kind.clone(),
                    name: owner.name.clone(),
                    controller: owner.controller.unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let conditions = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .map(|condition| PodCondition {
                    type_: condition.type_.clone(),
                    status: condition.status.clone(),
                    reason: condition.reason.clone().unwrap_or_default(),
                    message: condition.message.clone().unwrap_or_default(),
                    last_transition: condition
                        .last_transition_time
                        .as_ref()
                        .map(|time| time.0.as_second()),
                    last_probe: condition
                        .last_probe_time
                        .as_ref()
                        .map(|time| time.0.as_second()),
                })
                .collect()
        })
        .unwrap_or_default();
    let statuses = container_statuses(status);
    let containers = spec
        .map(|spec| {
            spec.containers
                .iter()
                .map(|container| container_detail(container, statuses.get(container.name.as_str())))
                .collect()
        })
        .unwrap_or_default();
    let init_containers = spec
        .map(|spec| {
            spec.init_containers
                .as_ref()
                .map(|containers| {
                    containers
                        .iter()
                        .map(|container| {
                            container_detail(container, statuses.get(container.name.as_str()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let volumes = spec
        .map(|spec| {
            spec.volumes
                .as_ref()
                .map(|volumes| volumes.iter().map(volume_detail).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let tolerations = spec
        .and_then(|spec| spec.tolerations.as_ref())
        .map(|tolerations| tolerations.iter().map(toleration_detail).collect())
        .unwrap_or_default();

    PodDetail {
        name: pod.name_any(),
        namespace: pod.namespace().unwrap_or_default(),
        uid: pod.uid().unwrap_or_default(),
        resource_version: pod.resource_version().unwrap_or_default(),
        created_at: pod.creation_timestamp().map(|time| time.0.as_second()),
        labels,
        annotations,
        node_name: spec
            .and_then(|spec| spec.node_name.clone())
            .unwrap_or_default(),
        service_account: spec
            .map(|spec| spec.service_account_name.clone().unwrap_or_default())
            .unwrap_or_default(),
        restart_policy: spec
            .and_then(|spec| spec.restart_policy.clone())
            .unwrap_or_default(),
        dns_policy: spec
            .and_then(|spec| spec.dns_policy.clone())
            .unwrap_or_default(),
        host_network: spec.and_then(|spec| spec.host_network).unwrap_or(false),
        hostname: spec
            .and_then(|spec| spec.hostname.clone())
            .unwrap_or_default(),
        subdomain: spec
            .and_then(|spec| spec.subdomain.clone())
            .unwrap_or_default(),
        priority_class: spec
            .and_then(|spec| spec.priority_class_name.clone())
            .unwrap_or_default(),
        priority: spec.and_then(|spec| spec.priority),
        qos_class: status
            .and_then(|status| status.qos_class.clone())
            .unwrap_or_default(),
        phase: status
            .and_then(|status| status.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        pod_ip: status
            .and_then(|status| status.pod_ip.clone())
            .unwrap_or_default(),
        pod_ips: status
            .and_then(|status| status.pod_ips.as_ref())
            .map(|ips| ips.iter().map(|ip| ip.ip.clone()).collect())
            .unwrap_or_default(),
        host_ip: status
            .and_then(|status| status.host_ip.clone())
            .unwrap_or_default(),
        nominated_node_name: status
            .and_then(|status| status.nominated_node_name.clone())
            .unwrap_or_default(),
        start_time: status
            .and_then(|status| status.start_time.as_ref())
            .map(|time| time.0.as_second()),
        conditions,
        containers,
        init_containers,
        volumes,
        tolerations,
        owners,
        raw_json: serde_json::to_string_pretty(pod).unwrap_or_default(),
    }
}

fn container_statuses(status: Option<&PodStatus>) -> HashMap<&str, &ContainerStatus> {
    status
        .and_then(|status| status.container_statuses.as_ref())
        .into_iter()
        .flatten()
        .chain(
            status
                .and_then(|status| status.init_container_statuses.as_ref())
                .into_iter()
                .flatten(),
        )
        .map(|status| (status.name.as_str(), status))
        .collect()
}

fn container_detail(container: &Container, status: Option<&&ContainerStatus>) -> PodContainer {
    let resources = container.resources.as_ref();
    let (state, state_detail) = status
        .and_then(|status| status.state.as_ref())
        .map(state_summary)
        .unwrap_or_default();
    let last_state = status
        .and_then(|status| status.last_state.as_ref())
        .map(|state| state_summary(state).0)
        .unwrap_or_default();
    PodContainer {
        name: container.name.clone(),
        image: container.image.clone().unwrap_or_default(),
        image_pull_policy: container.image_pull_policy.clone().unwrap_or_default(),
        ready: status.is_some_and(|status| status.ready),
        restart_count: status
            .map(|status| status.restart_count.max(0) as u32)
            .unwrap_or(0),
        state,
        state_detail,
        last_state,
        container_id: status
            .and_then(|status| status.container_id.clone())
            .unwrap_or_default(),
        image_id: status
            .map(|status| status.image_id.clone())
            .unwrap_or_default(),
        command: container.command.clone().unwrap_or_default(),
        args: container.args.clone().unwrap_or_default(),
        ports: container
            .ports
            .as_ref()
            .map(|ports| {
                ports
                    .iter()
                    .map(|port| {
                        let name = port
                            .name
                            .as_ref()
                            .map(|name| format!("{name} "))
                            .unwrap_or_default();
                        format!(
                            "{name}{}/{}",
                            port.container_port,
                            port.protocol.clone().unwrap_or_else(|| "TCP".to_string())
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        env: container
            .env
            .as_ref()
            .map(|env| env.iter().map(env_summary).collect())
            .unwrap_or_default(),
        mounts: container
            .volume_mounts
            .as_ref()
            .map(|mounts| {
                mounts
                    .iter()
                    .map(|mount| {
                        format!(
                            "{} → {}{}",
                            mount.name,
                            mount.mount_path,
                            if mount.read_only.unwrap_or(false) {
                                " (ro)"
                            } else {
                                ""
                            }
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        requests: resources
            .and_then(|resources| resources.requests.as_ref())
            .map(quantities)
            .unwrap_or_default(),
        limits: resources
            .and_then(|resources| resources.limits.as_ref())
            .map(quantities)
            .unwrap_or_default(),
    }
}

fn state_summary(state: &ContainerState) -> (String, String) {
    if let Some(running) = state.running.as_ref() {
        let detail = running
            .started_at
            .as_ref()
            .map(|time| format!("Started {}", time.0))
            .unwrap_or_default();
        ("Running".to_string(), detail)
    } else if let Some(waiting) = state.waiting.as_ref() {
        let reason = waiting
            .reason
            .clone()
            .unwrap_or_else(|| "Waiting".to_string());
        (
            format!("Waiting · {reason}"),
            waiting.message.clone().unwrap_or_default(),
        )
    } else if let Some(terminated) = state.terminated.as_ref() {
        let reason = terminated
            .reason
            .clone()
            .unwrap_or_else(|| "Terminated".to_string());
        (
            format!("Terminated · {reason}"),
            format!(
                "Exit code {}{}",
                terminated.exit_code,
                terminated
                    .message
                    .as_ref()
                    .map(|message| format!(" · {message}"))
                    .unwrap_or_default()
            ),
        )
    } else {
        ("Unknown".to_string(), String::new())
    }
}

fn env_summary(env: &EnvVar) -> String {
    if let Some(value) = env.value.as_ref() {
        return format!("{}={value}", env.name);
    }
    if let Some(source) = env.value_from.as_ref() {
        if let Some(field) = source.field_ref.as_ref() {
            return format!("{} (field {})", env.name, field.field_path);
        }
        if let Some(secret) = source.secret_key_ref.as_ref() {
            return format!("{} (secret {}/{})", env.name, secret.name, secret.key);
        }
        if let Some(config_map) = source.config_map_key_ref.as_ref() {
            return format!(
                "{} (configMap {}/{})",
                env.name, config_map.name, config_map.key
            );
        }
        if let Some(resource) = source.resource_field_ref.as_ref() {
            return format!("{} (resource {})", env.name, resource.resource);
        }
    }
    format!("{} (unset)", env.name)
}

fn volume_detail(volume: &Volume) -> PodVolume {
    let (type_, detail) = if let Some(source) = volume.config_map.as_ref() {
        ("ConfigMap", source.name.clone())
    } else if let Some(source) = volume.secret.as_ref() {
        ("Secret", source.secret_name.clone().unwrap_or_default())
    } else if let Some(source) = volume.persistent_volume_claim.as_ref() {
        ("PersistentVolumeClaim", source.claim_name.clone())
    } else if let Some(source) = volume.empty_dir.as_ref() {
        (
            "EmptyDir",
            source
                .medium
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        )
    } else if let Some(source) = volume.host_path.as_ref() {
        ("HostPath", source.path.clone())
    } else if let Some(source) = volume.csi.as_ref() {
        ("CSI", source.driver.clone())
    } else if let Some(source) = volume.nfs.as_ref() {
        ("NFS", format!("{}:{}", source.server, source.path))
    } else if volume.downward_api.is_some() {
        ("DownwardAPI", String::new())
    } else if volume.projected.is_some() {
        ("Projected", String::new())
    } else if volume.ephemeral.is_some() {
        ("Ephemeral", String::new())
    } else {
        ("Volume", String::new())
    };
    PodVolume {
        name: volume.name.clone(),
        type_: type_.to_string(),
        detail,
    }
}

fn toleration_detail(toleration: &Toleration) -> PodToleration {
    PodToleration {
        key: toleration.key.clone().unwrap_or_default(),
        operator: toleration
            .operator
            .clone()
            .unwrap_or_else(|| "Equal".to_string()),
        value: toleration.value.clone().unwrap_or_default(),
        effect: toleration.effect.clone().unwrap_or_default(),
        seconds: toleration.toleration_seconds,
    }
}

fn quantities(
    values: &BTreeMap<String, k8s_openapi::apimachinery::pkg::api::resource::Quantity>,
) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), value.0.clone()))
        .collect()
}

/// Parse a Kubernetes resource quantity into a plain number.
fn parse_quantity(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let split = value
        .find(|character: char| {
            !(character.is_ascii_digit()
                || character == '.'
                || character == '-'
                || character == '+')
        })
        .unwrap_or(value.len());
    let (number, suffix) = value.split_at(split);
    let number: f64 = number.parse().ok()?;
    let multiplier = match suffix {
        "" => 1.0,
        "n" => 1e-9,
        "u" => 1e-6,
        "m" => 1e-3,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        "P" => 1e15,
        "E" => 1e18,
        "Ki" => 1024.0,
        "Mi" => 1024.0 * 1024.0,
        "Gi" => 1024.0 * 1024.0 * 1024.0,
        "Ti" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "Pi" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "Ei" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            let exponent = suffix
                .strip_prefix('e')
                .or_else(|| suffix.strip_prefix('E'))?;
            let exponent: i32 = exponent.parse().ok()?;
            10f64.powi(exponent)
        }
    };
    Some(number * multiplier)
}

#[cfg(test)]
mod tests {
    use super::parse_quantity;

    fn assert_quantity(value: &str, expected: f64) {
        let parsed = parse_quantity(value).expect("quantity parses");
        assert!(
            (parsed - expected).abs() < f64::EPSILON * 8.0,
            "{value} parsed as {parsed}, expected {expected}"
        );
    }

    #[test]
    fn parses_cpu_and_memory_quantities() {
        assert_quantity("250m", 0.25);
        assert_quantity("150000000n", 0.15);
        assert_quantity("1Ki", 1024.0);
        assert_quantity("2Gi", 2.0 * 1024.0 * 1024.0 * 1024.0);
        assert_eq!(parse_quantity(""), None);
    }
}
