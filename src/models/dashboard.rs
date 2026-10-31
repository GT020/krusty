//! Serializable dashboard descriptors.
//!
//! Only widget identity, target, context, and layout are persisted. Live state
//! and subscriptions are rebuilt by remounting per-resource widgets.

use serde::{Deserialize, Serialize};

pub const DASHBOARD_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DashboardId(pub u64);

impl std::fmt::Display for DashboardId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "dashboard-{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WidgetId(pub u64);

impl WidgetId {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WidgetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "widget-{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetSize {
    Small,
    #[default]
    Medium,
    Large,
    Full,
}

impl WidgetSize {
    pub const ALL: [Self; 4] = [Self::Small, Self::Medium, Self::Large, Self::Full];

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::Full => "Full width",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetDescriptor {
    pub context_id: String,
    pub source: WidgetSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resource", rename_all = "snake_case")]
pub enum WidgetSource {
    Pod(PodWidgetSource),
    Node(NodeWidgetSource),
    Deployment(DeploymentWidgetSource),
    Section(ResourceSectionSource),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PodWidgetSource {
    Usage {
        namespace: String,
        name: String,
    },
    Logs {
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: Option<String>,
        follow: bool,
    },
    Exec {
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: Option<String>,
        shell: String,
    },
    List {
        namespace: Option<String>,
    },
    Section {
        namespace: String,
        name: String,
        section: PodSection,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodSection {
    Overview,
    Containers,
    InitContainers,
    Conditions,
    Volumes,
    Tolerations,
    Owners,
    Labels,
    Annotations,
    Manifest,
}

impl PodSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Containers => "Containers",
            Self::InitContainers => "Init containers",
            Self::Conditions => "Conditions",
            Self::Volumes => "Volumes",
            Self::Tolerations => "Tolerations",
            Self::Owners => "Owners",
            Self::Labels => "Labels",
            Self::Annotations => "Annotations",
            Self::Manifest => "Manifest",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeWidgetSource {
    Usage { name: String },
    Section { name: String, section: NodeSection },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSection {
    Overview,
    Capacity,
    Conditions,
    System,
    Addresses,
    Taints,
    Labels,
    Annotations,
    Images,
    Manifest,
}

impl NodeSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Capacity => "Capacity",
            Self::Conditions => "Conditions",
            Self::System => "System",
            Self::Addresses => "Addresses",
            Self::Taints => "Taints",
            Self::Labels => "Labels",
            Self::Annotations => "Annotations",
            Self::Images => "Images",
            Self::Manifest => "Manifest",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeploymentWidgetSource {
    Actions {
        namespace: String,
        name: String,
    },
    Section {
        namespace: String,
        name: String,
        section: DeploymentSection,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentSection {
    Overview,
    Selector,
    Strategy,
    Conditions,
    PodTemplate,
    Labels,
    Annotations,
    Manifest,
}

impl DeploymentSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Selector => "Selector",
            Self::Strategy => "Strategy",
            Self::Conditions => "Conditions",
            Self::PodTemplate => "Pod template",
            Self::Labels => "Labels",
            Self::Annotations => "Annotations",
            Self::Manifest => "Manifest",
        }
    }
}

/// A section widget rendered from the generic resource detail JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceSectionSource {
    pub kind: crate::models::ResourceKind,
    pub namespace: String,
    pub name: String,
    pub section: ResourceSection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "part", rename_all = "snake_case")]
pub enum ResourceSection {
    Overview,
    Path { title: String, path: Vec<String> },
    Labels,
    Annotations,
    Manifest,
}

impl ResourceSection {
    pub fn path(title: impl Into<String>, path: &[&str]) -> Self {
        Self::Path {
            title: title.into(),
            path: path.iter().map(|part| (*part).to_string()).collect(),
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Overview => "Overview".to_string(),
            Self::Path { title, .. } => title.clone(),
            Self::Labels => "Labels".to_string(),
            Self::Annotations => "Annotations".to_string(),
            Self::Manifest => "Manifest".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinnedWidgetSpec {
    pub id: WidgetId,
    pub size: WidgetSize,
    pub descriptor: WidgetDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: DashboardId,
    pub name: String,
    pub widgets: Vec<PinnedWidgetSpec>,
}

impl Dashboard {
    pub fn new(id: DashboardId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            widgets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardFile {
    pub version: u32,
    pub next_widget_id: u64,
    pub next_dashboard_id: u64,
    pub active: DashboardId,
    pub dashboards: Vec<Dashboard>,
}

impl Default for DashboardFile {
    fn default() -> Self {
        let home = DashboardId(1);
        Self {
            version: DASHBOARD_VERSION,
            next_widget_id: 1,
            next_dashboard_id: 2,
            active: home,
            dashboards: vec![Dashboard::new(home, "Home")],
        }
    }
}

impl DashboardFile {
    pub fn active_dashboard(&self) -> &Dashboard {
        self.dashboards
            .iter()
            .find(|dashboard| dashboard.id == self.active)
            .unwrap_or(&self.dashboards[0])
    }

    pub fn dashboard(&self, id: DashboardId) -> Option<&Dashboard> {
        self.dashboards.iter().find(|dashboard| dashboard.id == id)
    }

    pub fn set_active(&mut self, id: DashboardId) -> bool {
        if self.active == id || !self.dashboards.iter().any(|dashboard| dashboard.id == id) {
            return false;
        }
        self.active = id;
        true
    }

    pub fn add_dashboard(&mut self, name: impl Into<String>) -> DashboardId {
        let id = DashboardId(self.next_dashboard_id);
        self.next_dashboard_id = self.next_dashboard_id.wrapping_add(1);
        self.dashboards.push(Dashboard::new(id, name));
        id
    }

    pub fn remove_dashboard(&mut self, id: DashboardId) -> bool {
        if self.dashboards.len() <= 1 {
            return false;
        }
        let before = self.dashboards.len();
        self.dashboards.retain(|dashboard| dashboard.id != id);
        if self.dashboards.len() == before {
            return false;
        }
        if self.active == id {
            self.active = self.dashboards[0].id;
        }
        true
    }

    pub fn add_widget(
        &mut self,
        dashboard: DashboardId,
        descriptor: WidgetDescriptor,
        size: WidgetSize,
    ) -> Option<WidgetId> {
        let id = WidgetId(self.next_widget_id);
        self.next_widget_id = self.next_widget_id.wrapping_add(1);
        let dashboard = self
            .dashboards
            .iter_mut()
            .find(|candidate| candidate.id == dashboard)?;
        dashboard.widgets.push(PinnedWidgetSpec {
            id,
            size,
            descriptor,
        });
        Some(id)
    }

    pub fn remove_widget(&mut self, dashboard: DashboardId, id: WidgetId) -> bool {
        let Some(dashboard) = self
            .dashboards
            .iter_mut()
            .find(|candidate| candidate.id == dashboard)
        else {
            return false;
        };
        let before = dashboard.widgets.len();
        dashboard.widgets.retain(|widget| widget.id != id);
        before != dashboard.widgets.len()
    }

    pub fn set_widget_size(
        &mut self,
        dashboard: DashboardId,
        id: WidgetId,
        size: WidgetSize,
    ) -> bool {
        let Some(dashboard) = self
            .dashboards
            .iter_mut()
            .find(|candidate| candidate.id == dashboard)
        else {
            return false;
        };
        let Some(widget) = dashboard.widgets.iter_mut().find(|widget| widget.id == id) else {
            return false;
        };
        if widget.size == size {
            return false;
        }
        widget.size = size;
        true
    }

    pub fn contains(&self, dashboard: DashboardId, descriptor: &WidgetDescriptor) -> bool {
        self.dashboard(dashboard).is_some_and(|dashboard| {
            dashboard
                .widgets
                .iter()
                .any(|widget| &widget.descriptor == descriptor)
        })
    }

    pub fn dashboard_choices(&self) -> Vec<(DashboardId, String)> {
        self.dashboards
            .iter()
            .map(|dashboard| (dashboard.id, dashboard.name.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> WidgetDescriptor {
        WidgetDescriptor {
            context_id: "kube:dev".to_string(),
            source: WidgetSource::Pod(PodWidgetSource::Usage {
                namespace: "default".to_string(),
                name: "api".to_string(),
            }),
        }
    }

    #[test]
    fn widgets_and_dashboards_keep_stable_ids() {
        let mut file = DashboardFile::default();
        let second = file.add_dashboard("Second");
        assert_eq!(file.dashboards.len(), 2);
        assert!(file.set_active(second));

        let first = file
            .add_widget(second, descriptor(), WidgetSize::Small)
            .unwrap();
        let again = file
            .add_widget(second, descriptor(), WidgetSize::Large)
            .unwrap();
        assert_ne!(first, again);
        assert_eq!(
            file.dashboard(second)
                .unwrap()
                .widgets
                .iter()
                .find(|widget| widget.id == first)
                .map(|widget| widget.size),
            Some(WidgetSize::Small)
        );

        let json = serde_json::to_string_pretty(&file).expect("serialize");
        let restored: DashboardFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.dashboards.len(), 2);
        assert_eq!(restored.active, second);
        assert_eq!(restored.next_widget_id, file.next_widget_id);
    }

    #[test]
    fn removing_dashboards_and_widgets_is_explicit() {
        let mut file = DashboardFile::default();
        let second = file.add_dashboard("Second");
        let id = file
            .add_widget(second, descriptor(), WidgetSize::Medium)
            .unwrap();
        assert!(file.set_widget_size(second, id, WidgetSize::Full));
        assert!(!file.set_widget_size(second, id, WidgetSize::Full));
        assert!(file.remove_widget(second, id));
        assert!(!file.remove_widget(second, id));
        assert!(file.remove_dashboard(second));
        assert!(!file.remove_dashboard(file.active));
    }
}
