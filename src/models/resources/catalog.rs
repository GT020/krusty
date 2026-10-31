use std::sync::{Mutex, OnceLock};

type Icon = gpui_kit::assets::IconName;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ResourceKind {
    Pods,
    Deployments,
    StatefulSets,
    ReplicaSets,
    ReplicationControllers,
    DaemonSets,
    Jobs,
    CronJobs,
    HorizontalPodAutoscalers,
    PodDisruptionBudgets,
    Services,
    Ingresses,
    Endpoints,
    EndpointSlices,
    NetworkPolicies,
    PersistentVolumes,
    PersistentVolumeClaims,
    StorageClasses,
    ConfigMaps,
    Secrets,
    ServiceAccounts,
    Namespaces,
    Nodes,
    Leases,
    Events,
    ClusterRoles,
    Roles,
    RoleBindings,
    Custom(u16),
}

/// Every built-in kind, in the order used by persisted UI state.
const BUILTIN_KINDS: [ResourceKind; 28] = [
    ResourceKind::Pods,
    ResourceKind::Deployments,
    ResourceKind::StatefulSets,
    ResourceKind::ReplicaSets,
    ResourceKind::ReplicationControllers,
    ResourceKind::DaemonSets,
    ResourceKind::Jobs,
    ResourceKind::CronJobs,
    ResourceKind::HorizontalPodAutoscalers,
    ResourceKind::PodDisruptionBudgets,
    ResourceKind::Services,
    ResourceKind::Ingresses,
    ResourceKind::Endpoints,
    ResourceKind::EndpointSlices,
    ResourceKind::NetworkPolicies,
    ResourceKind::PersistentVolumes,
    ResourceKind::PersistentVolumeClaims,
    ResourceKind::StorageClasses,
    ResourceKind::ConfigMaps,
    ResourceKind::Secrets,
    ResourceKind::ServiceAccounts,
    ResourceKind::Namespaces,
    ResourceKind::Nodes,
    ResourceKind::Leases,
    ResourceKind::Events,
    ResourceKind::ClusterRoles,
    ResourceKind::Roles,
    ResourceKind::RoleBindings,
];

