//! Repository streams for Kubernetes resources.

pub mod config_maps;
pub mod daemon_sets;
pub mod deployments;
pub mod detail;
pub mod ingresses;
pub mod jobs;
pub mod nodes;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pods;
pub mod secrets;
pub mod services;
pub mod watcher;

pub use watcher::namespace_names;
