//! Domain layer: plain structs + pure state transitions. No gpui types.

pub mod cluster;
pub mod kubectl;
pub mod resources;

pub use cluster::NamespaceScope;
pub use resources::{
    Item, ObjectKey, ResourceFilter, ResourceKind, ResourceSnapshot, SnapshotPhase, WatchMode,
};