impl ResourceKind {
    pub fn label(self) -> &'static str {
        match self {
            ResourceKind::Pods => "Pods",
            ResourceKind::Deployments => "Deployments",
            ResourceKind::StatefulSets => "StatefulSets",
            ResourceKind::ReplicaSets => "ReplicaSets",
            ResourceKind::ReplicationControllers => "ReplicationControllers",
            ResourceKind::DaemonSets => "DaemonSets",
            ResourceKind::Jobs => "Jobs",
            ResourceKind::CronJobs => "CronJobs",
            ResourceKind::HorizontalPodAutoscalers => "HorizontalPodAutoscalers",
            ResourceKind::PodDisruptionBudgets => "PodDisruptionBudgets",
            ResourceKind::Services => "Services",
            ResourceKind::Ingresses => "Ingresses",
            ResourceKind::Endpoints => "Endpoints",
            ResourceKind::EndpointSlices => "EndpointSlices",
            ResourceKind::NetworkPolicies => "NetworkPolicies",
            ResourceKind::PersistentVolumes => "PersistentVolumes",
            ResourceKind::PersistentVolumeClaims => "PersistentVolumeClaims",
            ResourceKind::StorageClasses => "StorageClasses",
            ResourceKind::ConfigMaps => "ConfigMaps",
            ResourceKind::Secrets => "Secrets",
            ResourceKind::ServiceAccounts => "ServiceAccounts",
            ResourceKind::Namespaces => "Namespaces",
            ResourceKind::Nodes => "Nodes",
            ResourceKind::Leases => "Leases",
            ResourceKind::Events => "Events",
            ResourceKind::ClusterRoles => "ClusterRoles",
            ResourceKind::Roles => "Roles",
            ResourceKind::RoleBindings => "RoleBindings",
            ResourceKind::Custom(index) => custom_def(index)
                .map(|def| def.kind.as_str())
                .unwrap_or("Custom"),
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            ResourceKind::Pods => Icon::Boxes,
            ResourceKind::Deployments => Icon::Rocket,
            ResourceKind::StatefulSets => Icon::Database,
            ResourceKind::ReplicaSets => Icon::Copy,
            ResourceKind::ReplicationControllers => Icon::Copy,
            ResourceKind::DaemonSets => Icon::Layers,
            ResourceKind::Jobs => Icon::Loader,
            ResourceKind::CronJobs => Icon::CalendarClock,
            ResourceKind::HorizontalPodAutoscalers => Icon::Gauge,
            ResourceKind::PodDisruptionBudgets => Icon::ShieldCheck,
            ResourceKind::Services => Icon::Globe,
            ResourceKind::Ingresses => Icon::Network,
            ResourceKind::Endpoints => Icon::Share2,
            ResourceKind::EndpointSlices => Icon::Waypoints,
            ResourceKind::NetworkPolicies => Icon::Shield,
            ResourceKind::PersistentVolumes | ResourceKind::PersistentVolumeClaims => {
                Icon::HardDrive
            }
            ResourceKind::StorageClasses => Icon::Database,
            ResourceKind::ConfigMaps => Icon::FileText,
            ResourceKind::Secrets => Icon::KeyRound,
            ResourceKind::ServiceAccounts => Icon::User,
            ResourceKind::Namespaces => Icon::Folder,
            ResourceKind::Nodes => Icon::Cpu,
            ResourceKind::Leases => Icon::Clock,
            ResourceKind::Events => Icon::Activity,
            ResourceKind::ClusterRoles => Icon::ShieldCheck,
            ResourceKind::Roles => Icon::Shield,
            ResourceKind::RoleBindings => Icon::Link,
            ResourceKind::Custom(_) => Icon::Puzzle,
        }
    }

    pub fn nav_groups() -> Vec<(&'static str, Vec<ResourceKind>)> {
        let mut groups = vec![
            (
                "Cluster",
                vec![
                    ResourceKind::Nodes,
                    ResourceKind::Namespaces,
                    ResourceKind::Events,
                    ResourceKind::Leases,
                ],
            ),
            (
                "Workloads",
                vec![
                    ResourceKind::Pods,
                    ResourceKind::Deployments,
                    ResourceKind::StatefulSets,
                    ResourceKind::ReplicaSets,
                    ResourceKind::ReplicationControllers,
                    ResourceKind::DaemonSets,
                    ResourceKind::Jobs,
                    ResourceKind::CronJobs,
                    ResourceKind::HorizontalPodAutoscalers,
                    ResourceKind::PodDisruptionBudgets,
                ],
            ),
            (
                "Network",
                vec![
                    ResourceKind::Services,
                    ResourceKind::Ingresses,
                    ResourceKind::Endpoints,
                    ResourceKind::EndpointSlices,
                    ResourceKind::NetworkPolicies,
                ],
            ),
            (
                "Storage",
                vec![
                    ResourceKind::PersistentVolumes,
                    ResourceKind::PersistentVolumeClaims,
                    ResourceKind::StorageClasses,
                ],
            ),
            (
                "Config",
                vec![
                    ResourceKind::ConfigMaps,
                    ResourceKind::Secrets,
                    ResourceKind::ServiceAccounts,
                ],
            ),
            (
                "Access",
                vec![
                    ResourceKind::ClusterRoles,
                    ResourceKind::Roles,
                    ResourceKind::RoleBindings,
                ],
            ),
        ];
        let custom = custom_resource_entries();
        if !custom.is_empty() {
            groups.push((
                "Custom",
                custom
                    .into_iter()
                    .map(|(index, _)| ResourceKind::Custom(index))
                    .collect(),
            ));
        }
        groups
    }

    /// Cluster-scoped kinds ignore the namespace selector.
    pub fn is_namespaced(self) -> bool {
        match self {
            ResourceKind::Nodes
            | ResourceKind::Namespaces
            | ResourceKind::PersistentVolumes
            | ResourceKind::StorageClasses
            | ResourceKind::ClusterRoles => false,
            ResourceKind::Custom(index) => custom_def(index).is_some_and(|def| def.namespaced),
            _ => true,
        }
    }

    /// Stable identifier used to persist UI state. Custom kinds return `None`
    /// because their index is process-local; they are persisted by definition
    /// and resolved back to an index on the next launch.
    pub fn stable_id(self) -> Option<&'static str> {
        match self {
            ResourceKind::Custom(_) => None,
            _ => Some(self.label()),
        }
    }

    /// Resolve an identifier written by [`Self::stable_id`].
    pub fn from_stable_id(id: &str) -> Option<Self> {
        BUILTIN_KINDS
            .iter()
            .copied()
            .find(|kind| kind.label() == id)
    }
}

