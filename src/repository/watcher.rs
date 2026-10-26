//! Kubernetes list + watch loop for one resource kind / namespace scope.
//!
//! This module runs entirely on the cluster tokio runtime. It owns the
//! [`ResourceStore`] and only publishes immutable snapshots to the UI, throttled
//! to `FLUSH_INTERVAL` or every `FLUSH_AT` events, whichever comes first.

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use futures::TryStreamExt as _;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{
    ConfigMap, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{Api, ListParams, ResourceExt, WatchEvent, WatchParams};
use kube::core::NamespaceResourceScope;
use kube::{Client, Resource};
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::models::{
    Item, NamespaceScope, ObjectKey, ResourceKind, ResourceSnapshot, ResourceStore, SnapshotPhase,
};

const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const FLUSH_AT: usize = 256;
const RETRY_DELAY: Duration = Duration::from_millis(750);

/// Spawned once per subscription. Keep it generic over the store; the typed
/// kind dispatch lives in `run`.
pub async fn run(
    client: Client,
    kind: ResourceKind,
    scope: NamespaceScope,
    tx: watch::Sender<Arc<ResourceSnapshot>>,
    cancel: watch::Receiver<bool>,
    refresh: watch::Receiver<u64>,
) {
    let mut task = WatchTask {
        client,
        tx,
        cancel,
        refresh,
        store: ResourceStore::new(),
    };
    match kind {
        ResourceKind::Pods => task.run_namespaced::<Pod, _>(scope, pod_item).await,
        ResourceKind::Deployments => {
            task.run_namespaced::<Deployment, _>(scope, deployment_item)
                .await
        }
        ResourceKind::DaemonSets => {
            task.run_namespaced::<DaemonSet, _>(scope, daemon_set_item)
                .await
        }
        ResourceKind::Jobs => task.run_namespaced::<Job, _>(scope, job_item).await,
        ResourceKind::Services => task.run_namespaced::<Service, _>(scope, service_item).await,
        ResourceKind::Ingresses => task.run_namespaced::<Ingress, _>(scope, ingress_item).await,
        ResourceKind::PersistentVolumeClaims => {
            task.run_namespaced::<PersistentVolumeClaim, _>(scope, pvc_item)
                .await
        }
        ResourceKind::ConfigMaps => {
            task.run_namespaced::<ConfigMap, _>(scope, config_map_item)
                .await
        }
        ResourceKind::Secrets => task.run_namespaced::<Secret, _>(scope, secret_item).await,
        ResourceKind::PersistentVolumes => task.run_cluster::<PersistentVolume, _>(pv_item).await,
        ResourceKind::Nodes => task.run_cluster::<Node, _>(node_item).await,
    }
}

pub fn namespace_names(namespaces: Vec<Namespace>) -> Vec<String> {
    let mut names: Vec<_> = namespaces
        .into_iter()
        .filter_map(|namespace| namespace.metadata.name)
        .collect();
    names.sort_unstable();
    names
}

struct WatchTask {
    client: Client,
    tx: watch::Sender<Arc<ResourceSnapshot>>,
    cancel: watch::Receiver<bool>,
    refresh: watch::Receiver<u64>,
    store: ResourceStore,
}

impl WatchTask {
    async fn run_namespaced<K, F>(&mut self, scope: NamespaceScope, map: F)
    where
        K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
            + Clone
            + Debug
            + DeserializeOwned
            + Send
            + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        let api = match &scope {
            NamespaceScope::All => Api::<K>::all(self.client.clone()),
            NamespaceScope::One(namespace) => Api::<K>::namespaced(self.client.clone(), namespace),
        };
        self.run_stream(api, map).await;
    }

    async fn run_cluster<K, F>(&mut self, map: F)
    where
        K: Resource<DynamicType = ()> + Clone + Debug + DeserializeOwned + Send + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        let api = Api::<K>::all(self.client.clone());
        self.run_stream(api, map).await;
    }

    async fn run_stream<K, F>(&mut self, api: Api<K>, map: F)
    where
        K: Resource<DynamicType = ()> + Clone + Debug + DeserializeOwned + Send + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        let list_params = ListParams::default();
        let watch_params = WatchParams::default().timeout(280);
        let mut resource_version = String::new();
        let mut need_list = true;
        let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;

        loop {
            if self.cancel.has_changed().is_err() {
                return;
            }
            if need_list {
                let listed = tokio::select! {
                    _ = self.cancel.changed() => return,
                    result = api.list(&list_params) => result,
                };
                match listed {
                    Ok(list) => {
                        resource_version = list.metadata.resource_version.unwrap_or_default();
                        self.store.reset(list.items.iter().map(map));
                        self.publish(SnapshotPhase::Ready);
                        need_list = false;
                    }
                    Err(error) => {
                        tracing::warn!(%error, ?api, "repository: list failed");
                        self.publish(SnapshotPhase::Error(format!("list failed: {error}")));
                        if !self.sleep(RETRY_DELAY).await {
                            return;
                        }
                        continue;
                    }
                }
            }

            let watched = tokio::select! {
                _ = self.cancel.changed() => return,
                result = api.watch(&watch_params, &resource_version) => result,
            };
            let mut stream = match watched {
                Ok(stream) => Box::pin(stream),
                Err(error) => {
                    tracing::warn!(%error, "repository: watch setup failed");
                    if !self.sleep(RETRY_DELAY).await {
                        return;
                    }
                    need_list = true;
                    continue;
                }
            };

            let mut relist = false;
            loop {
                tokio::select! {
                    _ = self.cancel.changed() => return,
                    _ = self.refresh.changed() => {
                        self.refresh.borrow_and_update();
                        relist = true;
                        break;
                    }
                    _ = ticker.tick() => {
                        if self.store.pending() > 0 {
                            self.publish(SnapshotPhase::Ready);
                        }
                    }
                    event = stream.try_next() => {
                        match event {
                            Ok(Some(WatchEvent::Added(object) | WatchEvent::Modified(object))) => {
                                let item = map(&object);
                                if !item.resource_version.is_empty() {
                                    resource_version.clone_from(&item.resource_version);
                                }
                                self.store.upsert(item);
                            }
                            Ok(Some(WatchEvent::Deleted(object))) => {
                                if let Some(version) = object.resource_version() {
                                    resource_version = version;
                                }
                                self.store.remove(&ObjectKey::new(
                                    object.namespace().unwrap_or_default(),
                                    object.name_any(),
                                ));
                            }
                            Ok(Some(WatchEvent::Bookmark(bookmark))) => {
                                resource_version = bookmark.metadata.resource_version;
                            }
                            Ok(Some(WatchEvent::Error(status))) => {
                                tracing::warn!(
                                    code = status.code,
                                    message = %status.message,
                                    "repository: watch error event"
                                );
                                relist = status.code == 410
                                    || status.reason == "Expired"
                                    || status.reason == "Gone";
                                break;
                            }
                            Ok(None) => {
                                tracing::debug!("repository: watch stream ended, resuming");
                                break;
                            }
                            Err(error) => {
                                tracing::warn!(%error, "repository: watch stream failed");
                                if !self.sleep(RETRY_DELAY).await {
                                    return;
                                }
                                break;
                            }
                        }
                        if self.store.pending() >= FLUSH_AT {
                            self.publish(SnapshotPhase::Ready);
                        }
                    }
                }
            }

            if self.store.pending() > 0 {
                self.publish(SnapshotPhase::Ready);
            }
            if relist {
                need_list = true;
            } else if !self.sleep(Duration::from_millis(250)).await {
                return;
            }
        }
    }

    fn publish(&mut self, phase: SnapshotPhase) {
        let snapshot = Arc::new(self.store.snapshot(phase));
        tracing::trace!(
            objects = self.store.len(),
            revision = snapshot.revision(),
            "repository: publishing snapshot"
        );
        self.tx.send_replace(snapshot);
    }

    /// Returns false when the subscription was cancelled.
    async fn sleep(&mut self, duration: Duration) -> bool {
        tokio::select! {
            _ = self.cancel.changed() => false,
            _ = tokio::time::sleep(duration) => true,
        }
    }
}

