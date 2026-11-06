
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::sync::Arc;
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
use kube::api::{Api, DynamicObject, ListParams, WatchEvent, WatchParams};
use kube::core::{ApiResource, GroupVersionKind, NamespaceResourceScope};
use kube::{Client, Resource, ResourceExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, watch};

use crate::models::{Item, NamespaceScope, ObjectKey, ResourceKind, SnapshotPhase, WatchMode};
use crate::repository::Subscription;
use crate::repository::store::mapping::{
    cluster_role_item, config_map_item, cron_job_item, custom_item, daemon_set_item,
    deployment_item, endpoint_slice_item, endpoints_item, event_item,
    horizontal_pod_autoscaler_item, ingress_item, job_item, lease_item, namespace_item,
    network_policy_item, node_item, pod_disruption_budget_item, pod_item, pv_item, pvc_item,
    replica_set_item, replication_controller_item, role_binding_item, role_item, secret_item,
    service_account_item, service_item, stateful_set_item, storage_class_item,
};
use crate::repository::store::{
    ClusterCapabilities, KubeObjectRecord, StoreData, StoreSnapshot, StoreView, StoreWatchPlan,
    watch_plan,
};

const EVENT_FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const WATCH_TIMEOUT_SECS: u32 = 280;
const RECONNECT_DELAY: Duration = Duration::from_millis(500);
const WATCH_DENIED_RETRY: Duration = Duration::from_secs(5);
const STREAM_INITIAL_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
const STREAM_INITIAL_TOTAL_TIMEOUT: Duration = Duration::from_secs(60);
const STORE_ITEM_CAP: usize = 50_000;

type ApiFactory<K> = Arc<dyn Fn(&Client, &NamespaceScope) -> Api<K> + Send + Sync>;

/// Handle used by the repository registry and by view subscriptions.
pub(crate) struct KubeObjectStoreHandle {
    commands: mpsc::UnboundedSender<Command>,
    snapshots: watch::Sender<Arc<StoreSnapshot>>,
}

impl KubeObjectStoreHandle {
    pub(crate) fn object(&self, key: &ObjectKey) -> Option<Arc<KubeObjectRecord>> {
        self.snapshots.borrow().object(key).cloned()
    }

    pub(crate) fn snapshot_receiver(&self) -> watch::Receiver<Arc<StoreSnapshot>> {
        self.snapshots.subscribe()
    }

    pub(crate) fn subscribe(
        &self,
        scope: NamespaceScope,
        tokio: &tokio::runtime::Handle,
    ) -> Subscription {
        let _ = self.commands.send(Command::Subscribe(scope.clone()));

        let mut view = StoreView::new(scope.clone());
        let current = self.snapshots.borrow().clone();
        let initial = match view.apply(&current) {
            Some(snapshot) => snapshot,
            None => view.snapshot(),
        };
        let (view_tx, view_rx) = watch::channel(initial);
        let mut store_rx = self.snapshots.subscribe();
        let (cancel, mut cancel_rx) = watch::channel(false);

        let lease = Arc::new(StoreLease {
            cancel,
            commands: self.commands.clone(),
            scope,
        });
        let (paused, paused_rx) = watch::channel(false);
        let refresh_commands = self.commands.clone();
        let refresh = Arc::new(move || {
            let _ = refresh_commands.send(Command::Refresh);
        });

        tokio.spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => break,
                    result = store_rx.changed() => {
                        if result.is_err() {
                            break;
                        }
                        let snapshot = store_rx.borrow_and_update().clone();
                        if let Some(next) = view.apply(&snapshot) {
                            view_tx.send_replace(next);
                        }
                    }
                }
            }
        });

        Subscription::new(view_rx, paused, paused_rx, refresh, lease)
    }
}

struct StoreLease {
    cancel: watch::Sender<bool>,
    commands: mpsc::UnboundedSender<Command>,
    scope: NamespaceScope,
}

impl Drop for StoreLease {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
        let _ = self.commands.send(Command::Unsubscribe(self.scope.clone()));
    }
}

enum Command {
    Subscribe(NamespaceScope),
    Unsubscribe(NamespaceScope),
    Refresh,
}

enum InitialStream {
    Listed { resource_version: String },
    Unsupported,
    Cancelled,
}

enum StoreEvent {
    Replace(NamespaceScope, Vec<KubeObjectRecord>),
    Upsert(KubeObjectRecord),
    Remove(ObjectKey),
    Error(NamespaceScope, String),
    WatchError(NamespaceScope, String),
}

