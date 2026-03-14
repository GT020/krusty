use kube::{Client, Api};
use kube::api::ListParams;
use k8s_openapi::api::core::v1::{Pod, Node};
use k8s_openapi::api::apps::v1::Deployment;

pub async fn create_client() -> Result<Client, kube::Error> {
    Client::try_default().await
}

pub async fn fetch_pods(client: &Client, namespace: Option<&str>) -> Result<Vec<Pod>, kube::Error> {
    let pods: Api<Pod> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = pods.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_nodes(client: &Client) -> Result<Vec<Node>, kube::Error> {
    let nodes: Api<Node> = Api::all(client.clone());
    let list = nodes.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_deployments(client: &Client, namespace: Option<&str>) -> Result<Vec<Deployment>, kube::Error> {
    let deps: Api<Deployment> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = deps.list(&ListParams::default()).await?;
    Ok(list.items)
}
