//! ViewModels for Kubernetes resources.

pub mod config_maps;
pub mod daemon_sets;
pub mod deployments;
pub mod detail;
pub mod ingresses;
pub mod jobs;
pub mod node;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pod;
pub mod secrets;
pub mod services;

pub use config_maps::ConfigMapsViewModel;
pub use daemon_sets::DaemonSetsViewModel;
pub use deployments::DeploymentsViewModel;
pub use ingresses::IngressesViewModel;
pub use jobs::JobsViewModel;
pub use node::NodesViewModel;
pub use persistent_volume_claims::PersistentVolumeClaimsViewModel;
pub use persistent_volumes::PersistentVolumesViewModel;
pub use pod::PodsViewModel;
pub use secrets::SecretsViewModel;
pub use services::ServicesViewModel;