struct StoreTask<K, F>
where
    K: Resource + Clone + Debug + DeserializeOwned + Serialize + Send + Sync + 'static,
    F: Fn(&K) -> Item + Copy + Send + Sync + 'static,
{
    client: Client,
    tokio: tokio::runtime::Handle,
    epoch: u64,
    kind: ResourceKind,
    global_watch: bool,
    data: StoreData,
    scopes: HashMap<NamespaceScope, usize>,
    plan: Option<StoreWatchPlan>,
    watches: Vec<watch::Sender<bool>>,
    events: mpsc::UnboundedSender<StoreEvent>,
    event_rx: mpsc::UnboundedReceiver<StoreEvent>,
    command_rx: mpsc::UnboundedReceiver<Command>,
    snapshots: watch::Sender<Arc<StoreSnapshot>>,
    phase: SnapshotPhase,
    map: F,
    make_api: ApiFactory<K>,
}

impl<K, F> StoreTask<K, F>
where
    K: Resource
        + ResourceExt
        + Clone
        + Debug
        + DeserializeOwned
        + Serialize
        + Send
        + Sync
        + 'static
        + crate::repository::store::StoredObject,
    F: Fn(&K) -> Item + Copy + Send + Sync + 'static,
{
    async fn run(mut self) {
        let mut ticker = tokio::time::interval(EVENT_FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;

        loop {
            tokio::select! {
                command = self.command_rx.recv() => {
                    let Some(command) = command else { break };
                    self.handle_command(command);
                }
                event = self.event_rx.recv() => {
                    let Some(event) = event else { break };
                    self.handle_event(event);
                }
                _ = ticker.tick() => {
                    self.flush(false);
                }
            }
        }

        self.stop_watches();
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::Subscribe(scope) => {
                *self.scopes.entry(scope).or_default() += 1;
                self.reconcile_watches();
            }
            Command::Unsubscribe(scope) => {
                let remove = if let Some(count) = self.scopes.get_mut(&scope) {
                    if *count > 1 {
                        *count -= 1;
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                if remove {
                    self.scopes.remove(&scope);
                }
                self.reconcile_watches();
            }
            Command::Refresh => {
                let plan = self.plan.clone().unwrap_or_else(|| self.compute_plan());
                self.restart_watches(plan);
            }
        }
    }

    fn handle_event(&mut self, event: StoreEvent) {
        match event {
            StoreEvent::Replace(scope, records) => {
                let changed = match &scope {
                    NamespaceScope::All => self.data.replace_all(records),
                    NamespaceScope::One(namespace) => {
                        self.data.replace_namespace(namespace, records)
                    }
                };
                let trimmed = self.data.trim_oldest(STORE_ITEM_CAP);
                tracing::trace!(
                    kind = ?self.kind,
                    ?scope,
                    changed,
                    trimmed,
                    "store: list replaced"
                );
                self.phase = SnapshotPhase::Ready;
                self.flush(true);
            }
            StoreEvent::Upsert(record) => {
                self.data.upsert(record);
                self.phase = SnapshotPhase::Ready;
                self.flush(false);
            }
            StoreEvent::Remove(key) => {
                self.data.remove(&key);
                self.phase = SnapshotPhase::Ready;
                self.flush(false);
            }
            StoreEvent::Error(scope, message) => {
                tracing::warn!(kind = ?self.kind, ?scope, %message, "store: list error");
                self.phase = SnapshotPhase::Error(message);
                self.flush(true);
            }
            StoreEvent::WatchError(scope, message) => {
                tracing::warn!(kind = ?self.kind, ?scope, %message, "store: watch error");
                if self.data.is_empty() {
                    self.phase = SnapshotPhase::Error(message);
                    self.flush(true);
                }
                // With data from the initial list, watch failures are
                // transient: keep serving the cached rows and retry.
            }
        }
    }

    fn reconcile_watches(&mut self) {
        let plan = self.compute_plan();

        if self.scopes.is_empty() {
            self.plan = Some(plan);
            self.stop_watches();
            return;
        }

        if self.plan.as_ref() != Some(&plan) {
            self.plan = Some(plan.clone());
            self.restart_watches(plan);
        } else if self.watches.is_empty() {
            self.restart_watches(plan);
        }
    }

    fn compute_plan(&self) -> StoreWatchPlan {
        let global = if !self.kind.is_namespaced() {
            true
        } else {
            self.global_watch
        };
        watch_plan(
            self.scopes.keys().cloned(),
            &ClusterCapabilities {
                global_watch: global,
                accessible_namespaces: Vec::new(),
            },
        )
    }

    fn restart_watches(&mut self, plan: StoreWatchPlan) {
        self.stop_watches();

        match plan {
            StoreWatchPlan::All => {
                self.spawn_watch(NamespaceScope::All);
            }
            StoreWatchPlan::PerNamespace(namespaces) => {
                for namespace in namespaces {
                    self.spawn_watch(NamespaceScope::One(namespace));
                }
            }
            StoreWatchPlan::UnavailableAll => {
                self.phase = SnapshotPhase::Error(format!(
                    "Cluster-wide {} watch is not permitted; select a namespace.",
                    self.kind.label()
                ));
                self.flush(true);
            }
        }
    }

    fn spawn_watch(&mut self, scope: NamespaceScope) {
        let (cancel, cancel_rx) = watch::channel(false);
        let events = self.events.clone();
        let client = self.client.clone();
        let map = self.map;
        let make_api = self.make_api.clone();
        self.tokio
            .spawn(run_watch(client, scope, events, cancel_rx, map, make_api));
        self.watches.push(cancel);
    }

    fn stop_watches(&mut self) {
        for cancel in self.watches.drain(..) {
            let _ = cancel.send(true);
        }
    }

    fn flush(&mut self, force: bool) {
        if !force && self.data.pending() == 0 {
            return;
        }

        let snapshot = Arc::new(self.data.snapshot(self.phase.clone(), WatchMode::Watch));
        tracing::trace!(
            epoch = self.epoch,
            kind = ?self.kind,
            revision = snapshot.revision(),
            objects = snapshot.objects().len(),
            "store: flush"
        );
        self.snapshots.send_replace(snapshot);
    }
}

fn bookmark_ends_initial_events(annotations: &BTreeMap<String, String>) -> bool {
    annotations
        .get("k8s.io/initial-events-end")
        .is_some_and(|value| value == "true")
}

async fn stream_initial<K, F>(
    api: &Api<K>,
    scope: &NamespaceScope,
    events: &mpsc::UnboundedSender<StoreEvent>,
    cancel: &mut watch::Receiver<bool>,
    map: F,
) -> InitialStream
where
    K: Resource
        + ResourceExt
        + Clone
        + Debug
        + DeserializeOwned
        + Serialize
        + Send
        + Sync
        + 'static
        + crate::repository::store::StoredObject,
    F: Fn(&K) -> Item + Copy + Send + Sync + 'static,
{
    let params = WatchParams::streaming_lists().timeout(WATCH_TIMEOUT_SECS);
    let watched = tokio::select! {
        _ = cancel.changed() => return InitialStream::Cancelled,
        result = api.watch(&params, "") => result,
    };
    let mut stream = match watched {
        Ok(stream) => Box::pin(stream),
        Err(_) => return InitialStream::Unsupported,
    };

    let mut records: BTreeMap<ObjectKey, KubeObjectRecord> = BTreeMap::new();
    let mut resource_version = String::new();
    let total_deadline = tokio::time::Instant::now() + STREAM_INITIAL_TOTAL_TIMEOUT;
    let mut idle_deadline = tokio::time::Instant::now() + STREAM_INITIAL_IDLE_TIMEOUT;

    loop {
        let deadline = total_deadline.min(idle_deadline);
        tokio::select! {
            _ = cancel.changed() => return InitialStream::Cancelled,
            _ = tokio::time::sleep_until(deadline) => {
                return InitialStream::Unsupported;
            }
            event = stream.try_next() => match event {
                Ok(Some(WatchEvent::Added(object) | WatchEvent::Modified(object))) => {
                    idle_deadline =
                        tokio::time::Instant::now() + STREAM_INITIAL_IDLE_TIMEOUT;
                    if let Some(version) = object.resource_version() {
                        resource_version = version;
                    }
                    let record = KubeObjectRecord::new(object.clone(), map(&object));
                    records.insert(record.key.clone(), record);
                }
                Ok(Some(WatchEvent::Deleted(object))) => {
                    idle_deadline =
                        tokio::time::Instant::now() + STREAM_INITIAL_IDLE_TIMEOUT;
                    if let Some(version) = object.resource_version() {
                        resource_version = version;
                    }
                    records.remove(&ObjectKey::new(
                        object.namespace().unwrap_or_default(),
                        object.name_any(),
                    ));
                }
                Ok(Some(WatchEvent::Bookmark(bookmark))) => {
                    if !bookmark.metadata.resource_version.is_empty() {
                        resource_version = bookmark.metadata.resource_version.clone();
                    }
                    if !bookmark_ends_initial_events(&bookmark.metadata.annotations) {
                        return InitialStream::Unsupported;
                    }
                    if events
                        .send(StoreEvent::Replace(
                            scope.clone(),
                            records.into_values().collect(),
                        ))
                        .is_err()
                    {
                        return InitialStream::Cancelled;
                    }
                    return InitialStream::Listed { resource_version };
                }
                Ok(Some(WatchEvent::Error(_))) => return InitialStream::Unsupported,
                Ok(None) | Err(_) => return InitialStream::Unsupported,
            }
        }
    }
}

async fn run_watch<K, F>(
    client: Client,
    scope: NamespaceScope,
    events: mpsc::UnboundedSender<StoreEvent>,
    mut cancel: watch::Receiver<bool>,
    map: F,
    make_api: ApiFactory<K>,
) where
    K: Resource
        + ResourceExt
        + Clone
        + Debug
        + DeserializeOwned
        + Serialize
        + Send
        + Sync
        + 'static
        + crate::repository::store::StoredObject,
    F: Fn(&K) -> Item + Copy + Send + Sync + 'static,
{
    let api = make_api(&client, &scope);
    let mut resource_version = String::new();
    let mut need_list = true;
    let mut stream_initial_disabled = false;

    loop {
        if need_list {
            let mut streamed = false;
            if !stream_initial_disabled {
                match stream_initial(&api, &scope, &events, &mut cancel, map).await {
                    InitialStream::Listed {
                        resource_version: version,
                    } => {
                        tracing::debug!(?scope, "store: initial state came from a streaming list");
                        resource_version = version;
                        need_list = false;
                        streamed = true;
                    }
                    InitialStream::Unsupported => {
                        tracing::debug!(
                            ?scope,
                            "store: streaming list unsupported, falling back to list"
                        );
                        stream_initial_disabled = true;
                    }
                    InitialStream::Cancelled => return,
                }
            }

            if !streamed {
                let params = ListParams::default();
                let listed = tokio::select! {
                    _ = cancel.changed() => return,
                    result = api.list(&params) => result,
                };

                match listed {
                    Ok(list) => {
                        if let Some(version) = list.metadata.resource_version {
                            resource_version = version;
                        }
                        let records = list
                            .items
                            .iter()
                            .map(|object| KubeObjectRecord::new(object.clone(), map(object)))
                            .collect();
                        if events
                            .send(StoreEvent::Replace(scope.clone(), records))
                            .is_err()
                        {
                            return;
                        }
                        need_list = false;
                    }
                    Err(error) => {
                        let message = format!("Could not list resources: {error}");
                        let _ = events.send(StoreEvent::Error(scope.clone(), message));
                        if is_forbidden(&error) {
                            return;
                        }
                        if !sleep_or_cancel(&mut cancel, RECONNECT_DELAY).await {
                            return;
                        }
                        continue;
                    }
                }
            }
        }

        let params = WatchParams::default().timeout(WATCH_TIMEOUT_SECS);
        let watched = tokio::select! {
            _ = cancel.changed() => return,
            result = api.watch(&params, &resource_version) => result,
        };

        let mut stream = match watched {
            Ok(stream) => Box::pin(stream),
            Err(error) => {
                let message = format!("Could not watch resources: {error}");
                let _ = events.send(StoreEvent::WatchError(scope.clone(), message));
                if is_forbidden(&error) {
                    return;
                }
                if !sleep_or_cancel(&mut cancel, WATCH_DENIED_RETRY).await {
                    return;
                }
                continue;
            }
        };

        let mut relist = false;
        loop {
            let event = tokio::select! {
                _ = cancel.changed() => return,
                event = stream.try_next() => event,
            };

            match event {
                Ok(Some(WatchEvent::Added(object) | WatchEvent::Modified(object))) => {
                    if let Some(version) = object.resource_version() {
                        resource_version = version;
                    }
                    if events
                        .send(StoreEvent::Upsert(KubeObjectRecord::new(
                            object.clone(),
                            map(&object),
                        )))
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(Some(WatchEvent::Deleted(object))) => {
                    if let Some(version) = object.resource_version() {
                        resource_version = version;
                    }
                    let key =
                        ObjectKey::new(object.namespace().unwrap_or_default(), object.name_any());
                    if events.send(StoreEvent::Remove(key)).is_err() {
                        return;
                    }
                }
                Ok(Some(WatchEvent::Bookmark(bookmark))) => {
                    resource_version = bookmark.metadata.resource_version;
                }
                Ok(Some(WatchEvent::Error(status))) => {
                    relist =
                        status.code == 410 || status.reason == "Expired" || status.reason == "Gone";
                    if relist {
                        tracing::debug!(code = status.code, "store: watch expired, relisting");
                    } else if status.code == 403 {
                        let message = format!("Watch error (403): {}", status.message);
                        let _ = events.send(StoreEvent::WatchError(scope.clone(), message));
                        return;
                    } else {
                        let message = format!("Watch error ({}): {}", status.code, status.message);
                        let _ = events.send(StoreEvent::WatchError(scope.clone(), message));
                    }
                    break;
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = events.send(StoreEvent::WatchError(
                        scope.clone(),
                        format!("Watch stream failed: {error}"),
                    ));
                    break;
                }
            }
        }

        if relist {
            need_list = true;
        }

        if !sleep_or_cancel(&mut cancel, RECONNECT_DELAY).await {
            return;
        }
    }
}

fn is_forbidden(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 403)
}

async fn sleep_or_cancel(cancel: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    tokio::select! {
        _ = cancel.changed() => false,
        _ = tokio::time::sleep(duration) => true,
    }
}

/// Spawn the store for one kind. The typed match mirrors the existing watcher
/// dispatch and keeps built-in kinds on their concrete Kubernetes types.
pub(crate) fn spawn(
    kind: ResourceKind,
    client: Client,
    tokio: tokio::runtime::Handle,
    epoch: u64,
    global_watch: bool,
) -> Option<Arc<KubeObjectStoreHandle>> {
    match kind {
        ResourceKind::Pods => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            pod_item,
            namespaced_api::<Pod>,
        )),
        ResourceKind::Deployments => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            deployment_item,
            namespaced_api::<Deployment>,
        )),
        ResourceKind::StatefulSets => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            stateful_set_item,
            namespaced_api::<StatefulSet>,
        )),
        ResourceKind::ReplicaSets => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            replica_set_item,
            namespaced_api::<ReplicaSet>,
        )),
        ResourceKind::ReplicationControllers => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            replication_controller_item,
            namespaced_api::<ReplicationController>,
        )),
        ResourceKind::DaemonSets => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            daemon_set_item,
            namespaced_api::<DaemonSet>,
        )),
        ResourceKind::Jobs => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            job_item,
            namespaced_api::<Job>,
        )),
        ResourceKind::CronJobs => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            cron_job_item,
            namespaced_api::<CronJob>,
        )),
        ResourceKind::HorizontalPodAutoscalers => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            horizontal_pod_autoscaler_item,
            namespaced_api::<HorizontalPodAutoscaler>,
        )),
        ResourceKind::PodDisruptionBudgets => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            pod_disruption_budget_item,
            namespaced_api::<PodDisruptionBudget>,
        )),
        ResourceKind::Services => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            service_item,
            namespaced_api::<Service>,
        )),
        ResourceKind::Ingresses => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            ingress_item,
            namespaced_api::<Ingress>,
        )),
        ResourceKind::Endpoints => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            endpoints_item,
            namespaced_api::<Endpoints>,
        )),
        ResourceKind::EndpointSlices => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            endpoint_slice_item,
            namespaced_api::<EndpointSlice>,
        )),
        ResourceKind::NetworkPolicies => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            network_policy_item,
            namespaced_api::<NetworkPolicy>,
        )),
        ResourceKind::PersistentVolumeClaims => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            pvc_item,
            namespaced_api::<PersistentVolumeClaim>,
        )),
        ResourceKind::StorageClasses => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            storage_class_item,
            cluster_api::<StorageClass>,
        )),
        ResourceKind::ConfigMaps => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            config_map_item,
            namespaced_api::<ConfigMap>,
        )),
        ResourceKind::Secrets => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            secret_item,
            namespaced_api::<Secret>,
        )),
        ResourceKind::ServiceAccounts => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            service_account_item,
            namespaced_api::<ServiceAccount>,
        )),
        ResourceKind::PersistentVolumes => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            pv_item,
            cluster_api::<PersistentVolume>,
        )),
        ResourceKind::Namespaces => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            namespace_item,
            cluster_api::<Namespace>,
        )),
        ResourceKind::Nodes => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            node_item,
            cluster_api::<Node>,
        )),
        ResourceKind::Leases => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            lease_item,
            namespaced_api::<Lease>,
        )),
        ResourceKind::Events => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            event_item,
            namespaced_api::<Event>,
        )),
        ResourceKind::ClusterRoles => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            cluster_role_item,
            cluster_api::<ClusterRole>,
        )),
        ResourceKind::Roles => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            role_item,
            namespaced_api::<Role>,
        )),
        ResourceKind::RoleBindings => Some(spawn_typed(
            kind,
            client,
            tokio,
            epoch,
            global_watch,
            role_binding_item,
            namespaced_api::<RoleBinding>,
        )),
        ResourceKind::Custom(index) => {
            let def = crate::models::resources::catalog::custom_def(index)?;
            let resource = ApiResource::from_gvk_with_plural(
                &GroupVersionKind::gvk(&def.group, &def.version, &def.kind),
                &def.plural,
            );
            let make_api = move |client: &Client, scope: &NamespaceScope| match scope {
                NamespaceScope::All => Api::<DynamicObject>::all_with(client.clone(), &resource),
                NamespaceScope::One(namespace) => {
                    Api::<DynamicObject>::namespaced_with(client.clone(), namespace, &resource)
                }
            };
            Some(spawn_typed(
                kind,
                client,
                tokio,
                epoch,
                global_watch,
                custom_item,
                make_api,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_typed<K, F, M>(
    kind: ResourceKind,
    client: Client,
    tokio: tokio::runtime::Handle,
    epoch: u64,
    global_watch: bool,
    map: F,
    make_api: M,
) -> Arc<KubeObjectStoreHandle>
where
    K: Resource
        + ResourceExt
        + Clone
        + Debug
        + DeserializeOwned
        + Serialize
        + Send
        + Sync
        + 'static
        + crate::repository::store::StoredObject,
    F: Fn(&K) -> Item + Copy + Send + Sync + 'static,
    M: Fn(&Client, &NamespaceScope) -> Api<K> + Send + Sync + 'static,
{
    let mut data = StoreData::new();
    let initial = Arc::new(data.snapshot(SnapshotPhase::Loading, WatchMode::Watch));
    let (snapshots, _) = watch::channel(initial);
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (events, event_rx) = mpsc::unbounded_channel();

    let handle = Arc::new(KubeObjectStoreHandle {
        commands: commands.clone(),
        snapshots: snapshots.clone(),
    });

    let task = StoreTask {
        client,
        tokio: tokio.clone(),
        epoch,
        kind,
        global_watch,
        data,
        scopes: HashMap::new(),
        plan: None,
        watches: Vec::new(),
        events: events.clone(),
        event_rx,
        command_rx,
        snapshots,
        phase: SnapshotPhase::Loading,
        map,
        make_api: Arc::new(make_api),
    };
    tokio.spawn(task.run());

    handle
}

fn namespaced_api<K>(client: &Client, scope: &NamespaceScope) -> Api<K>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned
        + Send
        + 'static,
{
    match scope {
        NamespaceScope::All => Api::<K>::all(client.clone()),
        NamespaceScope::One(namespace) => Api::<K>::namespaced(client.clone(), namespace),
    }
}

fn cluster_api<K>(client: &Client, _: &NamespaceScope) -> Api<K>
where
    K: Resource<DynamicType = ()> + Clone + Debug + DeserializeOwned + Send + 'static,
{
    Api::<K>::all(client.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_initial_events_bookmark_ends_the_stream() {
        let mut annotations = BTreeMap::new();
        assert!(!bookmark_ends_initial_events(&annotations));

        annotations.insert("k8s.io/initial-events-end".into(), "false".into());
        assert!(!bookmark_ends_initial_events(&annotations));

        annotations.insert("k8s.io/initial-events-end".into(), "true".into());
        assert!(bookmark_ends_initial_events(&annotations));
    }
}
