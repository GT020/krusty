pub mod route;
pub use route::Route;

pub mod home;
pub mod sidebar;
pub mod resource_list;

pub mod pods;
pub mod deployments;
pub mod services;
pub mod configmaps;
pub mod secrets;
pub mod namespaces;
pub mod statefulsets;
pub mod daemonsets;
pub mod jobs;
pub mod cronjobs;
pub mod ingresses;
pub mod persistentvolumeclaims;
pub mod serviceaccounts;
pub mod endpoints;
