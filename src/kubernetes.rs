use kube::{api::{Api, DynamicObject, ListParams, WatchParams, WatchEvent}, Client, Resource};
use k8s_openapi::api::core::v1::{Event, Node, Pod, Secret, Service};
use k8s_openapi::api::apps::v1::Deployment;
use futures::{StreamExt, SinkExt};
use iced;
use tokio;

#[derive(Debug, Clone)]
pub enum KubeWatchEvent<T> {
    Added(T),
    Modified(T),
    Deleted(T),
}

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

pub fn watch_namespaced_resource<K, M, F>(
    id: impl std::hash::Hash + 'static + Send + Clone,
    client: std::sync::Arc<Client>,
    namespace: Option<String>,
    on_event: F
) -> iced::Subscription<M>
where
    K: Resource<Scope = kube::core::NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: Default + std::cmp::Eq + std::hash::Hash + Clone,
    M: 'static + Send + Clone,
    F: Fn(KubeWatchEvent<K>) -> M + Send + Sync + 'static + Clone,
{
    iced::Subscription::run_with_id(
        id,
        iced::stream::channel(100, move |mut output| async move {
            let api: Api<K> = match namespace {
                Some(ref ns) => Api::namespaced((*client).clone(), ns),
                None => Api::all((*client).clone()),
            };
            let wp = WatchParams::default();
            loop {
                match api.watch(&wp, "0").await {
                    Ok(stream) => {
                        let mut pinned_stream = std::pin::pin!(stream);
                        while let Some(event) = pinned_stream.next().await {
                            match event {
                                Ok(WatchEvent::Added(item)) => { let _ = output.send(on_event(KubeWatchEvent::Added(item))).await; }
                                Ok(WatchEvent::Modified(item)) => { let _ = output.send(on_event(KubeWatchEvent::Modified(item))).await; }
                                Ok(WatchEvent::Deleted(item)) => { let _ = output.send(on_event(KubeWatchEvent::Deleted(item))).await; }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => { tokio::time::sleep(std::time::Duration::from_secs(2)).await; }
                }
            }
        })
    )
}

pub fn watch_cluster_resource<K, M, F>(
    id: impl std::hash::Hash + 'static + Send + Clone,
    client: std::sync::Arc<Client>,
    on_event: F
) -> iced::Subscription<M>
where
    K: Resource<Scope = kube::core::ClusterResourceScope> + Clone + serde::de::DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: Default + std::cmp::Eq + std::hash::Hash + Clone,
    M: 'static + Send + Clone,
    F: Fn(KubeWatchEvent<K>) -> M + Send + Sync + 'static + Clone,
{
    iced::Subscription::run_with_id(
        id,
        iced::stream::channel(100, move |mut output| async move {
            let api: Api<K> = Api::all((*client).clone());
            let wp = WatchParams::default();
            loop {
                match api.watch(&wp, "0").await {
                    Ok(stream) => {
                        let mut pinned_stream = std::pin::pin!(stream);
                        while let Some(event) = pinned_stream.next().await {
                            match event {
                                Ok(WatchEvent::Added(item)) => { let _ = output.send(on_event(KubeWatchEvent::Added(item))).await; }
                                Ok(WatchEvent::Modified(item)) => { let _ = output.send(on_event(KubeWatchEvent::Modified(item))).await; }
                                Ok(WatchEvent::Deleted(item)) => { let _ = output.send(on_event(KubeWatchEvent::Deleted(item))).await; }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => { tokio::time::sleep(std::time::Duration::from_secs(2)).await; }
                }
            }
        })
    )
}