fn base_item<K>(object: &K) -> Item
where
    K: ResourceExt,
{
    let mut item = Item::new(object.name_any(), object.namespace());
    item.uid = object.uid().unwrap_or_default();
    item.resource_version = object.resource_version().unwrap_or_default();
    item.created_at = object.creation_timestamp().map(|time| time.0.as_second());
    item
}

fn version_label(labels: Option<&std::collections::BTreeMap<String, String>>) -> String {
    labels
        .and_then(|labels| labels.get("app.kubernetes.io/version").cloned())
        .unwrap_or_default()
}

fn pod_item(pod: &Pod) -> Item {
    let mut item = base_item(pod);
    let statuses = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref());
    let total = statuses.map(|statuses| statuses.len()).unwrap_or(0);
    let ready = statuses
        .map(|statuses| statuses.iter().filter(|status| status.ready).count())
        .unwrap_or(0);
    item.ready = format!("{ready}/{total}");
    item.restarts = statuses
        .map(|statuses| {
            statuses
                .iter()
                .map(|status| status.restart_count as usize)
                .sum()
        })
        .unwrap_or(0);
    item.status = pod
        .status
        .as_ref()
        .and_then(|status| status.phase.as_deref())
        .unwrap_or("unknown")
        .to_lowercase();
    let crashing = statuses.is_some_and(|statuses| {
        statuses.iter().any(|status| {
            status
                .state
                .as_ref()
                .and_then(|state| state.waiting.as_ref())
                .and_then(|waiting| waiting.reason.as_deref())
                .is_some_and(|reason| reason == "CrashLoopBackOff")
        })
    });
    if crashing {
        item.status = "crashloopbackoff".to_string();
    }
    item
}

