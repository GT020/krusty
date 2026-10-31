//! Kubernetes list + watch loop for one resource kind / namespace scope.
//!
//! This module runs entirely on the cluster tokio runtime. It owns the
//! [`ResourceStore`] and only publishes immutable snapshots to the UI, throttled
//! to `FLUSH_INTERVAL` or every `FLUSH_AT` events, whichever comes first.

use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures::TryStreamExt as _;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Event, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod,
    ReplicationController, Secret, Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::networking::v1::{Ingress, NetworkPolicy};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::api::rbac::v1::{ClusterRole, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::api::{Api, DynamicObject, ListParams, ResourceExt, WatchEvent, WatchParams};
use kube::core::{ApiResource, GroupVersionKind, NamespaceResourceScope};
use kube::{Client, Resource};
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::models::{
    Item, NamespaceScope, ObjectKey, PollRate, ResourceKind, ResourceSnapshot, ResourceStore,
    SnapshotPhase, WatchMode,
};
use crate::repository::retry::{self, RequestFailure, Retry, RetryAction};
use crate::repository::watch_prefs::{WatchPreference, WatchPrefs};

const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const FLUSH_AT: usize = 256;

/// List chunk size. Big kinds (thousands of pods/events) render their first
/// chunk immediately instead of waiting for one huge response.
const LIST_CHUNK_SIZE: u32 = 500;

/// Kinds whose watch the API server denied (RBAC) for the current context
/// epoch. New subscriptions for these kinds skip the doomed watch attempt and
/// start polling right away, so changing the namespace filter does not reset
/// polling back to a live-watch attempt.
static WATCH_DENIED: OnceLock<Mutex<HashSet<(u64, ResourceKind)>>> = OnceLock::new();

fn denied_registry() -> &'static Mutex<HashSet<(u64, ResourceKind)>> {
    WATCH_DENIED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn watch_denied(epoch: u64, kind: ResourceKind) -> bool {
    denied_registry()
        .lock()
        .is_ok_and(|set| set.contains(&(epoch, kind)))
}

/// Forget a denial: the user explicitly asked for live watch again.
pub fn clear_watch_denied(epoch: u64, kind: ResourceKind) {
    if let Ok(mut set) = denied_registry().lock() {
        set.remove(&(epoch, kind));
    }
}

fn mark_watch_denied(epoch: u64, kind: ResourceKind) {
    if let Ok(mut set) = denied_registry().lock() {
        set.retain(|(stale_epoch, _)| *stale_epoch == epoch);
        set.insert((epoch, kind));
    }
}

/// Spawned once per subscription. Keep it generic over the store; the typed
/// kind dispatch lives in `run`.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    client: Client,
    kind: ResourceKind,
    scope: NamespaceScope,
    epoch: u64,
    tx: watch::Sender<Arc<ResourceSnapshot>>,
    cancel: watch::Receiver<bool>,
    refresh: watch::Receiver<u64>,
    prefs: watch::Receiver<WatchPrefs>,
) {
    let mut task = WatchTask {
        client,
        kind,
        epoch,
        tx,
        cancel,
        refresh,
        prefs,
        store: ResourceStore::new(),
        mode: WatchMode::Watch,
    };
    match kind {
        ResourceKind::Pods => task.run_namespaced::<Pod, _>(scope, pod_item).await,
        ResourceKind::Deployments => {
            task.run_namespaced::<Deployment, _>(scope, deployment_item)
                .await
        }
        ResourceKind::StatefulSets => {
            task.run_namespaced::<StatefulSet, _>(scope, stateful_set_item)
                .await
        }
        ResourceKind::ReplicaSets => {
            task.run_namespaced::<ReplicaSet, _>(scope, replica_set_item)
                .await
        }
        ResourceKind::ReplicationControllers => {
            task.run_namespaced::<ReplicationController, _>(scope, replication_controller_item)
                .await
        }
        ResourceKind::DaemonSets => {
            task.run_namespaced::<DaemonSet, _>(scope, daemon_set_item)
                .await
        }
        ResourceKind::Jobs => task.run_namespaced::<Job, _>(scope, job_item).await,
        ResourceKind::CronJobs => {
            task.run_namespaced::<CronJob, _>(scope, cron_job_item)
                .await
        }
        ResourceKind::HorizontalPodAutoscalers => {
            task.run_namespaced::<HorizontalPodAutoscaler, _>(scope, horizontal_pod_autoscaler_item)
                .await
        }
        ResourceKind::PodDisruptionBudgets => {
            task.run_namespaced::<PodDisruptionBudget, _>(scope, pod_disruption_budget_item)
                .await
        }
        ResourceKind::Services => task.run_namespaced::<Service, _>(scope, service_item).await,
        ResourceKind::Ingresses => task.run_namespaced::<Ingress, _>(scope, ingress_item).await,
        ResourceKind::Endpoints => {
            task.run_namespaced::<Endpoints, _>(scope, endpoints_item)
                .await
        }
        ResourceKind::EndpointSlices => {
            task.run_namespaced::<EndpointSlice, _>(scope, endpoint_slice_item)
                .await
        }
        ResourceKind::NetworkPolicies => {
            task.run_namespaced::<NetworkPolicy, _>(scope, network_policy_item)
                .await
        }
        ResourceKind::PersistentVolumeClaims => {
            task.run_namespaced::<PersistentVolumeClaim, _>(scope, pvc_item)
                .await
        }
        ResourceKind::StorageClasses => {
            task.run_cluster::<StorageClass, _>(storage_class_item)
                .await
        }
        ResourceKind::ConfigMaps => {
            task.run_namespaced::<ConfigMap, _>(scope, config_map_item)
                .await
        }
        ResourceKind::Secrets => task.run_namespaced::<Secret, _>(scope, secret_item).await,
        ResourceKind::ServiceAccounts => {
            task.run_namespaced::<ServiceAccount, _>(scope, service_account_item)
                .await
        }
        ResourceKind::PersistentVolumes => task.run_cluster::<PersistentVolume, _>(pv_item).await,
        ResourceKind::Namespaces => task.run_cluster::<Namespace, _>(namespace_item).await,
        ResourceKind::Nodes => task.run_cluster::<Node, _>(node_item).await,
        ResourceKind::Leases => task.run_namespaced::<Lease, _>(scope, lease_item).await,
        ResourceKind::Events => task.run_namespaced::<Event, _>(scope, event_item).await,
        ResourceKind::ClusterRoles => task.run_cluster::<ClusterRole, _>(cluster_role_item).await,
        ResourceKind::Roles => task.run_namespaced::<Role, _>(scope, role_item).await,
        ResourceKind::RoleBindings => {
            task.run_namespaced::<RoleBinding, _>(scope, role_binding_item)
                .await
        }
        ResourceKind::Custom(index) => {
            if let Some(def) = crate::models::resources::catalog::custom_def(index) {
                let resource = ApiResource::from_gvk_with_plural(
                    &GroupVersionKind::gvk(&def.group, &def.version, &def.kind),
                    &def.plural,
                );
                task.run_dynamic(resource, scope).await;
            }
        }
    }
}

