//! ViewModel for the cluster overview dashboard.
//!
//! Independent of the Nodes and Pods screens: it subscribes directly to the
//! shared repository streams for the two APIs whose data it aggregates, and
//! polls the cluster-wide NodeMetrics API for node usage.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::{Context, Subscription, Window};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::node::detail::NodeUsage;
use crate::models::resources::overview::{ClusterOverviewStats, is_unhealthy_pod};
use crate::models::{Item, NamespaceScope, ResourceKind, ResourceSnapshot};
use crate::repository;

/// Pods change constantly on a busy cluster. The overview only shows
/// aggregate counts and a handful of rows, so coalesce watch snapshots instead
/// of recomputing and repainting for every 50 ms repository flush.
const OVERVIEW_REFRESH_INTERVAL: Duration = Duration::from_millis(250);
/// Rows shown in the "Pods needing attention" preview.
const UNHEALTHY_PREVIEW: usize = 8;

pub struct ClusterOverviewViewModel {
    active: Arc<AtomicBool>,
    epoch: u64,
    node_snapshot: Arc<ResourceSnapshot>,
    pod_snapshot: Arc<ResourceSnapshot>,
    stats: ClusterOverviewStats,
    /// Sorted unhealthy pods, truncated to [`UNHEALTHY_PREVIEW`], derived once
    /// per refresh so rendering does not rescan every pod.
    unhealthy: Arc<[Arc<Item>]>,
    metrics: Arc<BTreeMap<String, NodeUsage>>,
    metrics_error: Option<String>,
    metrics_generation: u64,
    nodes: Option<repository::Subscription>,
    pods: Option<repository::Subscription>,
    pump_cancel: Option<watch::Sender<()>>,
    metrics_cancel: Option<watch::Sender<bool>>,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the cluster observer
    cluster_sub: Subscription,
}

