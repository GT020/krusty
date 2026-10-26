//! Repository for Nodes: live list/watch plus one-shot detail and metrics reads.

use std::collections::BTreeMap;
use std::time::Duration;

use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, DynamicObject};
use kube::core::{ApiResource, GroupVersionKind};
use kube::{Client, ResourceExt};
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::resources::node::detail::{
    NodeAddress, NodeCondition, NodeDetail, NodeDetailUpdate, NodeImage, NodeSystemInfo, NodeTaint,
    NodeUsage,
};
use crate::models::{NamespaceScope, ResourceKind};
use gpui_kit::App;

use crate::repository::{Subscription, subscribe_kind};

const METRICS_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Subscribe to the live Nodes stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::Nodes, NamespaceScope::All)
}

/// A live detail stream for one node. Dropping it cancels the background task.
pub struct NodeDetailSubscription {
    updates: mpsc::UnboundedReceiver<NodeDetailUpdate>,
    cancel: watch::Sender<bool>,
}

impl NodeDetailSubscription {
    pub async fn next(&mut self) -> Option<NodeDetailUpdate> {
        self.updates.recv().await
    }

    /// A handle that stops this stream when sent `true`.
    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for NodeDetailSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

/// Start loading a node's full description and live usage samples.
pub fn subscribe_detail(cx: &mut App, node: &str) -> Result<NodeDetailSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_node_detail(
        client,
        node.to_string(),
        updates_tx,
        cancel_rx,
    ));
    Ok(NodeDetailSubscription { updates, cancel })
}

