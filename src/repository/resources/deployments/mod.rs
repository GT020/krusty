//! Repository for Deployments: subscribe to the live list/watch stream.

use k8s_openapi::api::apps::v1::Deployment;
use kube::Client;
use kube::api::{Api, DeleteParams, Patch, PatchParams, PostParams};
use serde_json::json;

fn api(client: Client, namespace: &str) -> Api<Deployment> {
    Api::namespaced(client, namespace)
}

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

pub async fn delete(client: Client, namespace: String, name: String) -> Result<(), String> {
    api(client, &namespace)
        .delete(&name, &DeleteParams::default())
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not delete deployment “{name}”: {error}"))
}

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
