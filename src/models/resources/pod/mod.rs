//! Model for Pods: holds the latest immutable snapshot and the open pod detail.

pub mod detail;
pub mod exec;
pub mod logs;
pub mod usage;

use std::sync::Arc;

use self::detail::{PodDetailState, PodDetailUpdate};
use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};

pub struct PodsModel {
    snapshot: Arc<ResourceSnapshot>,
    detail: PodDetailState,
}

impl PodsModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
            detail: PodDetailState::Closed,
        }
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> ResourceKind {
        ResourceKind::Pods
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

    pub fn detail(&self) -> &PodDetailState {
        &self.detail
    }

    pub fn begin_detail(&mut self, namespace: impl Into<String>, name: impl Into<String>) {
        self.detail = PodDetailState::Loading {
            namespace: namespace.into(),
            name: name.into(),
        };
    }

    pub fn close_detail(&mut self) {
        self.detail = PodDetailState::Closed;
    }

    /// Apply a repository update. Stale updates for a previously selected pod
    /// are ignored. Returns true when the state changed.
    pub fn apply_update(&mut self, update: PodDetailUpdate) -> bool {
        match update {
            PodDetailUpdate::Detail {
                namespace,
                name,
                result,
            } => {
                if self.detail.identity() != Some((namespace.as_str(), name.as_str())) {
                    return false;
                }
                match result {
                    Ok(detail) => self.detail = PodDetailState::Ready(detail),
                    Err(message) => {
                        self.detail = PodDetailState::Error {
                            namespace,
                            name,
                            message,
                        };
                    }
                }
                true
            }
        }
    }
}

impl Default for PodsModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use self::detail::{PodDetail, PodDetailUpdate};
    use super::*;

    #[test]
    fn applies_detail_only_for_the_selected_namespace_and_name() {
        let mut model = PodsModel::new();
        model.begin_detail("default", "api-a");
        let mismatch = PodDetailUpdate::Detail {
            namespace: "other".into(),
            name: "api-a".into(),
            result: Ok(Box::new(PodDetail::default())),
        };
        assert!(!model.apply_update(mismatch));

        let detail = PodDetail {
            name: "api-a".into(),
            namespace: "default".into(),
            ..Default::default()
        };
        assert!(model.apply_update(PodDetailUpdate::Detail {
            namespace: "default".into(),
            name: "api-a".into(),
            result: Ok(Box::new(detail)),
        }));
        assert!(matches!(model.detail(), PodDetailState::Ready(_)));
    }
}
