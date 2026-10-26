//! Discovery of custom resources installed in the active cluster.
//!
//! Only CustomResourceDefinitions are listed, so built-in Kubernetes APIs are
//! never imported. Known built-in API groups are filtered defensively as well.

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, ListParams};
use tokio::sync::oneshot;

use crate::cluster::ClusterState;
use crate::models::resources::catalog::CustomResourceDef;
use gpui_kit::App;

const BUILTIN_GROUPS: &[&str] = &[
    "",
    "admissionregistration.k8s.io",
    "apiextensions.k8s.io",
    "apiregistration.k8s.io",
    "apps",
    "authentication.k8s.io",
    "authorization.k8s.io",
    "autoscaling",
    "batch",
    "certificates.k8s.io",
    "coordination.k8s.io",
    "discovery.k8s.io",
    "events.k8s.io",
    "flowcontrol.apiserver.k8s.io",
    "internal.apiserver.k8s.io",
    "networking.k8s.io",
    "node.k8s.io",
    "policy",
    "rbac.authorization.k8s.io",
    "resource.k8s.io",
    "scheduling.k8s.io",
    "storage.k8s.io",
    "storagemigration.k8s.io",
];

/// Start a background scan and return a receiver for the result.
pub fn discover(
    cx: &App,
) -> Result<oneshot::Receiver<Result<Vec<CustomResourceDef>, String>>, String> {
    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let (tx, rx) = oneshot::channel();
    tokio.spawn(async move {
        let result = discover_with(client).await;
        let _ = tx.send(result);
    });
    Ok(rx)
}

async fn discover_with(client: kube::Client) -> Result<Vec<CustomResourceDef>, String> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let list = api
        .list(&ListParams::default())
        .await
        .map_err(|error| format!("Could not list custom resource definitions: {error}"))?;

    let mut defs = Vec::new();
    for crd in list.items {
        let spec = &crd.spec;
        if BUILTIN_GROUPS.contains(&spec.group.as_str()) {
            continue;
        }
        let Some(version) = spec
            .versions
            .iter()
            .find(|version| version.storage)
            .or_else(|| spec.versions.iter().find(|version| version.served))
        else {
            continue;
        };
        defs.push(CustomResourceDef {
            group: spec.group.clone(),
            version: version.name.clone(),
            kind: spec.names.kind.clone(),
            plural: spec.names.plural.clone(),
            namespaced: spec.scope == "Namespaced",
        });
    }
    defs.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    defs.dedup();
    Ok(defs)
}
