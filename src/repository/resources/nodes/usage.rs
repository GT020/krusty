//! Live metrics polling for a single node.
//!
//! Independent of the node detail stream so a usage widget can run on its own.

use std::time::Duration;

use k8s_openapi::api::core::v1::Node;
use kube::Client;
use kube::api::{Api, DynamicObject};
use kube::core::{ApiResource, GroupVersionKind};
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::resources::node::usage::NodeUsageUpdate;
use crate::models::resources::usage::UsageSample;
use gpui_kit::App;

use super::parse_quantity;

const METRICS_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// A live usage stream for one node. Dropping it cancels the background task.
pub struct NodeUsageSubscription {
    updates: mpsc::UnboundedReceiver<NodeUsageUpdate>,
    cancel: watch::Sender<bool>,
}

impl NodeUsageSubscription {
    pub async fn next(&mut self) -> Option<NodeUsageUpdate> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for NodeUsageSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

/// Start polling metrics for one node.
pub fn subscribe_usage(cx: &mut App, node: &str) -> Result<NodeUsageSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_usage(client, node.to_string(), updates_tx, cancel_rx));
    Ok(NodeUsageSubscription { updates, cancel })
}

async fn watch_usage(
    client: Client,
    node: String,
    updates: mpsc::UnboundedSender<NodeUsageUpdate>,
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
            let _ = updates.send(NodeUsageUpdate::Sample {
                node,
                result: Err(format!("Could not load node usage: {error}")),
            });
            return;
        }
    };
    let (cpu_capacity, memory_capacity) = capacity_from_node(&fetched);

    loop {
        let fetched = tokio::select! {
            _ = cancel.changed() => return,
            result = fetch_usage(&client, &node, cpu_capacity, memory_capacity) => result,
        };
        if updates
            .send(NodeUsageUpdate::Sample {
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
) -> Result<UsageSample, String> {
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

    Ok(UsageSample {
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

fn capacity_from_node(node: &Node) -> (Option<u64>, Option<u64>) {
    let allocatable = node
        .status
        .as_ref()
        .and_then(|status| status.allocatable.as_ref());
    let cpu = allocatable
        .and_then(|allocatable| allocatable.get("cpu"))
        .and_then(|quantity| parse_quantity(&quantity.0))
        .map(|cores| (cores * 1000.0).round() as u64);
    let memory = allocatable
        .and_then(|allocatable| allocatable.get("memory"))
        .and_then(|quantity| parse_quantity(&quantity.0))
        .map(|bytes| bytes.max(0.0).round() as u64);
    (cpu, memory)
}
