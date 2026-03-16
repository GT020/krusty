use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, api::{ListParams, DeleteParams, LogParams}};

pub struct PodRepo;

impl PodRepo {
    pub async fn list(client: &Client, namespace: Option<&str>) -> Result<Vec<Pod>, kube::Error> {
        let api: Api<Pod> = if let Some(ns) = namespace {
            Api::namespaced(client.clone(), ns)
        } else {
            Api::all(client.clone())
        };
        let list = api.list(&ListParams::default()).await?;
        Ok(list.items)
    }

    pub async fn delete(client: &Client, name: &str, namespace: &str) -> Result<(), kube::Error> {
        let ns = if namespace.is_empty() { "default" } else { namespace };
        let api: Api<Pod> = Api::namespaced(client.clone(), ns);
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }

    pub async fn logs(client: &Client, name: &str, namespace: &str, container: Option<&str>) -> Result<String, kube::Error> {
        let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let log_params = LogParams {
            container: container.map(|s| s.to_string()),
            tail_lines: Some(500),
            ..Default::default()
        };
        let logs = api.logs(name, &log_params).await?;
        Ok(logs)
    }
}