impl ClusterOverviewViewModel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let epoch = cx.global::<ClusterState>().context_epoch();
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let epoch = cx.global::<ClusterState>().context_epoch();
            if epoch == this.epoch {
                return;
            }
            this.epoch = epoch;
            if !this.is_active() {
                this.cancel_pump();
                this.cancel_metrics();
                return;
            }
            this.subscribe(cx);
            this.subscribe_metrics(cx);
        });
        let mut this = Self {
            active: Arc::new(AtomicBool::new(true)),
            epoch,
            node_snapshot: Arc::new(ResourceSnapshot::loading()),
            pod_snapshot: Arc::new(ResourceSnapshot::loading()),
            stats: ClusterOverviewStats::default(),
            unhealthy: Arc::from(Vec::<Arc<Item>>::new()),
            metrics: Arc::new(BTreeMap::new()),
            metrics_error: None,
            metrics_generation: 0,
            nodes: None,
            pods: None,
            pump_cancel: None,
            metrics_cancel: None,
            cluster_sub,
        };
        this.subscribe(cx);
        this.subscribe_metrics(cx);
        this
    }

    pub fn stats(&self) -> &ClusterOverviewStats {
        &self.stats
    }

    /// O(1) clone of the current nodes snapshot; the render path uses it
    /// instead of copying every `Arc<Item>` into a temporary `Vec`.
    pub fn node_snapshot(&self) -> Arc<ResourceSnapshot> {
        self.node_snapshot.clone()
    }

    /// Top unhealthy pods, already sorted and truncated for the preview.
    pub fn unhealthy_pods(&self) -> Arc<[Arc<Item>]> {
        self.unhealthy.clone()
    }

    pub fn metrics(&self) -> &Arc<BTreeMap<String, NodeUsage>> {
        &self.metrics
    }

    pub fn metrics_error(&self) -> Option<&str> {
        self.metrics_error.as_deref()
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        let was_active = self.active.swap(active, Ordering::Relaxed);
        if was_active == active {
            return;
        }
        if let Some(nodes) = &self.nodes {
            nodes.set_paused(!active);
        }
        if let Some(pods) = &self.pods {
            pods.set_paused(!active);
        }
        if active {
            if self.nodes.is_none() || self.pods.is_none() {
                self.subscribe(cx);
                self.subscribe_metrics(cx);
                cx.notify();
                return;
            }
            if let Some(nodes) = &self.nodes {
                self.node_snapshot = nodes.snapshot();
            }
            if let Some(pods) = &self.pods {
                self.pod_snapshot = pods.snapshot();
            }
            self.subscribe_metrics(cx);
            self.recompute();
            cx.notify();
        } else {
            self.cancel_metrics();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.nodes.is_none() || self.pods.is_none() {
            self.subscribe(cx);
        } else {
            if let Some(nodes) = &self.nodes {
                nodes.refresh();
            }
            if let Some(pods) = &self.pods {
                pods.refresh();
            }
        }
        self.subscribe_metrics(cx);
        cx.notify();
    }

    /// Subscribe to the Nodes and Pods streams and pump both into one summary.
    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.cancel_pump();
        self.nodes = match repository::subscribe(cx, ResourceKind::Nodes, NamespaceScope::All) {
            Ok(subscription) => Some(subscription),
            Err(error) => {
                self.node_snapshot = Arc::new(ResourceSnapshot::failed(error));
                None
            }
        };
        self.pods = match repository::subscribe(cx, ResourceKind::Pods, NamespaceScope::All) {
            Ok(subscription) => Some(subscription),
            Err(error) => {
                self.pod_snapshot = Arc::new(ResourceSnapshot::failed(error));
                None
            }
        };
        if let Some(nodes) = &self.nodes {
            nodes.set_paused(!self.is_active());
            self.node_snapshot = nodes.snapshot();
        }
        if let Some(pods) = &self.pods {
            pods.set_paused(!self.is_active());
            self.pod_snapshot = pods.snapshot();
        }
        self.recompute();

        let (cancel, mut cancel_rx) = watch::channel(());
        self.pump_cancel = Some(cancel);
        let mut nodes = self.nodes.clone();
        let mut pods = self.pods.clone();
        cx.spawn(async move |this, cx| {
            let mut last_refresh: Option<Instant> = None;
            loop {
                let update = tokio::select! {
                    _ = cancel_rx.changed() => break,
                    snapshot = next_snapshot(&mut nodes) => match snapshot {
                        Some(snapshot) => Some((false, snapshot)),
                        None => {
                            nodes = None;
                            None                        }
                    },
                    snapshot = next_snapshot(&mut pods) => match snapshot {
                        Some(snapshot) => Some((true, snapshot)),
                        None => {
                            pods = None;
                            None
                        }
                    },
                };
                let Some((pod_update, snapshot)) = update else {
                    if nodes.is_none() && pods.is_none() {
                        break;
                    }
                    continue;
                };

                // Coalesce bursts: watch channels keep only the latest value,
                // so waiting here drops intermediate snapshots instead of
                // recomputing and repainting for each one.
                if let Some(last) = last_refresh {
                    let elapsed = last.elapsed();
                    if elapsed < OVERVIEW_REFRESH_INTERVAL {
                        tokio::select! {
                            _ = cancel_rx.changed() => break,
                            _ = cx
                                .background_executor()
                                .timer(OVERVIEW_REFRESH_INTERVAL - elapsed) => {}                        }
                    }
                }
                last_refresh = Some(Instant::now());

                let applied = this.update(cx, |this, cx| {
                    if pod_update {
                        this.pod_snapshot = snapshot;
                    } else {
                        this.node_snapshot = snapshot;
                    }
                    // A hidden tab may still finish one in-flight refresh.
                    // Keep the snapshot but skip the work until `set_active`
                    // recomputes on resume.
                    if this.is_active() {
                        this.recompute();
                        cx.notify();
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn recompute(&mut self) {
        self.stats =
            ClusterOverviewStats::from_items(self.node_snapshot.items(), self.pod_snapshot.items());

        let mut unhealthy: Vec<Arc<Item>> = self
            .pod_snapshot
            .items()
            .iter()
            .filter(|pod| is_unhealthy_pod(pod))
            .cloned()
            .collect();
        unhealthy.sort_by(|left, right| {
            left.namespace
                .cmp(&right.namespace)
                .then_with(|| left.name.cmp(&right.name))
        });
        unhealthy.truncate(UNHEALTHY_PREVIEW);
        self.unhealthy = Arc::from(unhealthy);
    }

    fn cancel_pump(&mut self) {
        if let Some(cancel) = self.pump_cancel.take() {
            let _ = cancel.send(());
        }
        self.nodes = None;
        self.pods = None;
    }

    fn cancel_metrics(&mut self) {
        if let Some(cancel) = self.metrics_cancel.take() {
            let _ = cancel.send(true);
        }
        self.metrics = Arc::new(BTreeMap::new());
        self.metrics_error = None;
        self.metrics_generation = self.metrics_generation.wrapping_add(1);
    }

    fn subscribe_metrics(&mut self, cx: &mut Context<Self>) {
        if !self.is_active() {
            self.cancel_metrics();
            return;
        }
        if let Some(cancel) = self.metrics_cancel.take() {
            let _ = cancel.send(true);
        }
        self.metrics = Arc::new(BTreeMap::new());
        self.metrics_error = None;
        self.metrics_generation = self.metrics_generation.wrapping_add(1);
        let generation = self.metrics_generation;

        let mut subscription = match repository::resources::nodes::subscribe_metrics(cx) {
            Ok(subscription) => subscription,
            Err(error) => {
                self.metrics_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.metrics_cancel = Some(subscription.cancel_handle());
        cx.spawn(async move |this, cx| {
            while let Some(update) = subscription.next().await {
                let applied = this.update(cx, |this, cx| {
                    if this.metrics_generation != generation {
                        return false;
                    }
                    match update {
                        Ok(metrics) => {
                            this.metrics = Arc::new(metrics);
                            this.metrics_error = None;
                        }
                        Err(error) => {
                            this.metrics = Arc::new(BTreeMap::new());
                            this.metrics_error = Some(error);
                        }
                    }
                    cx.notify();
                    true
                });
                if !matches!(applied, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
}

impl Drop for ClusterOverviewViewModel {
    fn drop(&mut self) {
        if let Some(cancel) = self.pump_cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(cancel) = self.metrics_cancel.take() {
            let _ = cancel.send(true);
        }
    }
}

async fn next_snapshot(
    subscription: &mut Option<repository::Subscription>,
) -> Option<Arc<ResourceSnapshot>> {
    match subscription {
        Some(subscription) => subscription.next().await,
        None => std::future::pending().await,
    }
}
