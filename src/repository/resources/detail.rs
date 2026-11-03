//! Live detail follow for every Kubernetes resource kind.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use futures::TryStreamExt as _;
use kube::ResourceExt;
use kube::api::{Api, DynamicObject, WatchEvent, WatchParams};
use kube::core::{ApiResource, GroupVersionKind};
use serde_json::Value;
use tokio::sync::{broadcast, watch};

use crate::cluster::ClusterState;
use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailUpdate};
use crate::repository::retry::{self, Retry, RetryAction};
use gpui_kit::App;

const DETAIL_WATCH_TIMEOUT_SECS: u32 = 280;
const DETAIL_RECONNECT_DELAY: Duration = Duration::from_millis(250);
const DETAIL_POLL_INTERVAL: Duration = Duration::from_secs(5);
const DETAIL_BROADCAST_CAPACITY: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DetailKey {
    epoch: u64,
    kind: ResourceKind,
    namespace: String,
    name: String,
}

struct DetailEntry {
    updates: broadcast::Sender<ResourceDetailUpdate>,
    latest: Arc<Mutex<Option<ResourceDetailUpdate>>>,
    _cancel: watch::Sender<bool>,
}

fn detail_registry() -> &'static Mutex<HashMap<DetailKey, Weak<DetailEntry>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<DetailKey, Weak<DetailEntry>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct ResourceDetailSubscription {
    updates: broadcast::Receiver<ResourceDetailUpdate>,
    initial: Option<ResourceDetailUpdate>,
    cancel: watch::Sender<bool>,
    cancel_rx: watch::Receiver<bool>,
    _entry: Arc<DetailEntry>,
}

