use futures::{Stream, StreamExt, TryStreamExt};
use kube::Client;
use kube::Api;
use kube::api::{ResourceExt, WatchEvent};
use kube::config::{KubeConfigOptions, Kubeconfig};
use k8s_openapi::api::core::v1::{Pod, Namespace, Service, ConfigMap, PersistentVolumeClaim, ServiceAccount, Endpoints};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, DaemonSet};
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::batch::v1::{Job, CronJob};
use std::collections::BTreeMap;
use std::pin::Pin;

#[derive(Debug, Clone, PartialEq)]
pub struct K8sResource {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub api_version: String,
    pub uid: Option<String>,
    pub creation_timestamp: Option<String>,
    pub labels: BTreeMap<String, String>,
}

impl From<Pod> for K8sResource {
    fn from(pod: Pod) -> Self {
        Self {
            name: pod.name_unchecked(),
            namespace: pod.namespace(),
            kind: "Pod".to_string(),
            api_version: "v1".to_string(),
            uid: pod.metadata.uid.clone(),
            creation_timestamp: pod.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: pod.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Namespace> for K8sResource {
    fn from(ns: Namespace) -> Self {
        Self {
            name: ns.name_unchecked(),
            namespace: None,
            kind: "Namespace".to_string(),
            api_version: "v1".to_string(),
            uid: ns.metadata.uid.clone(),
            creation_timestamp: ns.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: ns.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Deployment> for K8sResource {
    fn from(dep: Deployment) -> Self {
        Self {
            name: dep.name_unchecked(),
            namespace: dep.namespace(),
            kind: "Deployment".to_string(),
            api_version: "apps/v1".to_string(),
            uid: dep.metadata.uid.clone(),
            creation_timestamp: dep.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: dep.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Service> for K8sResource {
    fn from(svc: Service) -> Self {
        Self {
            name: svc.name_unchecked(),
            namespace: svc.namespace(),
            kind: "Service".to_string(),
            api_version: "v1".to_string(),
            uid: svc.metadata.uid.clone(),
            creation_timestamp: svc.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: svc.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<ConfigMap> for K8sResource {
    fn from(cm: ConfigMap) -> Self {
        Self {
            name: cm.name_unchecked(),
            namespace: cm.namespace(),
            kind: "ConfigMap".to_string(),
            api_version: "v1".to_string(),
            uid: cm.metadata.uid.clone(),
            creation_timestamp: cm.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: cm.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Secret> for K8sResource {
    fn from(sec: Secret) -> Self {
        Self {
            name: sec.name_unchecked(),
            namespace: sec.namespace(),
            kind: "Secret".to_string(),
            api_version: "v1".to_string(),
            uid: sec.metadata.uid.clone(),
            creation_timestamp: sec.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: sec.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<PersistentVolumeClaim> for K8sResource {
    fn from(pvc: PersistentVolumeClaim) -> Self {
        Self {
            name: pvc.name_unchecked(),
            namespace: pvc.namespace(),
            kind: "PersistentVolumeClaim".to_string(),
            api_version: "v1".to_string(),
            uid: pvc.metadata.uid.clone(),
            creation_timestamp: pvc.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: pvc.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<StatefulSet> for K8sResource {
    fn from(ss: StatefulSet) -> Self {
        Self {
            name: ss.name_unchecked(),
            namespace: ss.namespace(),
            kind: "StatefulSet".to_string(),
            api_version: "apps/v1".to_string(),
            uid: ss.metadata.uid.clone(),
            creation_timestamp: ss.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: ss.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<DaemonSet> for K8sResource {
    fn from(ds: DaemonSet) -> Self {
        Self {
            name: ds.name_unchecked(),
            namespace: ds.namespace(),
            kind: "DaemonSet".to_string(),
            api_version: "apps/v1".to_string(),
            uid: ds.metadata.uid.clone(),
            creation_timestamp: ds.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: ds.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Ingress> for K8sResource {
    fn from(ing: Ingress) -> Self {
        Self {
            name: ing.name_unchecked(),
            namespace: ing.namespace(),
            kind: "Ingress".to_string(),
            api_version: "networking.k8s.io/v1".to_string(),
            uid: ing.metadata.uid.clone(),
            creation_timestamp: ing.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: ing.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Job> for K8sResource {
    fn from(job: Job) -> Self {
        Self {
            name: job.name_unchecked(),
            namespace: job.namespace(),
            kind: "Job".to_string(),
            api_version: "batch/v1".to_string(),
            uid: job.metadata.uid.clone(),
            creation_timestamp: job.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: job.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<CronJob> for K8sResource {
    fn from(cj: CronJob) -> Self {
        Self {
            name: cj.name_unchecked(),
            namespace: cj.namespace(),
            kind: "CronJob".to_string(),
            api_version: "batch/v1".to_string(),
            uid: cj.metadata.uid.clone(),
            creation_timestamp: cj.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: cj.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<ServiceAccount> for K8sResource {
    fn from(sa: ServiceAccount) -> Self {
        Self {
            name: sa.name_unchecked(),
            namespace: sa.namespace(),
            kind: "ServiceAccount".to_string(),
            api_version: "v1".to_string(),
            uid: sa.metadata.uid.clone(),
            creation_timestamp: sa.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: sa.metadata.labels.unwrap_or_default(),
        }
    }
}

impl From<Endpoints> for K8sResource {
    fn from(ep: Endpoints) -> Self {
        Self {
            name: ep.name_unchecked(),
            namespace: ep.namespace(),
            kind: "Endpoints".to_string(),
            api_version: "v1".to_string(),
            uid: ep.metadata.uid.clone(),
            creation_timestamp: ep.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()),
            labels: ep.metadata.labels.unwrap_or_default(),
        }
    }
}

use crate::k8s::context::set_connected;

pub async fn create_client() -> Result<Client, kube::Error> {
    let context = crate::k8s::context::get_current_context();
    let options = KubeConfigOptions {
        context: if context.is_empty() { None } else { Some(context) },
        ..Default::default()
    };
    match kube::Config::from_kubeconfig(&options).await {
        Ok(config) => Client::try_from(config),
        Err(e) => {
            tracing::error!("Failed to load kubeconfig: {}", e);
            match Client::try_default().await {
                Ok(client) => {
                    set_connected(true);
                    Ok(client)
                }
                Err(e) => {
                    tracing::error!("Failed to connect to cluster: {}", e);
                    set_connected(false);
                    Err(e)
                }
            }
        }
    }
}

pub fn list_contexts() -> Vec<String> {
    match Kubeconfig::read() {
        Ok(config) => config.contexts.iter().map(|c| c.name.clone()).collect(),
        Err(_) => vec!["default".to_string()],
    }
}

pub async fn list_pods(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Pod> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let pods = api.list(&Default::default()).await?;
    Ok(pods.into_iter().map(|p| p.into()).collect())
}

pub async fn list_namespaces() -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Namespace> = Api::all(client);
    let nss = api.list(&Default::default()).await?;
    Ok(nss.into_iter().map(|n| n.into()).collect())
}

pub async fn list_deployments(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Deployment> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let deps = api.list(&Default::default()).await?;
    Ok(deps.into_iter().map(|d| d.into()).collect())
}

pub async fn list_services(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Service> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let svcs = api.list(&Default::default()).await?;
    Ok(svcs.into_iter().map(|s| s.into()).collect())
}

pub async fn list_configmaps(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<ConfigMap> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let cms = api.list(&Default::default()).await?;
    Ok(cms.into_iter().map(|c| c.into()).collect())
}

pub async fn list_secrets(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Secret> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let secs = api.list(&Default::default()).await?;
    Ok(secs.into_iter().map(|s| s.into()).collect())
}

pub async fn list_persistentvolumeclaims(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<PersistentVolumeClaim> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let pvcs = api.list(&Default::default()).await?;
    Ok(pvcs.into_iter().map(|p| p.into()).collect())
}

pub async fn list_statefulsets(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<StatefulSet> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let sss = api.list(&Default::default()).await?;
    Ok(sss.into_iter().map(|s| s.into()).collect())
}

pub async fn list_daemonsets(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<DaemonSet> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let dss = api.list(&Default::default()).await?;
    Ok(dss.into_iter().map(|d| d.into()).collect())
}

pub async fn list_ingresses(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Ingress> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let ings = api.list(&Default::default()).await?;
    Ok(ings.into_iter().map(|i| i.into()).collect())
}

pub async fn list_jobs(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Job> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let jobs = api.list(&Default::default()).await?;
    Ok(jobs.into_iter().map(|j| j.into()).collect())
}

pub async fn list_cronjobs(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<CronJob> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let cjs = api.list(&Default::default()).await?;
    Ok(cjs.into_iter().map(|c| c.into()).collect())
}

pub async fn list_serviceaccounts(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<ServiceAccount> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let sas = api.list(&Default::default()).await?;
    Ok(sas.into_iter().map(|s| s.into()).collect())
}

pub async fn list_endpoints(ns: Option<&str>) -> Result<Vec<K8sResource>, Box<dyn std::error::Error>> {
    let client = create_client().await?;
    let api: Api<Endpoints> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    let eps = api.list(&Default::default()).await?;
    Ok(eps.into_iter().map(|e| e.into()).collect())
}

pub async fn watch_pods(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Pod>> + Send>> {
    let client = match create_client().await {
        Ok(c) => c,
        Err(_) => return futures::stream::iter(std::iter::empty()).boxed(),
    };
    let api: Api<Pod> = match ns {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    };
    match api.watch(&Default::default(), "").await {
        Ok(stream) => Box::pin(stream.filter_map(|r| async move { r.ok() })),
        Err(e) => {
            tracing::error!("Watch failed: {}", e);
            futures::stream::iter(std::iter::empty()).boxed()
        }
    }
}

pub async fn watch_namespaces() -> Pin<Box<dyn Stream<Item = WatchEvent<Namespace>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Namespace> = Api::all(client);
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(e) => {
                    tracing::error!("Watch failed: {}", e);
                    set_connected(false);
                    futures::stream::iter(std::iter::empty()).boxed()
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to create client: {}", e);
            set_connected(false);
            futures::stream::iter(std::iter::empty()).boxed()
        }
    }
}

pub async fn watch_deployments(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Deployment>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Deployment> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_services(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Service>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Service> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_configmaps(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<ConfigMap>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<ConfigMap> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_secrets(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Secret>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Secret> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_persistentvolumeclaims(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<PersistentVolumeClaim>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<PersistentVolumeClaim> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_statefulsets(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<StatefulSet>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<StatefulSet> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_daemonsets(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<DaemonSet>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<DaemonSet> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_ingresses(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Ingress>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Ingress> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_jobs(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Job>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Job> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_cronjobs(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<CronJob>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<CronJob> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_serviceaccounts(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<ServiceAccount>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<ServiceAccount> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}

pub async fn watch_endpoints(ns: Option<&str>) -> Pin<Box<dyn Stream<Item = WatchEvent<Endpoints>> + Send>> {
    match create_client().await {
        Ok(client) => {
            let api: Api<Endpoints> = match ns {
                Some(namespace) => Api::namespaced(client, namespace),
                None => Api::all(client),
            };
            match api.watch(&Default::default(), "").await {
                Ok(stream) => stream
                    .into_stream()
                    .filter_map(|r| async move { r.ok() })
                    .boxed(),
                Err(_) => futures::stream::iter(std::iter::empty()).boxed()
            }
        }
        Err(_) => futures::stream::iter(std::iter::empty()).boxed()
    }
}