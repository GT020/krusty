//! Model for NodesModel: holds the latest immutable snapshot.

use std::sync::Arc;
use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};

pub struct NodesModel {
    snapshot: Arc<ResourceSnapshot>,
}

impl NodesModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
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
}

impl Default for NodesModel {
    fn default() -> Self {
        Self::new()
    }
}
