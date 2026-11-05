//! Full-object cache records.
//!
//! The typed Kubernetes object is retained behind a type-erased trait so one
//! registry can hold every built-in kind and custom resource. Details downcast
//! to the concrete kind; list tables use the precomputed `Item` summary.

use std::any::Any;
use std::sync::Arc;

use kube::ResourceExt;

use crate::models::{Item, ObjectKey};

/// A Kubernetes object stored without knowing its concrete Rust type.
pub trait StoredObject: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn to_json(&self) -> serde_json::Value;
}

impl<T> StoredObject for T
where
    T: Any + Send + Sync + serde::Serialize + ResourceExt,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// One object in the shared store.
pub struct KubeObjectRecord {
    pub key: ObjectKey,
    pub uid: String,
    pub resource_version: String,
    pub summary: Arc<Item>,
    raw: Arc<dyn StoredObject>,
}

impl KubeObjectRecord {
    pub fn new<K>(object: K, summary: Item) -> Self
    where
        K: StoredObject + ResourceExt,
    {
        let key = ObjectKey::new(object.namespace().unwrap_or_default(), object.name_any());
        let uid = object.uid().unwrap_or_default();
        let resource_version = object.resource_version().unwrap_or_default();
        Self {
            key,
            uid,
            resource_version,
            summary: Arc::new(summary),
            raw: Arc::new(object),
        }
    }

    pub fn typed<T: Any>(&self) -> Option<&T> {
        self.raw.as_any().downcast_ref::<T>()
    }

    pub fn to_json(&self) -> serde_json::Value {
        self.raw.to_json()
    }
}

impl std::fmt::Debug for KubeObjectRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KubeObjectRecord")
            .field("key", &self.key)
            .field("uid", &self.uid)
            .field("resource_version", &self.resource_version)
            .finish_non_exhaustive()
    }
}