async fn watch_node_detail(
    client: Client,
    node: String,
    updates: mpsc::UnboundedSender<NodeDetailUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Node> = Api::all(client.clone());
    let fetched = tokio::select! {
        _ = cancel.changed() => return,
        result = api.get(&node) => result,
    };
    let fetched = match fetched {
        Ok(node) => node,
        Err(error) => {
            let _ = updates.send(NodeDetailUpdate::Detail {
                node,
                result: Err(format!("Could not load node details: {error}")),
            });
            return;
        }
    };
    let detail = detail_from_node(&fetched);
    let cpu_capacity = detail
        .allocatable
        .get("cpu")
        .and_then(|quantity| parse_quantity(quantity))
        .map(|cores| (cores * 1000.0).round() as u64);
    let memory_capacity = detail
        .allocatable
        .get("memory")
        .and_then(|quantity| parse_quantity(quantity))
        .map(|bytes| bytes.max(0.0).round() as u64);
    if updates
        .send(NodeDetailUpdate::Detail {
            node: node.clone(),
            result: Ok(Box::new(detail)),
        })
        .is_err()
    {
        return;
    }

    loop {
        let fetched = tokio::select! {
            _ = cancel.changed() => return,
            result = fetch_usage(&client, &node, cpu_capacity, memory_capacity) => result,
        };
        if updates
            .send(NodeDetailUpdate::Usage {
                node: node.clone(),
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
    node: &str,
    cpu_capacity_millicores: Option<u64>,
    memory_capacity_bytes: Option<u64>,
) -> Result<NodeUsage, String> {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "NodeMetrics");
    let resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(client.clone(), &resource);
    let metrics = api.get(node).await.map_err(|error| {
        format!("Node metrics are unavailable. Install metrics-server to see live usage. ({error})")
    })?;
    let usage = metrics
        .data
        .get("usage")
        .ok_or_else(|| "Node metrics did not include usage.".to_string())?;
    let cpu = usage.get("cpu").and_then(|value| value.as_str());
    let memory = usage.get("memory").and_then(|value| value.as_str());

    Ok(NodeUsage {
        observed_at: jiff::Timestamp::now().as_second(),
        cpu_used_millicores: cpu
            .and_then(parse_quantity)
            .map(|cores| (cores * 1000.0).round().max(0.0) as u64),
        cpu_capacity_millicores,
        memory_used_bytes: memory
            .and_then(parse_quantity)
            .map(|bytes| bytes.max(0.0).round() as u64),
        memory_capacity_bytes,
    })
}

fn detail_from_node(node: &Node) -> NodeDetail {
    let metadata = &node.metadata;
    let spec = node.spec.as_ref();
    let status = node.status.as_ref();

    let labels: BTreeMap<String, String> = metadata.labels.clone().unwrap_or_default();
    let annotations: BTreeMap<String, String> = metadata.annotations.clone().unwrap_or_default();
    let mut roles: Vec<String> = labels
        .iter()
        .filter_map(|(key, value)| {
            if key == "kubernetes.io/role" {
                Some(value.clone())
            } else {
                key.strip_prefix("node-role.kubernetes.io/")
                    .filter(|role| !role.is_empty())
                    .map(str::to_string)
            }
        })
        .collect();
    roles.sort();
    roles.dedup();

    let conditions: Vec<NodeCondition> = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| conditions.iter().map(condition_from_node).collect())
        .unwrap_or_default();
    let addresses: Vec<NodeAddress> = status
        .and_then(|status| status.addresses.as_ref())
        .map(|addresses| {
            addresses
                .iter()
                .map(|address| NodeAddress {
                    type_: address.type_.clone(),
                    address: address.address.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let taints: Vec<NodeTaint> = spec
        .and_then(|spec| spec.taints.as_ref())
        .map(|taints| {
            taints
                .iter()
                .map(|taint| NodeTaint {
                    key: taint.key.clone(),
                    value: taint.value.clone().unwrap_or_default(),
                    effect: taint.effect.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let images: Vec<NodeImage> = status
        .and_then(|status| status.images.as_ref())
        .map(|images| {
            images
                .iter()
                .map(|image| NodeImage {
                    names: image.names.clone().unwrap_or_default(),
                    size_bytes: image.size_bytes.unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let node_info = status
        .and_then(|status| status.node_info.as_ref())
        .map(|info| NodeSystemInfo {
            machine_id: info.machine_id.clone(),
            system_uuid: info.system_uuid.clone(),
            boot_id: info.boot_id.clone(),
            kernel_version: info.kernel_version.clone(),
            os_image: info.os_image.clone(),
            container_runtime_version: info.container_runtime_version.clone(),
            kubelet_version: info.kubelet_version.clone(),
            kube_proxy_version: info.kube_proxy_version.clone(),
            operating_system: info.operating_system.clone(),
            architecture: info.architecture.clone(),
        })
        .unwrap_or_default();

    let pod_cidrs = spec
        .map(|spec| spec.pod_cidrs.clone().unwrap_or_default())
        .unwrap_or_default();

    NodeDetail {
        name: node.name_any(),
        uid: node.uid().unwrap_or_default(),
        resource_version: node.resource_version().unwrap_or_default(),
        created_at: node.creation_timestamp().map(|time| time.0.as_second()),
        roles,
        labels,
        annotations,
        unschedulable: spec.and_then(|spec| spec.unschedulable).unwrap_or(false),
        provider_id: spec
            .and_then(|spec| spec.provider_id.clone())
            .unwrap_or_default(),
        pod_cidr: spec
            .and_then(|spec| spec.pod_cidr.clone())
            .unwrap_or_default(),
        pod_cidrs,
        addresses,
        capacity: quantities(status.and_then(|status| status.capacity.as_ref())),
        allocatable: quantities(status.and_then(|status| status.allocatable.as_ref())),
        node_info,
        conditions,
        taints,
        images,
        raw_json: serde_json::to_string_pretty(node).unwrap_or_default(),
    }
}

fn condition_from_node(condition: &k8s_openapi::api::core::v1::NodeCondition) -> NodeCondition {
    NodeCondition {
        type_: condition.type_.clone(),
        status: condition.status.clone(),
        reason: condition.reason.clone().unwrap_or_default(),
        message: condition.message.clone().unwrap_or_default(),
        last_heartbeat: condition
            .last_heartbeat_time
            .as_ref()
            .map(|time| time.0.as_second()),
        last_transition: condition
            .last_transition_time
            .as_ref()
            .map(|time| time.0.as_second()),
    }
}

fn quantities(
    values: Option<&BTreeMap<String, k8s_openapi::apimachinery::pkg::api::resource::Quantity>>,
) -> BTreeMap<String, String> {
    values
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.clone(), value.0.clone()))
                .collect()
        })
        .unwrap_or_default()
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
        assert_quantity("2", 2.0);
        assert_quantity("1Ki", 1024.0);
        assert_quantity("2Gi", 2.0 * 1024.0 * 1024.0 * 1024.0);
        assert_quantity("1e3", 1000.0);
        assert_eq!(parse_quantity(""), None);
    }
}
