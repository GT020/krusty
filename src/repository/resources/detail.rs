//! Generic one-shot detail fetch for every Kubernetes resource kind.

use kube::api::{Api, DynamicObject};
use kube::core::{ApiResource, GroupVersionKind};
use serde_json::Value;
use tokio::sync::{mpsc, watch};

use crate::cluster::ClusterState;
use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailUpdate};
use gpui_kit::App;

pub struct ResourceDetailSubscription {
    updates: mpsc::UnboundedReceiver<ResourceDetailUpdate>,
    cancel: watch::Sender<bool>,
}

impl ResourceDetailSubscription {
    pub async fn next(&mut self) -> Option<ResourceDetailUpdate> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl Drop for ResourceDetailSubscription {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

pub fn subscribe_detail(
    cx: &mut App,
    kind: ResourceKind,
    namespace: &str,
    name: &str,
) -> Result<ResourceDetailSubscription, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    tokio.spawn(watch_detail(
        client,
        kind,
        namespace.to_string(),
        name.to_string(),
        updates_tx,
        cancel_rx,
    ));
    Ok(ResourceDetailSubscription { updates, cancel })
}

async fn watch_detail(
    client: kube::Client,
    kind: ResourceKind,
    namespace: String,
    name: String,
    updates: mpsc::UnboundedSender<ResourceDetailUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    let resource = api_resource(kind);
    let api: Api<DynamicObject> = if kind.is_namespaced() {
        Api::namespaced_with(client, &namespace, &resource)
    } else {
        Api::all_with(client, &resource)
    };
    let fetched = tokio::select! {
        _ = cancel.changed() => return,
        result = api.get(&name) => result,
    };
    let result = match fetched {
        Ok(object) => {
            let mut value = serde_json::to_value(&object).unwrap_or(Value::Null);
            sanitize_secret(kind, &mut value);
            Ok(Box::new(ResourceDetail {
                kind,
                namespace: namespace.clone(),
                name: name.clone(),
                raw_json: serde_json::to_string_pretty(&value).unwrap_or_default(),
                value,
            }))
        }
        Err(error) => Err(format!("Could not load {} details: {error}", kind.label())),
    };
    let _ = updates.send(ResourceDetailUpdate {
        kind,
        namespace,
        name,
        result,
    });
}

fn api_resource(kind: ResourceKind) -> ApiResource {
    let (group, version, kind_name, plural) = match kind {
        ResourceKind::Pods => ("", "v1", "Pod", "pods"),
        ResourceKind::Deployments => ("apps", "v1", "Deployment", "deployments"),
        ResourceKind::DaemonSets => ("apps", "v1", "DaemonSet", "daemonsets"),
        ResourceKind::Jobs => ("batch", "v1", "Job", "jobs"),
        ResourceKind::Services => ("", "v1", "Service", "services"),
        ResourceKind::Ingresses => ("networking.k8s.io", "v1", "Ingress", "ingresses"),
        ResourceKind::PersistentVolumes => ("", "v1", "PersistentVolume", "persistentvolumes"),
        ResourceKind::PersistentVolumeClaims => {
            ("", "v1", "PersistentVolumeClaim", "persistentvolumeclaims")
        }
        ResourceKind::ConfigMaps => ("", "v1", "ConfigMap", "configmaps"),
        ResourceKind::Secrets => ("", "v1", "Secret", "secrets"),
        ResourceKind::Nodes => ("", "v1", "Node", "nodes"),
    };
    ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(group, version, kind_name), plural)
}

fn sanitize_secret(kind: ResourceKind, value: &mut Value) {
    if kind != ResourceKind::Secrets {
        return;
    }
    for key in ["data", "stringData"] {
        if let Some(values) = value.get_mut(key).and_then(Value::as_object_mut) {
            for value in values.values_mut() {
                *value = Value::String("••••••".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn masks_secret_values() {
        let mut value = json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "data": { "token": "c2VjcmV0" },
            "stringData": { "password": "plain" },
            "type": "Opaque"
        });
        sanitize_secret(ResourceKind::Secrets, &mut value);
        assert_eq!(value["data"]["token"], "••••••");
        assert_eq!(value["stringData"]["password"], "••••••");
        assert_eq!(value["type"], "Opaque");
    }

    #[test]
    fn maps_ingresses_to_their_real_plural() {
        assert_eq!(api_resource(ResourceKind::Ingresses).plural, "ingresses");
        assert_eq!(
            api_resource(ResourceKind::PersistentVolumeClaims).plural,
            "persistentvolumeclaims"
        );
        assert_eq!(api_resource(ResourceKind::Nodes).group, "");
    }
}
