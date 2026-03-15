use k8s_openapi::api::core::v1::Service;
use kube::{Api, Client, api::{ListParams, DeleteParams}};

pub struct ServiceRepo;

impl ServiceRepo {
    pub async fn list(client: &Client, namespace: Option<&str>) -> Result<Vec<Service>, kube::Error> {
        let api: Api<Service> = if let Some(ns) = namespace {
            Api::namespaced(client.clone(), ns)
        } else {
            Api::all(client.clone())
        };
        let list = api.list(&ListParams::default()).await?;
        Ok(list.items)
    }

    pub async fn delete(client: &Client, name: &str, namespace: &str) -> Result<(), kube::Error> {
        let ns = if namespace.is_empty() { "default" } else { namespace };
        let api: Api<Service> = Api::namespaced(client.clone(), ns);
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }
}
