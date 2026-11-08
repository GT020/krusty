//! ViewModels for Kubernetes resources.

pub mod deployments;
pub mod detail;
pub mod generic;
pub mod node;
pub mod overview;
pub mod pod;
pub mod table;

pub use generic::ResourceListViewModel;
pub use node::NodesViewModel;
pub use overview::ClusterOverviewViewModel;
pub use pod::PodsViewModel;
