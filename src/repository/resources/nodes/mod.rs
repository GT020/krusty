//! Repository for Nodes: live list/watch plus one-shot detail and metrics reads.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, DynamicObject, ListParams};
use kube::core::{ApiResource, GroupVersionKind};
use kube::{Client, ResourceExt};
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::resources::node::detail::{
    NodeAddress, NodeCondition, NodeDetail, NodeDetailUpdate, NodeImage, NodeSystemInfo, NodeTaint,
    NodeUsage,
};
use crate::models::{ObjectKey, ResourceKind, SnapshotPhase};
use gpui_kit::App;

use crate::repository::quantity::parse_quantity;
use crate::repository::stream::CancelOnDrop;

const METRICS_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// A live detail stream for one node. Dropping it cancels the background task.
pub type NodeDetailSubscription = CancelOnDrop<NodeDetailUpdate>;

/// Live cluster-wide NodeMetrics samples, refreshed on the metrics cadence.
pub type NodeMetricsSubscription = CancelOnDrop<Result<BTreeMap<String, NodeUsage>, String>>;

/// Poll all node usage from metrics-server. A missing metrics API is reported
/// through the stream so the overview can explain why usage is unavailable.
pub fn subscribe_metrics(cx: &mut App) -> Result<NodeMetricsSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_node_metrics(client, updates_tx, cancel_rx));
    Ok(CancelOnDrop::new(updates, cancel))
}

async fn watch_node_metrics(
    client: Client,
    updates: mpsc::UnboundedSender<Result<BTreeMap<String, NodeUsage>, String>>,
    mut cancel: watch::Receiver<bool>,
) {
    loop {
        let result = fetch_all_usage(&client).await;
        if updates.send(result).is_err() {
            return;
        }
        tokio::select! {
            _ = cancel.changed() => return,
            _ = tokio::time::sleep(METRICS_POLL_INTERVAL) => {}
        }
    }
}

async fn fetch_all_usage(client: &Client) -> Result<BTreeMap<String, NodeUsage>, String> {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "NodeMetrics");
    let resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(client.clone(), &resource);
    let list = api.list(&ListParams::default()).await.map_err(|error| {
        format!("Node metrics are unavailable. Install metrics-server to see usage. ({error})")
    })?;
    let observed_at = jiff::Timestamp::now().as_second();
    let mut usages = BTreeMap::new();
    for metrics in list.items {
        let name = metrics.name_any();
        let usage = metrics.data.get("usage");
        let cpu = usage
            .and_then(|usage| usage.get("cpu"))
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .map(|cores| (cores * 1000.0).round().max(0.0) as u64);
        let memory = usage
            .and_then(|usage| usage.get("memory"))
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .map(|bytes| bytes.max(0.0).round() as u64);
        usages.insert(
            name,
            NodeUsage {
                observed_at,
                cpu_used_millicores: cpu,
                cpu_capacity_millicores: None,
                memory_used_bytes: memory,
                memory_capacity_bytes: None,
            },
        );
    }
    Ok(usages)
}

/// Start loading a node's full description from the shared store, with one
/// `GET` fallback when the object is not cached. Usage samples remain on the
/// existing metrics-server cadence.
pub fn subscribe_detail(cx: &mut App, node: &str) -> Result<NodeDetailSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let key = ObjectKey::new("", node);
    let initial = crate::repository::store::cached_object(cx, ResourceKind::Nodes, &key);
    let snapshots = crate::repository::store::existing_store_snapshot(cx, ResourceKind::Nodes);
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_node_detail_from_store(
        client,
        node.to_string(),
        initial,
        snapshots,
        updates_tx,
        cancel_rx,
    ));
    Ok(CancelOnDrop::new(updates, cancel))
}