impl ResourceDetailSubscription {
    pub async fn next(&mut self) -> Option<ResourceDetailUpdate> {
        if let Some(initial) = self.initial.take() {
            return Some(initial);
        }
        loop {
            tokio::select! {
                _ = self.cancel_rx.changed() => return None,
                result = self.updates.recv() => match result {
                    Ok(update) => return Some(update),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        }
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for ResourceDetailSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

pub fn subscribe_detail(
    cx: &mut App,
    kind: ResourceKind,
    namespace: &str,
    name: &str,
) -> Result<ResourceDetailSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let key = DetailKey {
        epoch: state.context_epoch(),
        kind,
        namespace: namespace.to_string(),
        name: name.to_string(),
    };

    let mut registry = detail_registry()
        .lock()
        .map_err(|_| "detail registry is poisoned".to_string())?;
    registry.retain(|_, entry| entry.strong_count() > 0);

    if let Some(entry) = registry.get(&key).and_then(Weak::upgrade) {
        return Ok(subscription_for(entry));
    }

    let (updates, _) = broadcast::channel(DETAIL_BROADCAST_CAPACITY);
    let (cancel, cancel_rx) = watch::channel(false);
    let latest = Arc::new(Mutex::new(None));
    let entry = Arc::new(DetailEntry {
        updates: updates.clone(),
        latest: Arc::clone(&latest),
        _cancel: cancel,
    });
    let receiver = updates.subscribe();
    tokio.spawn(watch_detail(
        client,
        kind,
        namespace.to_string(),
        name.to_string(),
        updates,
        latest,
        cancel_rx,
    ));
    registry.insert(key, Arc::downgrade(&entry));
    drop(registry);

    let (sub_cancel, sub_cancel_rx) = watch::channel(false);
    Ok(ResourceDetailSubscription {
        updates: receiver,
        initial: None,
        cancel: sub_cancel,
        cancel_rx: sub_cancel_rx,
        _entry: entry,
    })
}

fn subscription_for(entry: Arc<DetailEntry>) -> ResourceDetailSubscription {
    let updates = entry.updates.subscribe();
    let initial = entry.latest.lock().ok().and_then(|latest| latest.clone());
    let (cancel, cancel_rx) = watch::channel(false);
    ResourceDetailSubscription {
        updates,
        initial,
        cancel,
        cancel_rx,
        _entry: entry,
    }
}

async fn watch_detail(
    client: kube::Client,
    kind: ResourceKind,
    namespace: String,
    name: String,
    updates: broadcast::Sender<ResourceDetailUpdate>,
    latest: Arc<Mutex<Option<ResourceDetailUpdate>>>,
    mut cancel: watch::Receiver<bool>,
) {
    let mut retry = Retry::new();
    let mut resource_version = String::new();
    let mut need_get = true;

    loop {
        let api = api_for(&client, kind, &namespace);
        if need_get {
            let fetched = tokio::select! {
                _ = cancel.changed() => return,
                result = api.get(&name) => result,
            };
            match fetched {
                Ok(object) => {
                    retry.list_succeeded();
                    if let Some(version) = object.resource_version() {
                        resource_version = version;
                    }
                    if !send_detail(&updates, &latest, kind, &namespace, &name, &object) {
                        return;
                    }
                    need_get = false;
                }
                Err(error) => {
                    if !publish(
                        &updates,
                        &latest,
                        ResourceDetailUpdate {
                            kind,
                            namespace: namespace.clone(),
                            name: name.clone(),
                            result: Err(format!(
                                "Could not load {} details: {error}",
                                kind.label()
                            )),
                        },
                    ) {
                        return;
                    }
                    match retry.failed(&error) {
                        RetryAction::Retry(delay) => {
                            if !sleep_or_cancel(&mut cancel, delay).await {
                                return;
                            }
                            continue;
                        }
                        RetryAction::GiveUp => return,
                    }
                }
            }
        }

        let params = WatchParams::default()
            .timeout(DETAIL_WATCH_TIMEOUT_SECS)
            .fields(&format!("metadata.name={name}"));
        let watched = tokio::select! {
            _ = cancel.changed() => return,
            result = retry::bounded(api.watch(&params, &resource_version)) => result,
        };
        let mut stream = match watched {
            Ok(stream) => {
                retry.watch_succeeded();
                Box::pin(stream)
            }
            Err(failure) => match failure.action(&mut retry) {
                RetryAction::Retry(delay) => {
                    if !sleep_or_cancel(&mut cancel, delay).await {
                        return;
                    }
                    continue;
                }
                RetryAction::GiveUp => {
                    poll_detail(
                        &client,
                        kind,
                        &namespace,
                        &name,
                        &updates,
                        &latest,
                        &mut cancel,
                    )
                    .await;
                    return;
                }
            },
        };

        let mut relist = false;
        loop {
            tokio::select! {
                _ = cancel.changed() => return,
                event = stream.try_next() => match event {
                    Ok(Some(WatchEvent::Added(object) | WatchEvent::Modified(object))) => {
                        retry.watch_succeeded();
                        if let Some(version) = object.resource_version() {
                            resource_version = version;
                        }
                        if !send_detail(&updates, &latest, kind, &namespace, &name, &object) {
                            return;
                        }
                    }
                    Ok(Some(WatchEvent::Deleted(_))) => {
                        publish(
                            &updates,
                            &latest,
                            ResourceDetailUpdate {
                                kind,
                                namespace: namespace.clone(),
                                name: name.clone(),
                                result: Err(format!("{} {} was deleted", kind.label(), name)),
                            },
                        );
                        return;
                    }
                    Ok(Some(WatchEvent::Bookmark(bookmark))) => {
                        retry.watch_succeeded();
                        resource_version = bookmark.metadata.resource_version;
                    }
                    Ok(Some(WatchEvent::Error(status))) => {
                        relist = status.code == 410
                            || status.reason == "Expired"
                            || status.reason == "Gone";
                        match retry.failed_status(status.code) {
                            RetryAction::Retry(delay) => {
                                if !sleep_or_cancel(&mut cancel, delay).await {
                                    return;
                                }
                                break;
                            }
                            RetryAction::GiveUp => {
                                poll_detail(
                                    &client,
                                    kind,
                                    &namespace,
                                    &name,
                                    &updates,
                                    &latest,
                                    &mut cancel,
                                )
                                .await;
                                return;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(error) => match retry.failed(&error) {
                        RetryAction::Retry(delay) => {
                            if !sleep_or_cancel(&mut cancel, delay).await {
                                return;
                            }
                            break;
                        }
                        RetryAction::GiveUp => {
                            poll_detail(
                                &client,
                                kind,
                                &namespace,
                                &name,
                                &updates,
                                &latest,
                                &mut cancel,
                            )
                            .await;
                            return;
                        }
                    },
                }
            }
        }

        if relist {
            need_get = true;
        }
        if !sleep_or_cancel(&mut cancel, DETAIL_RECONNECT_DELAY).await {
            return;
        }
    }
}

/// Watch was denied (or the object is not watchable): keep the detail fresh
/// with a low-frequency GET instead of hammering the watch endpoint.
async fn poll_detail(
    client: &kube::Client,
    kind: ResourceKind,
    namespace: &str,
    name: &str,
    updates: &broadcast::Sender<ResourceDetailUpdate>,
    latest: &Arc<Mutex<Option<ResourceDetailUpdate>>>,
    cancel: &mut watch::Receiver<bool>,
) {
    let mut retry = Retry::new();
    let api = api_for(client, kind, namespace);
    loop {
        let fetched = tokio::select! {
            _ = cancel.changed() => return,
            result = api.get(name) => result,
        };
        match fetched {
            Ok(object) => {
                retry.list_succeeded();
                if !send_detail(updates, latest, kind, namespace, name, &object) {
                    return;
                }
            }
            Err(error) => {
                if !publish(
                    updates,
                    latest,
                    ResourceDetailUpdate {
                        kind,
                        namespace: namespace.to_string(),
                        name: name.to_string(),
                        result: Err(format!("Could not load {} details: {error}", kind.label())),
                    },
                ) {
                    return;
                }
                match retry.failed(&error) {
                    RetryAction::Retry(delay) => {
                        if !sleep_or_cancel(cancel, delay).await {
                            return;
                        }
                        continue;
                    }
                    RetryAction::GiveUp => return,
                }
            }
        }
        if !sleep_or_cancel(cancel, DETAIL_POLL_INTERVAL).await {
            return;
        }
    }
}

fn fmt_detail(
    kind: ResourceKind,
    namespace: &str,
    name: &str,
    object: &DynamicObject,
) -> ResourceDetailUpdate {
    let mut value = serde_json::to_value(object).unwrap_or(Value::Null);
    sanitize_secret(kind, &mut value);
    ResourceDetailUpdate {
        kind,
        namespace: namespace.to_string(),
        name: name.to_string(),
        result: Ok(Box::new(ResourceDetail {
            kind,
            namespace: namespace.to_string(),
            name: name.to_string(),
            raw_json: serde_json::to_string_pretty(&value).unwrap_or_default(),
            value,
        })),
    }
}

fn send_detail(
    updates: &broadcast::Sender<ResourceDetailUpdate>,
    latest: &Arc<Mutex<Option<ResourceDetailUpdate>>>,
    kind: ResourceKind,
    namespace: &str,
    name: &str,
    object: &DynamicObject,
) -> bool {
    publish(updates, latest, fmt_detail(kind, namespace, name, object))
}

fn publish(
    updates: &broadcast::Sender<ResourceDetailUpdate>,
    latest: &Arc<Mutex<Option<ResourceDetailUpdate>>>,
    update: ResourceDetailUpdate,
) -> bool {
    if let Ok(mut latest) = latest.lock() {
        *latest = Some(update.clone());
    }
    updates.send(update).is_ok()
}

async fn sleep_or_cancel(cancel: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    tokio::select! {
        _ = cancel.changed() => false,
        _ = tokio::time::sleep(duration) => true,
    }
}

fn api_for(client: &kube::Client, kind: ResourceKind, namespace: &str) -> Api<DynamicObject> {
    let resource = api_resource(kind);
    if kind.is_namespaced() {
        Api::namespaced_with(client.clone(), namespace, &resource)
    } else {
        Api::all_with(client.clone(), &resource)
    }
}

fn api_resource(kind: ResourceKind) -> ApiResource {
    let (group, version, kind_name, plural) = match kind {
        ResourceKind::Pods => ("", "v1", "Pod", "pods"),
        ResourceKind::Deployments => ("apps", "v1", "Deployment", "deployments"),
        ResourceKind::StatefulSets => ("apps", "v1", "StatefulSet", "statefulsets"),
        ResourceKind::ReplicaSets => ("apps", "v1", "ReplicaSet", "replicasets"),
        ResourceKind::ReplicationControllers => {
            ("", "v1", "ReplicationController", "replicationcontrollers")
        }
        ResourceKind::DaemonSets => ("apps", "v1", "DaemonSet", "daemonsets"),
        ResourceKind::Jobs => ("batch", "v1", "Job", "jobs"),
        ResourceKind::CronJobs => ("batch", "v1", "CronJob", "cronjobs"),
        ResourceKind::HorizontalPodAutoscalers => (
            "autoscaling",
            "v2",
            "HorizontalPodAutoscaler",
            "horizontalpodautoscalers",
        ),
        ResourceKind::PodDisruptionBudgets => (
            "policy",
            "v1",
            "PodDisruptionBudget",
            "poddisruptionbudgets",
        ),
        ResourceKind::Services => ("", "v1", "Service", "services"),
        ResourceKind::Ingresses => ("networking.k8s.io", "v1", "Ingress", "ingresses"),
        ResourceKind::Endpoints => ("", "v1", "Endpoints", "endpoints"),
        ResourceKind::EndpointSlices => {
            ("discovery.k8s.io", "v1", "EndpointSlice", "endpointslices")
        }
        ResourceKind::NetworkPolicies => (
            "networking.k8s.io",
            "v1",
            "NetworkPolicy",
            "networkpolicies",
        ),
        ResourceKind::PersistentVolumes => ("", "v1", "PersistentVolume", "persistentvolumes"),
        ResourceKind::PersistentVolumeClaims => {
            ("", "v1", "PersistentVolumeClaim", "persistentvolumeclaims")
        }
        ResourceKind::StorageClasses => ("storage.k8s.io", "v1", "StorageClass", "storageclasses"),
        ResourceKind::ConfigMaps => ("", "v1", "ConfigMap", "configmaps"),
        ResourceKind::Secrets => ("", "v1", "Secret", "secrets"),
        ResourceKind::ServiceAccounts => ("", "v1", "ServiceAccount", "serviceaccounts"),
        ResourceKind::Namespaces => ("", "v1", "Namespace", "namespaces"),
        ResourceKind::Nodes => ("", "v1", "Node", "nodes"),
        ResourceKind::Leases => ("coordination.k8s.io", "v1", "Lease", "leases"),
        ResourceKind::Events => ("", "v1", "Event", "events"),
        ResourceKind::ClusterRoles => (
            "rbac.authorization.k8s.io",
            "v1",
            "ClusterRole",
            "clusterroles",
        ),
        ResourceKind::Roles => ("rbac.authorization.k8s.io", "v1", "Role", "roles"),
        ResourceKind::RoleBindings => (
            "rbac.authorization.k8s.io",
            "v1",
            "RoleBinding",
            "rolebindings",
        ),
        ResourceKind::Custom(index) => match crate::models::resources::catalog::custom_def(index) {
            Some(def) => {
                return ApiResource::from_gvk_with_plural(
                    &GroupVersionKind::gvk(&def.group, &def.version, &def.kind),
                    &def.plural,
                );
            }
            None => ("", "v1", "Unknown", "unknowns"),
        },
    };
    ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(group, version, kind_name), plural)
}

fn sanitize_secret(kind: ResourceKind, value: &mut Value) {
    if kind != ResourceKind::Secrets {
        return;
    }
    for key in ["data", "stringData"] {
        if let Some(values) = value.get_mut(key).and_then(Value::as_object_mut) {
            for value in values.values_mut() {
                *value = Value::String("••••••".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn masks_secret_values() {
        let mut value = json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "data": { "token": "c2VjcmV0" },
            "stringData": { "password": "plain" },
            "type": "Opaque"
        });
        sanitize_secret(ResourceKind::Secrets, &mut value);
        assert_eq!(value["data"]["token"], "••••••");
        assert_eq!(value["stringData"]["password"], "••••••");
        assert_eq!(value["type"], "Opaque");
    }

    #[test]
    fn maps_ingresses_to_their_real_plural() {
        assert_eq!(api_resource(ResourceKind::Ingresses).plural, "ingresses");
        assert_eq!(
            api_resource(ResourceKind::PersistentVolumeClaims).plural,
            "persistentvolumeclaims"
        );
        assert_eq!(api_resource(ResourceKind::Nodes).group, "");
        assert_eq!(api_resource(ResourceKind::CronJobs).plural, "cronjobs");
        assert_eq!(
            api_resource(ResourceKind::StorageClasses).group,
            "storage.k8s.io"
        );
        assert_eq!(api_resource(ResourceKind::Namespaces).plural, "namespaces");
    }
}
