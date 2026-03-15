use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceModel {
    pub name: String,
    pub namespace: String,
    pub age: String,
    pub status: String,
    pub raw: String,
}

impl From<Service> for ServiceModel {
    fn from(item: Service) -> Self {
        let name = item.name_any();
        let namespace = item.namespace().unwrap_or_else(|| "default".to_string());
        let age = item.creation_timestamp()
            .map(|t| t.0.to_string())
            .unwrap_or_default();
        let status = item.spec.as_ref().and_then(|spec| spec.type_.clone()).unwrap_or_else(|| "ClusterIP".to_string());
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