struct WatchTask {
    client: Client,
    kind: ResourceKind,
    epoch: u64,
    tx: watch::Sender<Arc<ResourceSnapshot>>,
    cancel: watch::Receiver<bool>,
    refresh: watch::Receiver<u64>,
    prefs: watch::Receiver<WatchPrefs>,
    store: ResourceStore,
    mode: WatchMode,
}

/// Why the poll loop exited.
enum PollOutcome {
    /// Subscription cancelled or listing denied: stop the task.
    Stop,
    /// The user switched this kind back to watch mode.
    ResumeWatch,
}

/// Outcome of a chunked list.
enum ListOutcome {
    /// Full list published; watch from this resource version.
    Listed(String),
    /// Subscription cancelled mid-list.
    Cancelled,
    /// A chunk request failed or timed out; the caller decides whether to retry.
    Failed(RequestFailure),
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

    async fn run_dynamic(&mut self, resource: ApiResource, scope: NamespaceScope) {
        let api = match &scope {
            NamespaceScope::All => Api::<DynamicObject>::all_with(self.client.clone(), &resource),
            NamespaceScope::One(namespace) => {
                Api::<DynamicObject>::namespaced_with(self.client.clone(), namespace, &resource)
            }
        };
        self.run_stream(api, custom_item).await;
    }

    async fn run_stream<K, F>(&mut self, api: Api<K>, map: F)
    where
        K: Resource + Clone + Debug + DeserializeOwned + Send + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        let watch_params = WatchParams::default().timeout(280);
        let mut resource_version = String::new();
        let mut need_list = true;
        let mut retry = Retry::new();
        let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;