/// A user-added custom resource kind discovered from a CRD or entered manually.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CustomResourceDef {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
}

/// Deleted definitions leave an empty slot so existing `ResourceKind::Custom`
/// indices remain stable for the lifetime of the process.
static CUSTOM_RESOURCES: OnceLock<Mutex<Vec<Option<&'static CustomResourceDef>>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Option<&'static CustomResourceDef>>> {
    CUSTOM_RESOURCES.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn custom_resources() -> Vec<&'static CustomResourceDef> {
    registry()
        .lock()
        .map(|defs| defs.iter().copied().flatten().collect())
        .unwrap_or_default()
}

/// Definitions paired with their stable index, excluding deleted entries.
pub fn custom_resource_entries() -> Vec<(u16, &'static CustomResourceDef)> {
    registry()
        .lock()
        .map(|defs| {
            defs.iter()
                .enumerate()
                .filter_map(|(index, def)| def.map(|def| (index as u16, def)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn custom_def(index: u16) -> Option<&'static CustomResourceDef> {
    registry()
        .lock()
        .ok()
        .and_then(|defs| defs.get(index as usize).copied().flatten())
}

/// Replace a previously registered custom resource definition in place while
/// keeping its `ResourceKind::Custom` index stable.
pub fn update_custom_resource(index: u16, def: CustomResourceDef) -> Option<ResourceKind> {
    let mut defs = registry().lock().ok()?;
    defs.get(index as usize).and_then(|def| *def)?;
    let def: &'static CustomResourceDef = Box::leak(Box::new(def));
    defs[index as usize] = Some(def);
    Some(ResourceKind::Custom(index))
}

/// Remove a definition while keeping its index reserved.
pub fn delete_custom_resource(index: u16) -> bool {
    let Ok(mut defs) = registry().lock() else {
        return false;
    };
    match defs.get_mut(index as usize) {
        Some(slot @ Some(_)) => {
            *slot = None;
            true
        }
        _ => false,
    }
}

/// Register a custom resource definition. Definitions are kept in insertion
/// order for the lifetime of the process, so `ResourceKind::Custom` indices stay
/// stable. Returns the existing kind when the same resource is added twice.
pub fn add_custom_resource(def: CustomResourceDef) -> ResourceKind {
    let Ok(mut defs) = registry().lock() else {
        return ResourceKind::Custom(0);
    };
    if let Some(index) = defs.iter().position(|existing| {
        existing.is_some_and(|existing| {
            existing.group == def.group
                && existing.version == def.version
                && existing.plural == def.plural
        })
    }) {
        return ResourceKind::Custom(index as u16);
    }
    let def: &'static CustomResourceDef = Box::leak(Box::new(def));
    defs.push(Some(def));
    ResourceKind::Custom((defs.len() - 1) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_stable_ids_round_trip() {
        for kind in BUILTIN_KINDS {
            let id = kind.stable_id().expect("built-in kind has a stable id");
            assert_eq!(ResourceKind::from_stable_id(id), Some(kind));
        }
    }

    #[test]
    fn custom_kinds_have_no_stable_id() {
        assert_eq!(ResourceKind::Custom(0).stable_id(), None);
        assert_eq!(ResourceKind::from_stable_id("Custom"), None);
        assert_eq!(ResourceKind::from_stable_id("NotAKind"), None);
    }
}
