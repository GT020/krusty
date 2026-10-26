//! Model for CronJobsModel: holds the latest immutable snapshot.

use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};
use std::sync::Arc;

pub struct CronJobsModel {
    snapshot: Arc<ResourceSnapshot>,
}

impl CronJobsModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
        }
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> ResourceKind {
        ResourceKind::CronJobs
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

impl Default for CronJobsModel {
    fn default() -> Self {
        Self::new()
    }
}
