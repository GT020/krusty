use k8s_openapi::api::apps::v1::Deployment;
use kube::{Api, Client, api::{ListParams, DeleteParams}};

pub struct DeploymentRepo;

impl DeploymentRepo {
    pub async fn list(client: &Client, namespace: Option<&str>) -> Result<Vec<Deployment>, kube::Error> {
        let api: Api<Deployment> = if let Some(ns) = namespace {
            Api::namespaced(client.clone(), ns)
        } else {
            Api::all(client.clone())
        };
        let list = api.list(&ListParams::default()).await?;
        Ok(list.items)
    }

    pub async fn delete(client: &Client, name: &str, namespace: &str) -> Result<(), kube::Error> {
        let ns = if namespace.is_empty() { "default" } else { namespace };
        let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }
}
