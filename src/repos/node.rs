use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client, api::{ListParams, DeleteParams}};

pub struct NodeRepo;

impl NodeRepo {
    pub async fn list(client: &Client) -> Result<Vec<Node>, kube::Error> {
        let api: Api<Node> = Api::all(client.clone());
        let list = api.list(&ListParams::default()).await?;
        Ok(list.items)
    }

    pub async fn delete(client: &Client, name: &str) -> Result<(), kube::Error> {
        let api: Api<Node> = Api::all(client.clone());
        api.delete(name, &DeleteParams::default()).await?;
        Ok(())
    }
}