fn deployment_item(deployment: &Deployment) -> Item {
    let mut item = base_item(deployment);
    let desired = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = deployment
        .status
        .as_ref()
        .and_then(|status| status.ready_replicas)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(deployment.metadata.labels.as_ref());
    item
}

fn daemon_set_item(daemon_set: &DaemonSet) -> Item {
    let mut item = base_item(daemon_set);
    let desired = daemon_set
        .status
        .as_ref()
        .map(|status| status.desired_number_scheduled)
        .unwrap_or(0);
    let ready = daemon_set
        .status
        .as_ref()
        .map(|status| status.number_ready)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(daemon_set.metadata.labels.as_ref());
    item
}

fn job_item(job: &Job) -> Item {
    let mut item = base_item(job);
    let want = job
        .spec
        .as_ref()
        .and_then(|spec| spec.completions)
        .unwrap_or(1);
    let done = job
        .status
        .as_ref()
        .and_then(|status| status.succeeded)
        .unwrap_or(0);
    item.ready = format!("{done}/{want}");
    item.capacity = want.to_string();
    item.status = if done >= want { "succeeded" } else { "running" }.to_string();
    item
}

fn service_item(service: &Service) -> Item {
    let mut item = base_item(service);
    let spec = service.spec.as_ref();
    item.status = spec
        .and_then(|spec| spec.type_.clone())
        .unwrap_or_else(|| "ClusterIP".to_string())
        .to_lowercase();
    item.version = spec
        .and_then(|spec| spec.ports.as_ref())
        .and_then(|ports| ports.first())
        .map(|port| port.port.to_string())
        .unwrap_or_default();
    item
}

fn ingress_item(ingress: &Ingress) -> Item {
    let mut item = base_item(ingress);
    item.status = "ingress".to_string();
    item.version = ingress
        .spec
        .as_ref()
        .and_then(|spec| spec.rules.as_ref())
        .and_then(|rules| rules.first())
        .and_then(|rule| rule.host.clone())
        .unwrap_or_default();
    item
}

fn pv_item(persistent_volume: &PersistentVolume) -> Item {
    let mut item = base_item(persistent_volume);
    item.capacity = persistent_volume
        .spec
        .as_ref()
        .and_then(|spec| spec.capacity.as_ref())
        .and_then(|capacity| capacity.get("storage"))
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.status = persistent_volume
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_default()
        .to_lowercase();
    item
}

fn pvc_item(persistent_volume_claim: &PersistentVolumeClaim) -> Item {
    let mut item = base_item(persistent_volume_claim);
    item.capacity = persistent_volume_claim
        .status
        .as_ref()
        .and_then(|status| status.capacity.as_ref())
        .and_then(|capacity| capacity.get("storage"))
        .or_else(|| {
            persistent_volume_claim
                .spec
                .as_ref()
                .and_then(|spec| spec.resources.as_ref())
                .and_then(|resources| resources.requests.as_ref())
                .and_then(|requests| requests.get("storage"))
        })
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.status = persistent_volume_claim
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_default()
        .to_lowercase();
    item
}

fn config_map_item(config_map: &ConfigMap) -> Item {
    let mut item = base_item(config_map);
    let data_keys = config_map.data.as_ref().map(|data| data.len()).unwrap_or(0);
    let binary_keys = config_map
        .binary_data
        .as_ref()
        .map(|data| data.len())
        .unwrap_or(0);
    item.status = "keys".to_string();
    item.capacity = (data_keys + binary_keys).to_string();
    item.version = config_map
        .data
        .as_ref()
        .and_then(|data| data.keys().next().cloned())
        .unwrap_or_default();
    item
}

fn secret_item(secret: &Secret) -> Item {
    let mut item = base_item(secret);
    item.status = "keys".to_string();
    item.capacity = secret
        .data
        .as_ref()
        .map(|data| data.len())
        .unwrap_or(0)
        .to_string();
    item.version = secret
        .data
        .as_ref()
        .and_then(|data| data.keys().next().cloned())
        .unwrap_or_default();
    item
}

fn node_item(node: &Node) -> Item {
    let mut item = base_item(node);
    let status = node.status.as_ref();
    let ready = status
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == "Ready")
        })
        .map(|condition| condition.status == "True")
        .unwrap_or(false);
    item.status = if ready { "ready" } else { "notready" }.to_string();
    item.capacity = status
        .and_then(|status| status.allocatable.as_ref())
        .and_then(|allocatable| allocatable.get("cpu"))
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.version = status
        .and_then(|status| status.node_info.as_ref())
        .map(|info| info.kubelet_version.clone())
        .unwrap_or_default();
    item
}
