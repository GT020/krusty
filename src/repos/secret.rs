use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client, api::{ListParams, DeleteParams}};

pub struct SecretRepo;

impl SecretRepo {
    pub async fn list(client: &Client, namespace: Option<&str>) -> Result<Vec<Secret>, kube::Error> {
        let api: Api<Secret> = if let Some(ns) = namespace {
            Api::namespaced(client.clone(), ns)
        } else {
            Api::all(client.clone())
        };
        let list = api.list(&ListParams::default()).await?;
        Ok(list.items)
    }

    pub async fn delete(client: &Client, name: &str, namespace: &str) -> Result<(), kube::Error> {
        let ns = if namespace.is_empty() { "default" } else { namespace };
        let api: Api<Secret> = Api::namespaced(client.clone(), ns);
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }
}
