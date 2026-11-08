//! Snapshot model shared by every built-in resource kind and custom resources.

use std::sync::Arc;

use crate::models::{ResourceSnapshot, SnapshotPhase};

/// Holds the latest immutable snapshot for one resource list.
pub struct ResourceModel {
    snapshot: Arc<ResourceSnapshot>,
}

impl ResourceModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
        }
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

impl Default for ResourceModel {
    fn default() -> Self {
        Self::new()
    }
}
