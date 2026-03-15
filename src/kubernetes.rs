use kube::{api::{Api, DynamicObject}, Client};
use kube::api::ListParams;
use k8s_openapi::api::core::v1::{Event, Node, Pod, Secret, Service};
use k8s_openapi::api::apps::v1::Deployment;

pub async fn create_client() -> Result<Client, kube::Error> {
    Client::try_default().await
}

pub async fn fetch_pods(client: &Client, namespace: Option<&str>) -> Result<Vec<Pod>, kube::Error> {
    let api: Api<Pod> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_nodes(client: &Client) -> Result<Vec<Node>, kube::Error> {
    let api: Api<Node> = Api::all(client.clone());
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_deployments(client: &Client, namespace: Option<&str>) -> Result<Vec<Deployment>, kube::Error> {
    let api: Api<Deployment> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_secrets(client: &Client, namespace: Option<&str>) -> Result<Vec<Secret>, kube::Error> {
    let api: Api<Secret> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_events(client: &Client, namespace: Option<&str>) -> Result<Vec<Event>, kube::Error> {
    let api: Api<Event> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_services(client: &Client, namespace: Option<&str>) -> Result<Vec<Service>, kube::Error> {
    let api: Api<Service> = if let Some(ns) = namespace {
        Api::namespaced(client.clone(), ns)
    } else {
        Api::all(client.clone())
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

pub async fn fetch_ingressroutes(client: &Client, namespace: Option<&str>) -> Result<Vec<DynamicObject>, kube::Error> {
    let gvk = kube::core::GroupVersionKind::gvk("traefik.containo.us", "v1alpha1", "IngressRoute");
    let (api_resource, _) = kube::discovery::pinned_kind(client, &gvk).await?;
    let api: Api<DynamicObject> = if let Some(ns) = namespace {
        Api::namespaced_with(client.clone(), ns, &api_resource)
    } else {
        Api::all_with(client.clone(), &api_resource)
    };
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}
