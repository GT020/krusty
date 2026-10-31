#![allow(unused_imports)]
//! Domain layer: plain structs + pure state transitions. No gpui types.

pub mod cluster;
pub mod kubectl;
pub mod resources;

pub use cluster::NamespaceScope;
pub use resources::{
    ConfigMapsModel, DaemonSetsModel, DeploymentsModel, IngressesModel, Item, JobsModel,
    NodesModel, ObjectKey, PersistentVolumeClaimsModel, PersistentVolumesModel, PodsModel,
    PollRate, ResourceDetail, ResourceDetailState, ResourceDetailUpdate, ResourceKind,
    ResourceSnapshot, ResourceStore, SecretsModel, ServicesModel, SnapshotPhase, WatchMode,
};
