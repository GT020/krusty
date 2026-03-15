use k8s_openapi::api::core::v1::Pod;
use kube::ResourceExt;

#[derive(Clone, Debug, PartialEq)]
pub struct PodModel {
    pub name: String,
    pub namespace: String,
    pub age: String,
    pub status: String,
    pub raw: String,
}

impl From<Pod> for PodModel {
    fn from(item: Pod) -> Self {
        let name = item.name_any();
        let namespace = item.namespace().unwrap_or_else(|| "default".to_string());
        let age = item.creation_timestamp()
            .map(|t| t.0.to_string())
            .unwrap_or_default();
        let status = item.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_else(|| "Unknown".to_string());
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