#[allow(clippy::too_many_arguments)]
async fn watch_node_detail_from_store(
    client: Client,
    node: String,
    initial: Option<Arc<crate::repository::store::KubeObjectRecord>>,
    mut snapshots: Option<watch::Receiver<Arc<crate::repository::store::StoreSnapshot>>>,
    updates: mpsc::UnboundedSender<NodeDetailUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    let key = ObjectKey::new("", &node);
    let mut last_version = String::new();
    let mut cpu_capacity = None;
    let mut memory_capacity = None;
    let mut saw_store_object = false;
    let mut initial_get_done = false;
    let mut ticker = tokio::time::interval(METRICS_POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    if let Some(record) = initial.as_ref()
        && let Some(object) = record.typed::<Node>()
    {
        last_version.clone_from(&record.resource_version);
        let detail = detail_from_node(object);
        (cpu_capacity, memory_capacity) = capacity_from_detail(&detail);
        if !send_node_detail(&updates, &node, detail) {
            return;
        }
        saw_store_object = true;
        initial_get_done = true;
    }

    loop {
        let snapshot = snapshots
            .as_mut()
            .map(|receiver| receiver.borrow_and_update().clone());

        if let Some(snapshot) = snapshot.as_ref() {
            if let Some(record) = snapshot.object(&key) {
                if let Some(object) = record.typed::<Node>()
                    && record.resource_version != last_version
                {
                    last_version.clone_from(&record.resource_version);
                    let detail = detail_from_node(object);
                    (cpu_capacity, memory_capacity) = capacity_from_detail(&detail);
                    if !send_node_detail(&updates, &node, detail) {
                        return;
                    }
                    saw_store_object = true;
                    initial_get_done = true;
                }
            } else if saw_store_object && matches!(snapshot.phase(), SnapshotPhase::Ready) {
                let _ = updates.send(NodeDetailUpdate::Detail {
                    node: node.clone(),
                    result: Err("This node was deleted".to_string()),
                });
                return;
            }
        }

        let store_ready = snapshot.as_ref().is_none_or(|snapshot| {
            matches!(
                snapshot.phase(),
                SnapshotPhase::Ready | SnapshotPhase::Error(_)
            )
        });

        if !initial_get_done && store_ready {
            initial_get_done = true;
            let api: Api<Node> = Api::all(client.clone());
            let fetched = tokio::select! {
                _ = cancel.changed() => return,
                result = api.get(&node) => result,
            };
            match fetched {
                Ok(object) => {
                    let detail = detail_from_node(&object);
                    (cpu_capacity, memory_capacity) = capacity_from_detail(&detail);
                    if !send_node_detail(&updates, &node, detail) {
                        return;
                    }
                }
                Err(error) => {
                    let _ = updates.send(NodeDetailUpdate::Detail {
                        node: node.clone(),
                        result: Err(format!("Could not load node details: {error}")),
                    });
                    return;
                }
            }
        }

        tokio::select! {
            _ = cancel.changed() => return,
            changed = async {
                match snapshots.as_mut() {
                    Some(receiver) => receiver.changed().await.is_ok(),
                    None => std::future::pending::<bool>().await,
                }
            } => {
                if !changed {
                    return;
                }
            }
            _ = ticker.tick() => {
                let usage = fetch_usage(&client, &node, cpu_capacity, memory_capacity).await;
                if updates
                    .send(NodeDetailUpdate::Usage {
                        node: node.clone(),
                        result: usage,
                    })
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

fn capacity_from_detail(detail: &NodeDetail) -> (Option<u64>, Option<u64>) {
    let cpu = detail
        .allocatable
        .get("cpu")
        .and_then(|quantity| parse_quantity(quantity))
        .map(|cores| (cores * 1000.0).round() as u64);
    let memory = detail
        .allocatable
        .get("memory")
        .and_then(|quantity| parse_quantity(quantity))
        .map(|bytes| bytes.max(0.0).round() as u64);
    (cpu, memory)
}

fn send_node_detail(
    updates: &mpsc::UnboundedSender<NodeDetailUpdate>,
    node: &str,
    detail: NodeDetail,
) -> bool {
    updates
        .send(NodeDetailUpdate::Detail {
            node: node.to_string(),
            result: Ok(Box::new(detail)),
        })
        .is_ok()
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
