//! Pod detail and usage domain types (no gpui or kube dependencies).

use std::collections::BTreeMap;

pub use crate::models::usage::{UsageHistory, UsageSample as PodUsage};

#[derive(Clone, Debug, Default)]
pub struct PodDetail {
    pub name: String,
    pub namespace: String,
    pub uid: String,
    pub resource_version: String,
    pub created_at: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub node_name: String,
    pub service_account: String,
    pub restart_policy: String,
    pub dns_policy: String,
    pub host_network: bool,
    pub hostname: String,
    pub subdomain: String,
    pub priority_class: String,
    pub priority: Option<i32>,
    pub qos_class: String,
    pub phase: String,
    pub pod_ip: String,
    pub pod_ips: Vec<String>,
    pub host_ip: String,
    pub nominated_node_name: String,
    pub start_time: Option<i64>,
    pub conditions: Vec<PodCondition>,
    pub containers: Vec<PodContainer>,
    pub init_containers: Vec<PodContainer>,
    pub volumes: Vec<PodVolume>,
    pub tolerations: Vec<PodToleration>,
    pub owners: Vec<PodOwner>,
    pub raw_json: String,
}

impl PodDetail {
    pub fn status_label(&self) -> &str {
        if self.phase.is_empty() {
            "Unknown"
        } else {
            &self.phase
        }
    }

    pub fn ready_containers(&self) -> usize {
        self.containers
            .iter()
            .filter(|container| container.ready)
            .count()
    }

    pub fn restart_total(&self) -> u32 {
        self.containers
            .iter()
            .map(|container| container.restart_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
pub struct PodCondition {
    pub type_: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition: Option<i64>,
    pub last_probe: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct PodContainer {
    pub name: String,
    pub image: String,
    pub image_pull_policy: String,
    pub ready: bool,
    pub restart_count: u32,
    pub state: String,
    pub state_detail: String,
    pub last_state: String,
    pub container_id: String,
    pub image_id: String,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub ports: Vec<String>,
    pub env: Vec<String>,
    pub mounts: Vec<String>,
    pub requests: BTreeMap<String, String>,
    pub limits: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct PodVolume {
    pub name: String,
    pub type_: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct PodToleration {
    pub key: String,
    pub operator: String,
    pub value: String,
    pub effect: String,
    pub seconds: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct PodOwner {
    pub kind: String,
    pub name: String,
    pub controller: bool,
}

/// Presentation-agnostic state for the open pod detail pane.
#[derive(Clone, Debug, Default)]
pub enum PodDetailState {
    #[default]
    Closed,
    Loading {
        namespace: String,
        name: String,
    },
    Ready(Box<PodDetail>),
    Error {
        namespace: String,
        name: String,
        message: String,
    },
}

impl PodDetailState {
    pub fn identity(&self) -> Option<(&str, &str)> {
        match self {
            Self::Closed => None,
            Self::Loading { namespace, name } => Some((namespace, name)),
            Self::Ready(detail) => Some((&detail.namespace, &detail.name)),
            Self::Error {
                namespace, name, ..
            } => Some((namespace, name)),
        }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }
}

/// An update pushed from the repository while a pod detail pane is open.
#[derive(Clone, Debug)]
pub enum PodDetailUpdate {
    Detail {
        namespace: String,
        name: String,
        result: Result<Box<PodDetail>, String>,
    },
    Usage {
        namespace: String,
        name: String,
        result: Result<PodUsage, String>,
    },
}
