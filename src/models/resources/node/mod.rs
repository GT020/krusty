//! Model for Nodes: holds the latest immutable snapshot and the open node detail.

pub mod detail;
pub mod usage;

use std::sync::Arc;

use self::detail::{NodeDetailState, NodeDetailUpdate};
use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};

pub struct NodesModel {
    snapshot: Arc<ResourceSnapshot>,
    detail: NodeDetailState,
}

impl NodesModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
            detail: NodeDetailState::Closed,
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

    pub fn begin_detail(&mut self, node: impl Into<String>) {
        let node = node.into();
        self.detail = NodeDetailState::Loading { node };
    }

    pub fn close_detail(&mut self) {
        self.detail = NodeDetailState::Closed;
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
                    }
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
    use self::detail::{NodeDetail, NodeDetailUpdate};
    use super::*;

    #[test]
    fn ignores_detail_updates_for_a_previously_selected_node() {
        let mut model = NodesModel::new();
        model.begin_detail("worker-a");
        let stale = NodeDetailUpdate::Detail {
            node: "worker-b".into(),
            result: Ok(Box::new(NodeDetail::default())),
        };
        assert!(!model.apply_update(stale));

        let detail = NodeDetail {
            name: "worker-a".into(),
            ..Default::default()
        };
        assert!(model.apply_update(NodeDetailUpdate::Detail {
            node: "worker-a".into(),
            result: Ok(Box::new(detail)),
        }));
        assert!(matches!(model.detail(), NodeDetailState::Ready(_)));
    }
}
