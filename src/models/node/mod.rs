//! Model for Nodes: holds the latest immutable snapshot and the open node detail.

pub mod detail;

use std::sync::Arc;

use self::detail::{NodeDetailState, NodeDetailUpdate, UsageHistory};
use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};

const MAX_USAGE_SAMPLES: usize = 60;

pub struct NodesModel {
    snapshot: Arc<ResourceSnapshot>,
    detail: NodeDetailState,
    usage: UsageHistory,
    usage_error: Option<String>,
}

impl NodesModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
            detail: NodeDetailState::Closed,
            usage: UsageHistory::new(MAX_USAGE_SAMPLES),
            usage_error: None,
        }
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> ResourceKind {
        ResourceKind::Nodes
    }

    pub fn apply_snapshot(&mut self, snapshot: Arc<ResourceSnapshot>) {
        self.snapshot = snapshot;
    }

    pub fn snapshot(&self) -> &Arc<ResourceSnapshot> {
        &self.snapshot
    }

    pub fn phase(&self) -> &SnapshotPhase {
        self.snapshot.phase()
    }

    pub fn item_count(&self) -> usize {
        self.snapshot.items().len()
    }

    pub fn detail(&self) -> &NodeDetailState {
        &self.detail
    }

    pub fn usage(&self) -> &UsageHistory {
        &self.usage
    }

    pub fn usage_error(&self) -> Option<&str> {
        self.usage_error.as_deref()
    }

    pub fn begin_detail(&mut self, node: impl Into<String>) {
        let node = node.into();
        self.detail = NodeDetailState::Loading { node };
        self.usage.clear();
        self.usage_error = None;
    }

    pub fn close_detail(&mut self) {
        self.detail = NodeDetailState::Closed;
        self.usage.clear();
        self.usage_error = None;
    }

    /// Apply a repository update. Stale updates for a previously selected
    /// node are ignored. Returns true when the state changed.
    pub fn apply_update(&mut self, update: NodeDetailUpdate) -> bool {
        match update {
            NodeDetailUpdate::Detail { node, result } => {
                if self.detail.node() != Some(node.as_str()) {
                    return false;
                }
                match result {
                    Ok(detail) => self.detail = NodeDetailState::Ready(detail),
                    Err(message) => {
                        self.detail = NodeDetailState::Error { node, message };
                        self.usage.clear();
                        self.usage_error = None;
                    }
                }
                true
            }
            NodeDetailUpdate::Usage { node, result } => {
                if self.detail.node() != Some(node.as_str()) {
                    return false;
                }
                match result {
                    Ok(sample) => {
                        self.usage.push(sample);
                        self.usage_error = None;
                    }
                    Err(message) => self.usage_error = Some(message),
                }
                true
            }
        }
    }
}

impl Default for NodesModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use self::detail::{NodeDetail, NodeDetailUpdate, NodeUsage};
    use super::*;

    fn usage(at: i64) -> NodeUsage {
        NodeUsage {
            observed_at: at,
            cpu_used_millicores: Some(250),
            cpu_capacity_millicores: Some(1000),
            memory_used_bytes: Some(256 * 1024 * 1024),
            memory_capacity_bytes: Some(1024 * 1024 * 1024),
        }
    }

    #[test]
    fn ignores_updates_for_a_previously_selected_node() {
        let mut model = NodesModel::new();
        model.begin_detail("worker-a");
        let stale = NodeDetailUpdate::Usage {
            node: "worker-b".into(),
            result: Ok(usage(1)),
        };
        assert!(!model.apply_update(stale));
        assert!(model.usage().samples().is_empty());

        let current = NodeDetailUpdate::Usage {
            node: "worker-a".into(),
            result: Ok(usage(2)),
        };
        assert!(model.apply_update(current));
        assert_eq!(model.usage().samples().len(), 1);
    }

    #[test]
    fn applies_detail_and_keeps_usage_history_bounded() {
        let mut model = NodesModel::new();
        model.begin_detail("worker-a");
        let detail = NodeDetail {
            name: "worker-a".into(),
            ..Default::default()
        };
        assert!(model.apply_update(NodeDetailUpdate::Detail {
            node: "worker-a".into(),
            result: Ok(Box::new(detail)),
        }));
        assert!(matches!(model.detail(), NodeDetailState::Ready(_)));

        for at in 0..(MAX_USAGE_SAMPLES as i64 + 10) {
            model.apply_update(NodeDetailUpdate::Usage {
                node: "worker-a".into(),
                result: Ok(usage(at)),
            });
        }
        assert_eq!(model.usage().samples().len(), MAX_USAGE_SAMPLES);
        assert_eq!(
            model
                .usage()
                .samples()
                .last()
                .map(|sample| sample.observed_at),
            Some(MAX_USAGE_SAMPLES as i64 + 9)
        );
        assert_eq!(
            model.usage().latest().and_then(NodeUsage::cpu_percent),
            Some(25.0)
        );
    }
}
