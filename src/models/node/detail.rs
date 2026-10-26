//! Node detail and usage domain types (no gpui or kube dependencies).

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct NodeDetail {
    pub name: String,
    pub uid: String,
    pub resource_version: String,
    pub created_at: Option<i64>,
    pub roles: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub unschedulable: bool,
    pub provider_id: String,
    pub pod_cidr: String,
    pub pod_cidrs: Vec<String>,
    pub addresses: Vec<NodeAddress>,
    pub capacity: BTreeMap<String, String>,
    pub allocatable: BTreeMap<String, String>,
    pub node_info: NodeSystemInfo,
    pub conditions: Vec<NodeCondition>,
    pub taints: Vec<NodeTaint>,
    pub images: Vec<NodeImage>,
    /// The complete node object as returned by the API server.
    pub raw_json: String,
}

impl NodeDetail {
    pub fn is_ready(&self) -> bool {
        self.conditions
            .iter()
            .find(|condition| condition.type_ == "Ready")
            .is_some_and(|condition| condition.status == "True")
    }

    pub fn status_label(&self) -> &'static str {
        if self.is_ready() { "Ready" } else { "NotReady" }
    }

    pub fn roles_label(&self) -> String {
        if self.roles.is_empty() {
            "worker".to_string()
        } else {
            self.roles.join(", ")
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NodeSystemInfo {
    pub machine_id: String,
    pub system_uuid: String,
    pub boot_id: String,
    pub kernel_version: String,
    pub os_image: String,
    pub container_runtime_version: String,
    pub kubelet_version: String,
    pub kube_proxy_version: String,
    pub operating_system: String,
    pub architecture: String,
}

#[derive(Clone, Debug)]
pub struct NodeAddress {
    pub type_: String,
    pub address: String,
}

#[derive(Clone, Debug)]
pub struct NodeCondition {
    pub type_: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_heartbeat: Option<i64>,
    pub last_transition: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct NodeTaint {
    pub key: String,
    pub value: String,
    pub effect: String,
}

#[derive(Clone, Debug)]
pub struct NodeImage {
    pub names: Vec<String>,
    pub size_bytes: i64,
}

/// One point of live node usage, sampled from metrics-server.
#[derive(Clone, Debug)]
pub struct NodeUsage {
    pub observed_at: i64,
    pub cpu_used_millicores: Option<u64>,
    pub cpu_capacity_millicores: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_capacity_bytes: Option<u64>,
}

impl NodeUsage {
    pub fn cpu_percent(&self) -> Option<f32> {
        let used = self.cpu_used_millicores? as f32;
        let capacity = self.cpu_capacity_millicores? as f32;
        (capacity > 0.0).then(|| (used / capacity * 100.0).clamp(0.0, 100.0))
    }

    pub fn memory_percent(&self) -> Option<f32> {
        let used = self.memory_used_bytes? as f32;
        let capacity = self.memory_capacity_bytes? as f32;
        (capacity > 0.0).then(|| (used / capacity * 100.0).clamp(0.0, 100.0))
    }
}

/// Presentation-agnostic state for the open node detail pane.
#[derive(Clone, Debug, Default)]
pub enum NodeDetailState {
    #[default]
    Closed,
    Loading {
        node: String,
    },
    Ready(Box<NodeDetail>),
    Error {
        node: String,
        message: String,
    },
}

impl NodeDetailState {
    pub fn node(&self) -> Option<&str> {
        match self {
            Self::Closed => None,
            Self::Loading { node } => Some(node),
            Self::Ready(detail) => Some(&detail.name),
            Self::Error { node, .. } => Some(node),
        }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }
}

/// An update pushed from the repository while a node detail pane is open.
#[derive(Clone, Debug)]
pub enum NodeDetailUpdate {
    Detail {
        node: String,
        result: Result<Box<NodeDetail>, String>,
    },
    Usage {
        node: String,
        result: Result<NodeUsage, String>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct UsageHistory {
    samples: Vec<NodeUsage>,
    max_samples: usize,
}

impl UsageHistory {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::new(),
            max_samples: max_samples.max(2),
        }
    }

    pub fn push(&mut self, sample: NodeUsage) {
        self.samples.push(sample);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    pub fn samples(&self) -> &[NodeUsage] {
        &self.samples
    }

    pub fn latest(&self) -> Option<&NodeUsage> {
        self.samples.last()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}
