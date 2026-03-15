use k8s_openapi::api::apps::v1::Deployment;
use kube::ResourceExt;

#[derive(Clone, Debug, PartialEq)]
pub struct DeploymentModel {
    pub name: String,
    pub namespace: String,
    pub age: String,
    pub status: String,
    pub raw: String,
}

impl From<Deployment> for DeploymentModel {
    fn from(item: Deployment) -> Self {
        let name = item.name_any();
        let namespace = item.namespace().unwrap_or_else(|| "default".to_string());
        let age = item.creation_timestamp()
            .map(|t| t.0.to_string())
            .unwrap_or_default();
        let status = "-".to_string();
        let raw = serde_json::to_string_pretty(&item).unwrap_or_default();

        Self {
            name,
            namespace,
            age,
            status,
            raw,
        }
    }
}
