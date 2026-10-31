//! One-shot deployment summary used by the actions widget.

use k8s_openapi::api::apps::v1::Deployment;
use kube::Client;
use kube::api::Api;
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use gpui_kit::App;

#[derive(Clone, Debug, Default)]
pub struct DeploymentSummary {
    pub replicas: Option<i32>,
    pub manifest: Option<serde_json::Value>,
}

/// A one-shot summary fetch. Dropping it cancels the background task.
pub struct DeploymentSummarySubscription {
    updates: mpsc::UnboundedReceiver<Result<DeploymentSummary, String>>,
    cancel: watch::Sender<bool>,
}

impl DeploymentSummarySubscription {
    pub async fn next(&mut self) -> Option<Result<DeploymentSummary, String>> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for DeploymentSummarySubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

pub fn subscribe_summary(
    cx: &mut App,
    namespace: &str,
    name: &str,
) -> Result<DeploymentSummarySubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_summary(
        client,
        namespace.to_string(),
        name.to_string(),
        updates_tx,
        cancel_rx,
    ));
    Ok(DeploymentSummarySubscription { updates, cancel })
}

async fn watch_summary(
    client: Client,
    namespace: String,
    name: String,
    updates: mpsc::UnboundedSender<Result<DeploymentSummary, String>>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Deployment> = Api::namespaced(client, &namespace);
    let fetched = tokio::select! {
        _ = cancel.changed() => return,
        result = api.get(&name) => result,
    };
    let result = match fetched {
        Ok(deployment) => Ok(summary_from_deployment(&deployment)),
        Err(error) => Err(format!("Could not load deployment “{name}”: {error}")),
    };
    let _ = updates.send(result);
}

fn summary_from_deployment(deployment: &Deployment) -> DeploymentSummary {
    DeploymentSummary {
        replicas: deployment.spec.as_ref().and_then(|spec| spec.replicas),
        manifest: serde_json::to_value(deployment).ok(),
    }
}
