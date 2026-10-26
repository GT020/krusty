//! Generic detail state for resource kinds that do not have a dedicated view.

use serde_json::Value;

use crate::models::ResourceKind;

#[derive(Clone, Debug)]
pub struct ResourceDetail {
    pub kind: ResourceKind,
    pub namespace: String,
    pub name: String,
    pub value: Value,
    pub raw_json: String,
}

#[derive(Clone, Debug, Default)]
pub enum ResourceDetailState {
    #[default]
    Closed,
    Loading {
        kind: ResourceKind,
        namespace: String,
        name: String,
    },
    Ready(Box<ResourceDetail>),
    Error {
        kind: ResourceKind,
        namespace: String,
        name: String,
        message: String,
    },
}

impl ResourceDetailState {
    pub fn identity(&self) -> Option<(ResourceKind, &str, &str)> {
        match self {
            Self::Closed => None,
            Self::Loading {
                kind,
                namespace,
                name,
            } => Some((*kind, namespace, name)),
            Self::Ready(detail) => Some((detail.kind, &detail.namespace, &detail.name)),
            Self::Error {
                kind,
                namespace,
                name,
                ..
            } => Some((*kind, namespace, name)),
        }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }
}

#[derive(Clone, Debug)]
pub struct ResourceDetailUpdate {
    pub kind: ResourceKind,
    pub namespace: String,
    pub name: String,
    pub result: Result<Box<ResourceDetail>, String>,
}