        loop {
            if self.cancel.has_changed().is_err() {
                return;
            }
            // The user prefers polling for this kind: skip the watch entirely.
            if self.prefs.borrow().mode == WatchPreference::Poll {
                match self.poll_loop(&api, map).await {
                    PollOutcome::Stop => return,
                    PollOutcome::ResumeWatch => {
                        need_list = true;
                        retry = Retry::new();
                        continue;
                    }
                }
            }
            if need_list {
                match self.list_progressively(&api, map).await {
                    ListOutcome::Listed(version) => {
                        retry.list_succeeded();
                        resource_version = version;
                        need_list = false;
                    }
                    ListOutcome::Cancelled => return,
                    ListOutcome::Failed(failure) => match failure.action(&mut retry) {
                        RetryAction::Retry(delay) => {
                            tracing::warn!(%failure, ?api, delay_ms = delay.as_millis() as u64, "repository: list failed, retrying");
                            self.publish(SnapshotPhase::Error(format!("list failed: {failure}")));
                            if !self.sleep(delay).await {
                                return;
                            }
                            continue;
                        }
                        RetryAction::GiveUp => {
                            tracing::error!(%failure, ?api, "repository: list rejected, stopping watch");
                            self.publish(SnapshotPhase::Error(format!("list rejected: {failure}")));
                            return;
                        }
                    },
                }
            }

            let watched = tokio::select! {
                _ = self.cancel.changed() => return,
                result = retry::bounded(api.watch(&watch_params, &resource_version)) => result,
            };
            let mut stream = match watched {
                Ok(stream) => {
                    clear_watch_denied(self.epoch, self.kind);
                    Box::pin(stream)
                }
                Err(failure) => match failure.action(&mut retry) {
                    RetryAction::Retry(delay) => {
                        tracing::warn!(%failure, delay_ms = delay.as_millis() as u64, "repository: watch setup failed, retrying");
                        if !self.sleep(delay).await {
                            return;
                        }
                        need_list = true;
                        continue;
                    }
                    RetryAction::GiveUp => {
                        tracing::warn!(%failure, ?api, "repository: watch denied, polling instead");
                        mark_watch_denied(self.epoch, self.kind);
                        match self.poll_loop(&api, map).await {
                            PollOutcome::Stop => return,
                            PollOutcome::ResumeWatch => {
                                need_list = true;
                                retry = Retry::new();
                                continue;
                            }
                        }
                    }
                },
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
                    _ = self.prefs.changed() => {
                        let prefs = *self.prefs.borrow_and_update();
                        if prefs.mode == WatchPreference::Poll {
                            tracing::info!(?api, "repository: watch disabled by preference, polling instead");
                            drop(stream);
                            match self.poll_loop(&api, map).await {
                                PollOutcome::Stop => return,
                                PollOutcome::ResumeWatch => {
                                    relist = true;
                                    retry = Retry::new();
                                    break;
                                }
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        if self.store.pending() > 0 {
                            self.publish(SnapshotPhase::Ready);
                        }
                    }
                    event = stream.try_next() => {
                        match event {
                            Ok(Some(WatchEvent::Added(object) | WatchEvent::Modified(object))) => {
                                retry.watch_succeeded();
                                let item = map(&object);
                                if !item.resource_version.is_empty() {
                                    resource_version.clone_from(&item.resource_version);
                                }
                                self.store.upsert(item);
                            }
                            Ok(Some(WatchEvent::Deleted(object))) => {
                                retry.watch_succeeded();
                                if let Some(version) = object.resource_version() {
                                    resource_version = version;
                                }
                                self.store.remove(&ObjectKey::new(
                                    object.namespace().unwrap_or_default(),
                                    object.name_any(),
                                ));
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
                                        tracing::warn!(
                                            code = status.code,
                                            message = %status.message,
                                            delay_ms = delay.as_millis() as u64,
                                            "repository: watch error event, retrying"
                                        );
                                        if !self.sleep(delay).await {
                                            return;
                                        }
                                        break;
                                    }
                                    RetryAction::GiveUp => {
                                        tracing::warn!(
                                            code = status.code,
                                            message = %status.message,
                                            "repository: watch denied, polling instead"
                                        );
                                        mark_watch_denied(self.epoch, self.kind);
                                        drop(stream);
                                        match self.poll_loop(&api, map).await {
                                            PollOutcome::Stop => return,
                                            PollOutcome::ResumeWatch => {
                                                relist = true;
                                                retry = Retry::new();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::debug!("repository: watch stream ended, resuming");
                                break;
                            }
                            Err(error) => match retry.failed(&error) {
                                RetryAction::Retry(delay) => {
                                    tracing::warn!(%error, delay_ms = delay.as_millis() as u64, "repository: watch stream failed, retrying");
                                    if !self.sleep(delay).await {
                                        return;
                                    }
                                    break;
                                }
                                RetryAction::GiveUp => {
                                    tracing::warn!(%error, ?api, "repository: watch denied, polling instead");
                                    mark_watch_denied(self.epoch, self.kind);
                                    drop(stream);
                                    match self.poll_loop(&api, map).await {
                                        PollOutcome::Stop => return,
                                        PollOutcome::ResumeWatch => {
                                            relist = true;
                                            retry = Retry::new();
                                            break;
                                        }
                                    }
                                }
                            },
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
        let snapshot = Arc::new(self.store.snapshot(phase).with_mode(self.mode));
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

    /// List every object in chunks, publishing each chunk so large kinds
    /// render progressively instead of waiting for one huge response.
    /// Returns the resource version to watch from.
    async fn list_progressively<K, F>(&mut self, api: &Api<K>, map: F) -> ListOutcome
    where
        K: Resource + Clone + Debug + DeserializeOwned + Send + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        let mut params = ListParams::default().limit(LIST_CHUNK_SIZE);
        let mut resource_version = String::new();
        let mut first_chunk = true;
        loop {
            let listed = tokio::select! {
                _ = self.cancel.changed() => return ListOutcome::Cancelled,
                result = retry::bounded(api.list(&params)) => result,
            };
            let chunk = match listed {
                Ok(chunk) => chunk,
                Err(failure) => return ListOutcome::Failed(failure),
            };
            // Every chunk carries the version of the same list snapshot.
            if let Some(version) = chunk.metadata.resource_version {
                resource_version = version;
            }
            if first_chunk {
                self.store.reset(chunk.items.iter().map(map));
                first_chunk = false;
            } else {
                for object in &chunk.items {
                    self.store.upsert(map(object));
                }
            }
            self.publish(SnapshotPhase::Ready);
            match chunk.metadata.continue_ {
                Some(token) if !token.is_empty() => {
                    params = params.continue_token(&token);
                }
                _ => break,
            }
        }
        ListOutcome::Listed(resource_version)
    }

    /// Poll instead of watching — because watch was denied (RBAC) or the user
    /// prefers polling for this kind. Runs until the subscription is
    /// cancelled, listing is denied, or the user switches back to watch mode.
    /// `PollRate::Off` disables the timer; manual refresh still re-lists.
    async fn poll_loop<K, F>(&mut self, api: &Api<K>, map: F) -> PollOutcome
    where
        K: Resource + Clone + Debug + DeserializeOwned + Send + 'static,
        F: Fn(&K) -> Item + Copy,
    {
        self.mode = WatchMode::Poll(self.prefs.borrow().rate);
        // Reflect the mode switch in the UI immediately, not after the first
        // poll roundtrip.
        if self.store.len() > 0 {
            self.publish(SnapshotPhase::Ready);
        }
        let mut retry = Retry::new();

        loop {
            if self.cancel.has_changed().is_err() {
                return PollOutcome::Stop;
            }

            match self.list_progressively(api, map).await {
                ListOutcome::Listed(_) => {
                    retry.list_succeeded();
                }
                ListOutcome::Cancelled => return PollOutcome::Stop,
                ListOutcome::Failed(failure) => match failure.action(&mut retry) {
                    RetryAction::Retry(delay) => {
                        tracing::warn!(%failure, ?api, delay_ms = delay.as_millis() as u64, "repository: poll list failed, retrying");
                        self.publish(SnapshotPhase::Error(format!("list failed: {failure}")));
                        if !self.sleep(delay).await {
                            return PollOutcome::Stop;
                        }
                        continue;
                    }
                    RetryAction::GiveUp => {
                        tracing::error!(%failure, ?api, "repository: poll list rejected, stopping watch");
                        self.publish(SnapshotPhase::Error(format!("list rejected: {failure}")));
                        return PollOutcome::Stop;
                    }
                },
            }

            // Wait for the next tick. Refresh or a preference change re-list
            // immediately; `Off` waits without a timer.
            let rate = self.prefs.borrow().rate;
            tokio::select! {
                _ = self.cancel.changed() => return PollOutcome::Stop,
                _ = self.refresh.changed() => {
                    self.refresh.borrow_and_update();
                }
                _ = self.prefs.changed() => {
                    let prefs = *self.prefs.borrow_and_update();
                    self.mode = WatchMode::Poll(prefs.rate);
                    if prefs.mode == WatchPreference::Watch {
                        self.mode = WatchMode::Watch;
                        self.publish(SnapshotPhase::Ready);
                        return PollOutcome::ResumeWatch;
                    }
                    // Show the new rate/off state instantly; the re-list
                    // happens on the next loop turn.
                    self.publish(SnapshotPhase::Ready);
                }
                _ = async {
                        match rate {
                            PollRate::Every(secs) => {
                                tokio::time::sleep(Duration::from_secs(secs)).await
                            }
                            PollRate::Off => std::future::pending().await,
                        }
                    } => {}
            }
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

fn stateful_set_item(stateful_set: &StatefulSet) -> Item {
    let mut item = base_item(stateful_set);
    let desired = stateful_set
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = stateful_set
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
    item.version = version_label(stateful_set.metadata.labels.as_ref());
    item
}

fn replica_set_item(replica_set: &ReplicaSet) -> Item {
    let mut item = base_item(replica_set);
    let desired = replica_set
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = replica_set
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
    item.version = version_label(replica_set.metadata.labels.as_ref());
    item
}

fn replication_controller_item(controller: &ReplicationController) -> Item {
    let mut item = base_item(controller);
    let desired = controller
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = controller
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
    item.version = version_label(controller.metadata.labels.as_ref());
    item
}

fn horizontal_pod_autoscaler_item(autoscaler: &HorizontalPodAutoscaler) -> Item {
    let mut item = base_item(autoscaler);
    let spec = autoscaler.spec.as_ref();
    let max = spec.map(|spec| spec.max_replicas).unwrap_or(0);
    let min = spec.and_then(|spec| spec.min_replicas).unwrap_or(0);
    let current = autoscaler
        .status
        .as_ref()
        .and_then(|status| status.current_replicas)
        .unwrap_or(0);
    item.ready = format!("{current}/{min}");
    item.capacity = max.to_string();
    item.status = if current >= min { "active" } else { "idle" }.to_string();
    item.version = spec
        .map(|spec| {
            format!(
                "{}/{}",
                spec.scale_target_ref.kind, spec.scale_target_ref.name
            )
        })
        .unwrap_or_default();
    item
}

fn pod_disruption_budget_item(budget: &PodDisruptionBudget) -> Item {
    let mut item = base_item(budget);
    let status = budget.status.as_ref();
    let current = status.map(|status| status.current_healthy).unwrap_or(0);
    let desired = status.map(|status| status.desired_healthy).unwrap_or(0);
    let allowed = status.map(|status| status.disruptions_allowed).unwrap_or(0);
    item.ready = format!("{current}/{desired}");
    item.capacity = allowed.to_string();
    item.status = if allowed > 0 { "ready" } else { "blocked" }.to_string();
    item
}

fn lease_item(lease: &Lease) -> Item {
    let mut item = base_item(lease);
    let spec = lease.spec.as_ref();
    item.version = spec
        .and_then(|spec| spec.holder_identity.clone())
        .unwrap_or_default();
    item.capacity = spec
        .and_then(|spec| spec.lease_duration_seconds)
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_default();
    item.status = "active".to_string();
    item
}

fn endpoints_item(endpoints: &Endpoints) -> Item {
    let mut item = base_item(endpoints);
    let addresses = endpoints
        .subsets
        .as_ref()
        .map(|subsets| {
            subsets
                .iter()
                .map(|subset| {
                    subset.addresses.as_ref().map(Vec::len).unwrap_or(0)
                        + subset
                            .not_ready_addresses
                            .as_ref()
                            .map(Vec::len)
                            .unwrap_or(0)
                })
                .sum::<usize>()
        })
        .unwrap_or(0);
    item.capacity = addresses.to_string();
    item.status = "active".to_string();
    item
}

fn endpoint_slice_item(slice: &EndpointSlice) -> Item {
    let mut item = base_item(slice);
    item.capacity = slice.endpoints.len().to_string();
    item.version = slice.address_type.clone();
    item.status = "active".to_string();
    item
}

fn network_policy_item(policy: &NetworkPolicy) -> Item {
    let mut item = base_item(policy);
    item.capacity = policy
        .spec
        .as_ref()
        .and_then(|spec| spec.policy_types.as_ref())
        .map(|types| types.join(", "))
        .unwrap_or_default();
    item.status = "active".to_string();
    item
}

fn event_item(event: &Event) -> Item {
    let mut item = base_item(event);
    item.status = event
        .type_
        .clone()
        .unwrap_or_else(|| "normal".to_string())
        .to_lowercase();
    item.version = event.reason.clone().unwrap_or_default();
    item.capacity = event.count.unwrap_or(1).to_string();
    item
}

fn cluster_role_item(cluster_role: &ClusterRole) -> Item {
    let mut item = base_item(cluster_role);
    item.capacity = cluster_role
        .rules
        .as_ref()
        .map(|rules| rules.len())
        .unwrap_or(0)
        .to_string();
    item.status = "active".to_string();
    item
}

fn role_item(role: &Role) -> Item {
    let mut item = base_item(role);
    item.capacity = role
        .rules
        .as_ref()
        .map(|rules| rules.len())
        .unwrap_or(0)
        .to_string();
    item.status = "active".to_string();
    item
}

fn role_binding_item(binding: &RoleBinding) -> Item {
    let mut item = base_item(binding);
    item.capacity = binding
        .subjects
        .as_ref()
        .map(|subjects| subjects.len())
        .unwrap_or(0)
        .to_string();
    item.version = format!("{}/{}", binding.role_ref.kind, binding.role_ref.name);
    item.status = "active".to_string();
    item
}

fn custom_item(object: &DynamicObject) -> Item {
    let mut item = base_item(object);
    let Some(status) = object.data.get("status") else {
        return item;
    };
    if let Some(phase) = status.get("phase").and_then(serde_json::Value::as_str) {
        item.status = phase.to_lowercase();
    } else if let Some(conditions) = status
        .get("conditions")
        .and_then(serde_json::Value::as_array)
    {
        let ready = conditions.iter().find(|condition| {
            condition.get("type").and_then(serde_json::Value::as_str) == Some("Ready")
        });
        if let Some(ready) = ready {
            item.status =
                if ready.get("status").and_then(serde_json::Value::as_str) == Some("True") {
                    "ready"
                } else {
                    "notready"
                }
                .to_string();
        }
    }
    item
}

fn cron_job_item(cron_job: &CronJob) -> Item {
    let mut item = base_item(cron_job);
    let active = cron_job
        .status
        .as_ref()
        .and_then(|status| status.active.as_ref())
        .map(|active| active.len())
        .unwrap_or(0);
    let suspended = cron_job
        .spec
        .as_ref()
        .and_then(|spec| spec.suspend)
        .unwrap_or(false);
    item.capacity = active.to_string();
    item.status = if suspended { "suspended" } else { "active" }.to_string();
    item.version = cron_job
        .spec
        .as_ref()
        .map(|spec| spec.schedule.clone())
        .unwrap_or_default();
    item
}

fn service_account_item(service_account: &ServiceAccount) -> Item {
    let mut item = base_item(service_account);
    let secrets = service_account.secrets.as_ref().map(Vec::len).unwrap_or(0);
    item.status = "account".to_string();
    item.capacity = secrets.to_string();
    item.version = service_account
        .secrets
        .as_ref()
        .and_then(|secrets| secrets.first())
        .and_then(|secret| secret.name.clone())
        .unwrap_or_default();
    item
}

fn storage_class_item(storage_class: &StorageClass) -> Item {
    let mut item = base_item(storage_class);
    item.version = storage_class.provisioner.clone();
    item.capacity = storage_class.reclaim_policy.clone().unwrap_or_default();
    item.ready = storage_class
        .volume_binding_mode
        .clone()
        .unwrap_or_default();
    item
}

fn namespace_item(namespace: &Namespace) -> Item {
    let mut item = base_item(namespace);
    item.status = namespace
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_denied_registry_roundtrip() {
        // Unique epoch so the process-global registry is not disturbed.
        let epoch = u64::MAX;
        assert!(!watch_denied(epoch, ResourceKind::Pods));
        mark_watch_denied(epoch, ResourceKind::Pods);
        assert!(watch_denied(epoch, ResourceKind::Pods));
        assert!(!watch_denied(epoch, ResourceKind::Nodes));
        clear_watch_denied(epoch, ResourceKind::Pods);
        assert!(!watch_denied(epoch, ResourceKind::Pods));
    }
}
