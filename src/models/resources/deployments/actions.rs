//! Deployment action state and events (no gpui or kube dependencies).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Pending,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct ActionFeedback {
    status: ActionStatus,
    message: String,
}

impl ActionFeedback {
    pub fn pending(message: String) -> Self {
        Self {
            status: ActionStatus::Pending,
            message,
        }
    }

    pub fn success(message: String) -> Self {
        Self {
            status: ActionStatus::Success,
            message,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            status: ActionStatus::Error,
            message,
        }
    }

    pub fn status(&self) -> ActionStatus {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Emitted after a successful action so classic detail or a dashboard host can
/// refresh or remove itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentActionEvent {
    Changed,
    Deleted,
}

/// State for one deployment's action surface.
#[derive(Clone, Debug, Default)]
pub struct DeploymentActionsModel {
    replicas: Option<i32>,
    manifest: Option<serde_json::Value>,
    feedback: Option<ActionFeedback>,
}

impl DeploymentActionsModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replicas(&self) -> Option<i32> {
        self.replicas
    }

    pub fn set_replicas(&mut self, replicas: Option<i32>) {
        self.replicas = replicas;
    }

    pub fn manifest(&self) -> Option<&serde_json::Value> {
        self.manifest.as_ref()
    }

    pub fn set_manifest(&mut self, manifest: Option<serde_json::Value>) {
        self.manifest = manifest;
    }

    pub fn feedback(&self) -> Option<&ActionFeedback> {
        self.feedback.as_ref()
    }

    pub fn set_feedback(&mut self, feedback: Option<ActionFeedback>) {
        self.feedback = feedback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_tracks_status_and_message() {
        let mut model = DeploymentActionsModel::new();
        assert!(model.feedback().is_none());

        model.set_feedback(Some(ActionFeedback::pending("Scaling…".into())));
        assert_eq!(
            model.feedback().map(ActionFeedback::status),
            Some(ActionStatus::Pending)
        );

        model.set_feedback(Some(ActionFeedback::success("Scaled.".into())));
        assert_eq!(
            model.feedback().map(ActionFeedback::status),
            Some(ActionStatus::Success)
        );
    }
}
