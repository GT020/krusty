//! Repository for Deployments: live list/watch, actions, and summary reads.

pub mod summary;

use k8s_openapi::api::apps::v1::Deployment;
use kube::Client;
use kube::api::{Api, DeleteParams, Patch, PatchParams, PostParams};
use serde_json::json;

use crate::models::{NamespaceScope, ResourceKind};
use crate::repository::{Subscription, subscribe_kind};
use gpui_kit::App;

/// Subscribe to the live Deployments stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App, scope: NamespaceScope) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::Deployments, scope)
}

fn api(client: Client, namespace: &str) -> Api<Deployment> {
    Api::namespaced(client, namespace)
}

/// Set the desired replica count for a deployment.
pub async fn scale(
    client: Client,
    namespace: String,
    name: String,
    replicas: i32,
) -> Result<(), String> {
    let patch = json!({ "spec": { "replicas": replicas } });
    api(client, &namespace)
        .patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not scale deployment “{name}”: {error}"))
}

/// Trigger a rolling restart by updating the pod template annotation.
pub async fn restart(client: Client, namespace: String, name: String) -> Result<(), String> {
    let restarted_at = jiff::Timestamp::now().to_string();
    let patch = json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": restarted_at
                    }
                }
            }
        }
    });
    api(client, &namespace)
        .patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not restart deployment “{name}”: {error}"))
}

/// Delete a deployment and the pods it manages.
pub async fn delete(client: Client, namespace: String, name: String) -> Result<(), String> {
    api(client, &namespace)
        .delete(&name, &DeleteParams::default())
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not delete deployment “{name}”: {error}"))
}

/// Replace a deployment with the supplied YAML manifest.
pub async fn replace(
    client: Client,
    namespace: String,
    name: String,
    yaml: String,
) -> Result<(), String> {
    let mut deployment: Deployment =
        serde_saphyr::from_str(&yaml).map_err(|error| format!("Invalid YAML: {error}"))?;
    deployment.metadata.name = Some(name.clone());
    deployment.metadata.namespace = Some(namespace.clone());
    api(client, &namespace)
        .replace(&name, &PostParams::default(), &deployment)
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not update deployment “{name}”: {error}"))
}
