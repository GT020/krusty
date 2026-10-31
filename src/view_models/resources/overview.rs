//! ViewModel for the cluster overview dashboard.
//!
//! Independent of the Nodes and Pods screens: it subscribes directly to the
//! shared repository streams for the two APIs whose data it aggregates, and
//! polls the cluster-wide NodeMetrics API for node usage.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::{Context, Subscription, Window};
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::node::detail::NodeUsage;
use crate::models::resources::overview::ClusterOverviewStats;
use crate::models::{NamespaceScope, ResourceKind, ResourceSnapshot};
use crate::repository;

pub struct ClusterOverviewViewModel {
    active: Arc<AtomicBool>,
    epoch: u64,
    node_snapshot: Arc<ResourceSnapshot>,
    pod_snapshot: Arc<ResourceSnapshot>,
    stats: ClusterOverviewStats,
    metrics: BTreeMap<String, NodeUsage>,
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
            this.subscribe(cx);
            this.subscribe_metrics(cx);
        });
        let mut this = Self {
            active: Arc::new(AtomicBool::new(true)),
            epoch,
            node_snapshot: Arc::new(ResourceSnapshot::loading()),
            pod_snapshot: Arc::new(ResourceSnapshot::loading()),
            stats: ClusterOverviewStats::default(),
            metrics: BTreeMap::new(),
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

    pub fn nodes(&self) -> &[Arc<crate::models::Item>] {
        self.node_snapshot.items()
    }

    pub fn pods(&self) -> &[Arc<crate::models::Item>] {
        self.pod_snapshot.items()
    }

    pub fn metrics(&self) -> &BTreeMap<String, NodeUsage> {
        &self.metrics
    }

    pub fn metrics_error(&self) -> Option<&str> {
        self.metrics_error.as_deref()
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        let was_active = self.active.swap(active, Ordering::Relaxed);
        if was_active == active {
            return;
        }
        if active {
            self.recompute();
            cx.notify();
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
            self.node_snapshot = nodes.snapshot();
        }
        if let Some(pods) = &self.pods {
            self.pod_snapshot = pods.snapshot();
        }
        self.recompute();

        let (cancel, mut cancel_rx) = watch::channel(());
        self.pump_cancel = Some(cancel);
        let mut nodes = self.nodes.clone();
        let mut pods = self.pods.clone();
        let active = Arc::clone(&self.active);
        cx.spawn(async move |this, cx| {
            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => break,
                    snapshot = next_snapshot(&mut nodes) => {
                        let Some(snapshot) = snapshot else {
                            nodes = None;
                            continue;
                        };
                        let should_notify = active.load(Ordering::Relaxed);
                        let applied = this.update(cx, |this, cx| {
                            this.node_snapshot = snapshot;
                            this.recompute();
                            if should_notify {
                                cx.notify();
                            }
                        });
                        if applied.is_err() {
                            break;
                        }
                    }
                    snapshot = next_snapshot(&mut pods) => {
                        let Some(snapshot) = snapshot else {
                            pods = None;
                            continue;
                        };
                        let should_notify = active.load(Ordering::Relaxed);
                        let applied = this.update(cx, |this, cx| {
                            this.pod_snapshot = snapshot;
                            this.recompute();
                            if should_notify {
                                cx.notify();
                            }
                        });
                        if applied.is_err() {
                            break;
                        }
                    }
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn recompute(&mut self) {
        self.stats =
            ClusterOverviewStats::from_items(self.node_snapshot.items(), self.pod_snapshot.items());
    }

    fn cancel_pump(&mut self) {
        if let Some(cancel) = self.pump_cancel.take() {
            let _ = cancel.send(());
        }
        self.nodes = None;
        self.pods = None;
    }

    fn subscribe_metrics(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.metrics_cancel.take() {
            let _ = cancel.send(true);
        }
        self.metrics.clear();
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
                            this.metrics = metrics;
                            this.metrics_error = None;
                        }
                        Err(error) => {
                            this.metrics.clear();
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
