//! Live metrics polling for a single pod.
//!
//! This stream is independent of the pod detail stream so a usage widget can
//! run on its own, including on the dashboard with no tab open.

use std::time::Duration;

use k8s_openapi::api::core::v1::{Container, Pod};
use kube::Client;
use kube::api::{Api, DynamicObject};
use kube::core::{ApiResource, GroupVersionKind};
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::resources::pod::usage::PodUsageUpdate;
use crate::models::resources::usage::UsageSample;
use gpui_kit::App;

use super::parse_quantity;

const METRICS_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// A live usage stream for one pod. Dropping it cancels the background task.
pub struct PodUsageSubscription {
    updates: mpsc::UnboundedReceiver<PodUsageUpdate>,
    cancel: watch::Sender<bool>,
}

impl PodUsageSubscription {
    pub async fn next(&mut self) -> Option<PodUsageUpdate> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for PodUsageSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

/// Start polling metrics for one pod. The first error is delivered as an update
/// so the widget can render an inline message.
pub fn subscribe_usage(
    cx: &mut App,
    namespace: &str,
    name: &str,
) -> Result<PodUsageSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_usage(
        client,
        namespace.to_string(),
        name.to_string(),
        updates_tx,
        cancel_rx,
    ));
    Ok(PodUsageSubscription { updates, cancel })
}

async fn watch_usage(
    client: Client,
    namespace: String,
    name: String,
    updates: mpsc::UnboundedSender<PodUsageUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);
    let fetched = tokio::select! {
        _ = cancel.changed() => return,
        result = api.get(&name) => result,
    };
    let pod = match fetched {
        Ok(pod) => pod,
        Err(error) => {
            let _ = updates.send(PodUsageUpdate::Sample {
                namespace,
                name,
                result: Err(format!("Could not load pod usage: {error}")),
            });
            return;
        }
    };
    let (cpu_capacity, memory_capacity) = capacity_from_pod(&pod);

    loop {
        let fetched = tokio::select! {
            _ = cancel.changed() => return,
            result = fetch_usage(&client, &namespace, &name, cpu_capacity, memory_capacity) => result,
        };
        if updates
            .send(PodUsageUpdate::Sample {
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
) -> Result<UsageSample, String> {
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

    Ok(UsageSample {
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
