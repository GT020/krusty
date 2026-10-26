use gpui_kit::component::sidebar::SidebarMenuItem;

type Icon = gpui_kit::assets::IconName;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ResourceKind {
    Pods,
    Deployments,
    DaemonSets,
    Jobs,
    Services,
    Ingresses,
    PersistentVolumes,
    PersistentVolumeClaims,
    ConfigMaps,
    Secrets,
    Nodes,
}

impl ResourceKind {
    pub fn label(self) -> &'static str {
        match self {
            ResourceKind::Pods => "Pods",
            ResourceKind::Deployments => "Deployments",
            ResourceKind::DaemonSets => "DaemonSets",
            ResourceKind::Jobs => "Jobs",
            ResourceKind::Services => "Services",
            ResourceKind::Ingresses => "Ingresses",
            ResourceKind::PersistentVolumes => "PersistentVolumes",
            ResourceKind::PersistentVolumeClaims => "PersistentVolumeClaims",
            ResourceKind::ConfigMaps => "ConfigMaps",
            ResourceKind::Secrets => "Secrets",
            ResourceKind::Nodes => "Nodes",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            ResourceKind::Pods => Icon::Boxes,
            ResourceKind::Deployments => Icon::Rocket,
            ResourceKind::DaemonSets => Icon::Layers,
            ResourceKind::Jobs => Icon::Loader,
            ResourceKind::Services => Icon::Globe,
            ResourceKind::Ingresses => Icon::Network,
            ResourceKind::PersistentVolumes | ResourceKind::PersistentVolumeClaims => {
                Icon::HardDrive
            }
            ResourceKind::ConfigMaps => Icon::FileText,
            ResourceKind::Secrets => Icon::KeyRound,
            ResourceKind::Nodes => Icon::Cpu,
        }
    }

    pub fn nav_groups() -> Vec<(&'static str, Vec<ResourceKind>)> {
        vec![
            ("Cluster", vec![ResourceKind::Nodes]),
            (
                "Workloads",
                vec![
                    ResourceKind::Pods,
                    ResourceKind::Deployments,
                    ResourceKind::DaemonSets,
                    ResourceKind::Jobs,
                ],
            ),
            (
                "Network",
                vec![ResourceKind::Services, ResourceKind::Ingresses],
            ),
            (
                "Storage",
                vec![
                    ResourceKind::PersistentVolumes,
                    ResourceKind::PersistentVolumeClaims,
                ],
            ),
            (
                "Config",
                vec![ResourceKind::ConfigMaps, ResourceKind::Secrets],
            ),
        ]
    }

    pub fn menu_item(self) -> SidebarMenuItem {
        SidebarMenuItem::new(self.label()).icon(self.icon())
    }

    /// Cluster-scoped kinds ignore the namespace selector.
    pub fn is_namespaced(self) -> bool {
        !matches!(self, ResourceKind::Nodes | ResourceKind::PersistentVolumes)
    }
}
