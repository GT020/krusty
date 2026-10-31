//! Pod usage domain state (no gpui or kube dependencies).

use crate::models::resources::usage::{UsageHistory, UsageSample};

pub const MAX_USAGE_SAMPLES: usize = 60;

/// An update pushed from the repository while a pod usage widget is active.
#[derive(Clone, Debug)]
pub enum PodUsageUpdate {
    Sample {
        namespace: String,
        name: String,
        result: Result<UsageSample, String>,
    },
}

/// Live usage samples for one pod.
#[derive(Clone, Debug)]
pub struct PodUsageModel {
    history: UsageHistory,
    error: Option<String>,
}

impl Default for PodUsageModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PodUsageModel {
    pub fn new() -> Self {
        Self {
            history: UsageHistory::new(MAX_USAGE_SAMPLES),
            error: None,
        }
    }

    pub fn apply_sample(&mut self, sample: UsageSample) {
        self.history.push(sample);
        self.error = None;
    }

    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
    }

    pub fn history(&self) -> &UsageHistory {
        &self.history
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(observed_at: i64) -> UsageSample {
        UsageSample {
            observed_at,
            cpu_used_millicores: Some(25),
            cpu_capacity_millicores: Some(100),
            memory_used_bytes: Some(64 * 1024 * 1024),
            memory_capacity_bytes: Some(256 * 1024 * 1024),
        }
    }

    #[test]
    fn samples_accumulate_and_clear() {
        let mut model = PodUsageModel::new();
        model.apply_sample(sample(1));
        model.apply_sample(sample(2));
        assert_eq!(model.history().samples().len(), 2);
        assert_eq!(
            model.history().latest().map(|sample| sample.observed_at),
            Some(2)
        );

        model.set_error("metrics unavailable".to_string());
        assert_eq!(model.error(), Some("metrics unavailable"));

        model.apply_sample(sample(3));
        assert!(model.error().is_none());
    }
}
