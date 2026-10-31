//! Cluster-wide health summary derived from the Nodes and Pods snapshots.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::Item;

/// Normalized pod status used by the overview breakdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodHealth {
    Running,
    Pending,
    Succeeded,
    Failed,
    CrashLoopBackOff,
    Unknown,
}

impl PodHealth {
    pub fn from_status(status: &str) -> Self {
        match status.to_ascii_lowercase().as_str() {
            "running" => Self::Running,
            "pending" => Self::Pending,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "crashloopbackoff" => Self::CrashLoopBackOff,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Pending => "Pending",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::CrashLoopBackOff => "CrashLoopBackOff",
            Self::Unknown => "Unknown",
        }
    }

    pub fn is_healthy(self) -> bool {
        matches!(self, Self::Running | Self::Succeeded)
    }
}

/// Counts shown on the cluster overview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClusterOverviewStats {
    pub nodes_total: usize,
    pub nodes_ready: usize,
    pub pods_total: usize,
    pub pods_running: usize,
    pub pods_pending: usize,
    pub pods_succeeded: usize,
    pub pods_failed: usize,
    pub pods_crashloopbackoff: usize,
    pub pods_unknown: usize,
    pub pod_restarts: usize,
    /// Pods scheduled on each node, keyed by node name.
    pub pods_by_node: BTreeMap<String, usize>,
}

impl ClusterOverviewStats {
    pub fn from_items(nodes: &[Arc<Item>], pods: &[Arc<Item>]) -> Self {
        let mut stats = Self {
            nodes_total: nodes.len(),
            pods_total: pods.len(),
            ..Self::default()
        };
        for node in nodes {
            if node.status == "ready" {
                stats.nodes_ready += 1;
            }
        }
        for pod in pods {
            stats.pod_restarts += pod.restarts;
            if !pod.node.is_empty() {
                *stats.pods_by_node.entry(pod.node.clone()).or_default() += 1;
            }
            match PodHealth::from_status(&pod.status) {
                PodHealth::Running => stats.pods_running += 1,
                PodHealth::Pending => stats.pods_pending += 1,
                PodHealth::Succeeded => stats.pods_succeeded += 1,
                PodHealth::Failed => stats.pods_failed += 1,
                PodHealth::CrashLoopBackOff => stats.pods_crashloopbackoff += 1,
                PodHealth::Unknown => stats.pods_unknown += 1,
            }
        }
        stats
    }

    pub fn nodes_not_ready(&self) -> usize {
        self.nodes_total.saturating_sub(self.nodes_ready)
    }

    /// Pods that are not running successfully or already completed.
    pub fn pods_unhealthy(&self) -> usize {
        self.pods_pending + self.pods_failed + self.pods_crashloopbackoff + self.pods_unknown
    }

    pub fn pods_on(&self, node: &str) -> usize {
        self.pods_by_node.get(node).copied().unwrap_or(0)
    }
}

pub fn is_unhealthy_pod(item: &Item) -> bool {
    !PodHealth::from_status(&item.status).is_healthy()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, status: &str) -> Arc<Item> {
        let mut item = Item::new(name.to_string(), Some("default".to_string()));
        item.status = status.to_string();
        Arc::new(item)
    }

    #[test]
    fn summarizes_node_readiness() {
        let nodes = vec![item("worker-a", "ready"), item("worker-b", "notready")];

        let stats = ClusterOverviewStats::from_items(&nodes, &[]);

        assert_eq!(stats.nodes_total, 2);
        assert_eq!(stats.nodes_ready, 1);
        assert_eq!(stats.nodes_not_ready(), 1);
    }

    #[test]
    fn summarizes_pod_health_and_restarts() {
        let mut crash = Item::new("crash".to_string(), Some("default".to_string()));
        crash.status = "crashloopbackoff".to_string();
        crash.restarts = 4;
        let pods = vec![
            item("api", "running"),
            item("batch", "succeeded"),
            item("pending", "pending"),
            item("failed", "failed"),
            item("other", "containercreating"),
            Arc::new(crash),
        ];

        let stats = ClusterOverviewStats::from_items(&[], &pods);

        assert_eq!(stats.pods_total, 6);
        assert_eq!(stats.pods_running, 1);
        assert_eq!(stats.pods_succeeded, 1);
        assert_eq!(stats.pods_pending, 1);
        assert_eq!(stats.pods_failed, 1);
        assert_eq!(stats.pods_crashloopbackoff, 1);
        assert_eq!(stats.pods_unknown, 1);
        assert_eq!(stats.pods_unhealthy(), 4);
        assert_eq!(stats.pod_restarts, 4);
    }

    #[test]
    fn counts_pods_per_node() {
        let mut pod_a = Item::new("a".to_string(), Some("default".to_string()));
        pod_a.node = "worker-a".to_string();
        let mut pod_b = Item::new("b".to_string(), Some("default".to_string()));
        pod_b.node = "worker-a".to_string();
        let mut pod_c = Item::new("c".to_string(), Some("default".to_string()));
        pod_c.node = "worker-b".to_string();
        let unscheduled = Item::new("d".to_string(), Some("default".to_string()));

        let stats = ClusterOverviewStats::from_items(
            &[],
            &[
                Arc::new(pod_a),
                Arc::new(pod_b),
                Arc::new(pod_c),
                Arc::new(unscheduled),
            ],
        );

        assert_eq!(stats.pods_on("worker-a"), 2);
        assert_eq!(stats.pods_on("worker-b"), 1);
        assert_eq!(stats.pods_on("missing"), 0);
        assert_eq!(stats.pods_by_node.len(), 2);
    }
}
