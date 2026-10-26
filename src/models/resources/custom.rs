//! Model for user-defined custom resources: holds the latest immutable snapshot.

use crate::models::{ResourceSnapshot, SnapshotPhase};
use std::sync::Arc;

pub struct CustomResourceModel {
    snapshot: Arc<ResourceSnapshot>,
}

impl CustomResourceModel {
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

impl Default for CustomResourceModel {
    fn default() -> Self {
        Self::new()
    }
}
