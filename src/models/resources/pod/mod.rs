//! Model for Pods: holds the latest immutable snapshot and the open pod detail.

pub mod detail;

use std::sync::Arc;

use self::detail::{PodDetailState, PodDetailUpdate, UsageHistory};
use crate::models::{ResourceKind, ResourceSnapshot, SnapshotPhase};

const MAX_USAGE_SAMPLES: usize = 60;

pub struct PodsModel {
    snapshot: Arc<ResourceSnapshot>,
    detail: PodDetailState,
    usage: UsageHistory,
    usage_error: Option<String>,
}

impl PodsModel {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ResourceSnapshot::loading()),
            detail: PodDetailState::Closed,
            usage: UsageHistory::new(MAX_USAGE_SAMPLES),
            usage_error: None,
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

    pub fn usage(&self) -> &UsageHistory {
        &self.usage
    }

    pub fn usage_error(&self) -> Option<&str> {
        self.usage_error.as_deref()
    }

    pub fn begin_detail(&mut self, namespace: impl Into<String>, name: impl Into<String>) {
        self.detail = PodDetailState::Loading {
            namespace: namespace.into(),
            name: name.into(),
        };
        self.usage.clear();
        self.usage_error = None;
    }

    pub fn close_detail(&mut self) {
        self.detail = PodDetailState::Closed;
        self.usage.clear();
        self.usage_error = None;
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
                        self.usage.clear();
                        self.usage_error = None;
                    }
                }
                true
            }
            PodDetailUpdate::Usage {
                namespace,
                name,
                result,
            } => {
                if self.detail.identity() != Some((namespace.as_str(), name.as_str())) {
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

impl Default for PodsModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use self::detail::{PodDetail, PodDetailUpdate, PodUsage};
    use super::*;

    fn usage(at: i64) -> PodUsage {
        PodUsage {
            observed_at: at,
            cpu_used_millicores: Some(25),
            cpu_capacity_millicores: Some(100),
            memory_used_bytes: Some(64 * 1024 * 1024),
            memory_capacity_bytes: Some(256 * 1024 * 1024),
        }
    }

    #[test]
    fn ignores_updates_for_a_previously_selected_pod() {
        let mut model = PodsModel::new();
        model.begin_detail("default", "api-a");
        let stale = PodDetailUpdate::Usage {
            namespace: "default".into(),
            name: "api-b".into(),
            result: Ok(usage(1)),
        };
        assert!(!model.apply_update(stale));
        assert!(model.usage().samples().is_empty());
    }

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
