//! Item summaries for each built-in resource kind.
//!
//! These mappers are shared by the store engine. They are pure conversions from
//! a typed Kubernetes object to the table row shown by the UI.

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Event, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod,
    ReplicationController, Secret, Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::networking::v1::{Ingress, NetworkPolicy};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::api::rbac::v1::{ClusterRole, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::ResourceExt;
use kube::api::DynamicObject;

use crate::models::Item;

fn base_item<K>(object: &K) -> Item
where
    K: ResourceExt,
{
    let mut item = Item::new(object.name_any(), object.namespace());
    item.uid = object.uid().unwrap_or_default();
    item.resource_version = object.resource_version().unwrap_or_default();
    item.created_at = object.creation_timestamp().map(|time| time.0.as_second());
    item
}

fn version_label(labels: Option<&std::collections::BTreeMap<String, String>>) -> String {
    labels
        .and_then(|labels| labels.get("app.kubernetes.io/version").cloned())
        .unwrap_or_default()
}

pub(crate) fn pod_item(pod: &Pod) -> Item {
    let mut item = base_item(pod);
    item.node = pod
        .spec
        .as_ref()
        .and_then(|spec| spec.node_name.clone())
        .unwrap_or_default();
    let statuses = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref());
    let total = statuses.map(|statuses| statuses.len()).unwrap_or(0);
    let ready = statuses
        .map(|statuses| statuses.iter().filter(|status| status.ready).count())
        .unwrap_or(0);
    item.ready = format!("{ready}/{total}");
    item.restarts = statuses
        .map(|statuses| {
            statuses
                .iter()
                .map(|status| status.restart_count as usize)
                .sum()
        })
        .unwrap_or(0);
    item.status = pod
        .status
        .as_ref()
        .and_then(|status| status.phase.as_deref())
        .unwrap_or("unknown")
        .to_lowercase();
    let crashing = statuses.is_some_and(|statuses| {
        statuses.iter().any(|status| {
            status
                .state
                .as_ref()
                .and_then(|state| state.waiting.as_ref())
                .and_then(|waiting| waiting.reason.as_deref())
                .is_some_and(|reason| reason == "CrashLoopBackOff")
        })
    });
    if crashing {
        item.status = "crashloopbackoff".to_string();
    }
    item
}

pub(crate) fn deployment_item(deployment: &Deployment) -> Item {
    let mut item = base_item(deployment);
    let desired = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = deployment
        .status
        .as_ref()
        .and_then(|status| status.ready_replicas)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(deployment.metadata.labels.as_ref());
    item
}

pub(crate) fn daemon_set_item(daemon_set: &DaemonSet) -> Item {
    let mut item = base_item(daemon_set);
    let desired = daemon_set
        .status
        .as_ref()
        .map(|status| status.desired_number_scheduled)
        .unwrap_or(0);
    let ready = daemon_set
        .status
        .as_ref()
        .map(|status| status.number_ready)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(daemon_set.metadata.labels.as_ref());
    item
}

pub(crate) fn job_item(job: &Job) -> Item {
    let mut item = base_item(job);
    let want = job
        .spec
        .as_ref()
        .and_then(|spec| spec.completions)
        .unwrap_or(1);
    let done = job
        .status
        .as_ref()
        .and_then(|status| status.succeeded)
        .unwrap_or(0);
    item.ready = format!("{done}/{want}");
    item.capacity = want.to_string();
    item.status = if done >= want { "succeeded" } else { "running" }.to_string();
    item
}

pub(crate) fn service_item(service: &Service) -> Item {
    let mut item = base_item(service);
    let spec = service.spec.as_ref();
    item.status = spec
        .and_then(|spec| spec.type_.clone())
        .unwrap_or_else(|| "ClusterIP".to_string())
        .to_lowercase();
    item.version = spec
        .and_then(|spec| spec.ports.as_ref())
        .and_then(|ports| ports.first())
        .map(|port| port.port.to_string())
        .unwrap_or_default();
    item
}

pub(crate) fn ingress_item(ingress: &Ingress) -> Item {
    let mut item = base_item(ingress);
    item.status = "ingress".to_string();
    item.version = ingress
        .spec
        .as_ref()
        .and_then(|spec| spec.rules.as_ref())
        .and_then(|rules| rules.first())
        .and_then(|rule| rule.host.clone())
        .unwrap_or_default();
    item
}

pub(crate) fn pv_item(persistent_volume: &PersistentVolume) -> Item {
    let mut item = base_item(persistent_volume);
    item.capacity = persistent_volume
        .spec
        .as_ref()
        .and_then(|spec| spec.capacity.as_ref())
        .and_then(|capacity| capacity.get("storage"))
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.status = persistent_volume
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_default()
        .to_lowercase();
    item
}

pub(crate) fn pvc_item(persistent_volume_claim: &PersistentVolumeClaim) -> Item {
    let mut item = base_item(persistent_volume_claim);
    item.capacity = persistent_volume_claim
        .status
        .as_ref()
        .and_then(|status| status.capacity.as_ref())
        .and_then(|capacity| capacity.get("storage"))
        .or_else(|| {
            persistent_volume_claim
                .spec
                .as_ref()
                .and_then(|spec| spec.resources.as_ref())
                .and_then(|resources| resources.requests.as_ref())
                .and_then(|requests| requests.get("storage"))
        })
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.status = persistent_volume_claim
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_default()
        .to_lowercase();
    item
}

pub(crate) fn config_map_item(config_map: &ConfigMap) -> Item {
    let mut item = base_item(config_map);
    let data_keys = config_map.data.as_ref().map(|data| data.len()).unwrap_or(0);
    let binary_keys = config_map
        .binary_data
        .as_ref()
        .map(|data| data.len())
        .unwrap_or(0);
    item.status = "keys".to_string();
    item.capacity = (data_keys + binary_keys).to_string();
    item.version = config_map
        .data
        .as_ref()
        .and_then(|data| data.keys().next().cloned())
        .unwrap_or_default();
    item
}

pub(crate) fn secret_item(secret: &Secret) -> Item {
    let mut item = base_item(secret);
    item.status = "keys".to_string();
    item.capacity = secret
        .data
        .as_ref()
        .map(|data| data.len())
        .unwrap_or(0)
        .to_string();
    item.version = secret
        .data
        .as_ref()
        .and_then(|data| data.keys().next().cloned())
        .unwrap_or_default();
    item
}

pub(crate) fn stateful_set_item(stateful_set: &StatefulSet) -> Item {
    let mut item = base_item(stateful_set);
    let desired = stateful_set
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = stateful_set
        .status
        .as_ref()
        .and_then(|status| status.ready_replicas)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(stateful_set.metadata.labels.as_ref());
    item
}

pub(crate) fn replica_set_item(replica_set: &ReplicaSet) -> Item {
    let mut item = base_item(replica_set);
    let desired = replica_set
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = replica_set
        .status
        .as_ref()
        .and_then(|status| status.ready_replicas)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(replica_set.metadata.labels.as_ref());
    item
}

pub(crate) fn replication_controller_item(controller: &ReplicationController) -> Item {
    let mut item = base_item(controller);
    let desired = controller
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(0);
    let ready = controller
        .status
        .as_ref()
        .and_then(|status| status.ready_replicas)
        .unwrap_or(0);
    item.ready = format!("{ready}/{desired}");
    item.capacity = desired.to_string();
    item.status = if ready >= desired {
        "ready"
    } else {
        "progressing"
    }
    .to_string();
    item.version = version_label(controller.metadata.labels.as_ref());
    item
}

pub(crate) fn horizontal_pod_autoscaler_item(autoscaler: &HorizontalPodAutoscaler) -> Item {
    let mut item = base_item(autoscaler);
    let spec = autoscaler.spec.as_ref();
    let max = spec.map(|spec| spec.max_replicas).unwrap_or(0);
    let min = spec.and_then(|spec| spec.min_replicas).unwrap_or(0);
    let current = autoscaler
        .status
        .as_ref()
        .and_then(|status| status.current_replicas)
        .unwrap_or(0);
    item.ready = format!("{current}/{min}");
    item.capacity = max.to_string();
    item.status = if current >= min { "active" } else { "idle" }.to_string();
    item.version = spec
        .map(|spec| {
            format!(
                "{}/{}",
                spec.scale_target_ref.kind, spec.scale_target_ref.name
            )
        })
        .unwrap_or_default();
    item
}

pub(crate) fn pod_disruption_budget_item(budget: &PodDisruptionBudget) -> Item {
    let mut item = base_item(budget);
    let status = budget.status.as_ref();
    let current = status.map(|status| status.current_healthy).unwrap_or(0);
    let desired = status.map(|status| status.desired_healthy).unwrap_or(0);
    let allowed = status.map(|status| status.disruptions_allowed).unwrap_or(0);
    item.ready = format!("{current}/{desired}");
    item.capacity = allowed.to_string();
    item.status = if allowed > 0 { "ready" } else { "blocked" }.to_string();
    item
}

pub(crate) fn lease_item(lease: &Lease) -> Item {
    let mut item = base_item(lease);
    let spec = lease.spec.as_ref();
    item.version = spec
        .and_then(|spec| spec.holder_identity.clone())
        .unwrap_or_default();
    item.capacity = spec
        .and_then(|spec| spec.lease_duration_seconds)
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_default();
    item.status = "active".to_string();
    item
}

pub(crate) fn endpoints_item(endpoints: &Endpoints) -> Item {
    let mut item = base_item(endpoints);
    let addresses = endpoints
        .subsets
        .as_ref()
        .map(|subsets| {
            subsets
                .iter()
                .map(|subset| {
                    subset.addresses.as_ref().map(Vec::len).unwrap_or(0)
                        + subset
                            .not_ready_addresses
                            .as_ref()
                            .map(Vec::len)
                            .unwrap_or(0)
                })
                .sum::<usize>()
        })
        .unwrap_or(0);
    item.capacity = addresses.to_string();
    item.status = "active".to_string();
    item
}

pub(crate) fn endpoint_slice_item(slice: &EndpointSlice) -> Item {
    let mut item = base_item(slice);
    item.capacity = slice.endpoints.len().to_string();
    item.version = slice.address_type.clone();
    item.status = "active".to_string();
    item
}

pub(crate) fn network_policy_item(policy: &NetworkPolicy) -> Item {
    let mut item = base_item(policy);
    item.capacity = policy
        .spec
        .as_ref()
        .and_then(|spec| spec.policy_types.as_ref())
        .map(|types| types.join(", "))
        .unwrap_or_default();
    item.status = "active".to_string();
    item
}

pub(crate) fn event_item(event: &Event) -> Item {
    let mut item = base_item(event);
    item.status = event
        .type_
        .clone()
        .unwrap_or_else(|| "normal".to_string())
        .to_lowercase();
    item.version = event.reason.clone().unwrap_or_default();
    item.capacity = event.count.unwrap_or(1).to_string();
    item
}

pub(crate) fn cluster_role_item(cluster_role: &ClusterRole) -> Item {
    let mut item = base_item(cluster_role);
    item.capacity = cluster_role
        .rules
        .as_ref()
        .map(|rules| rules.len())
        .unwrap_or(0)
        .to_string();
    item.status = "active".to_string();
    item
}

pub(crate) fn role_item(role: &Role) -> Item {
    let mut item = base_item(role);
    item.capacity = role
        .rules
        .as_ref()
        .map(|rules| rules.len())
        .unwrap_or(0)
        .to_string();
    item.status = "active".to_string();
    item
}

pub(crate) fn role_binding_item(binding: &RoleBinding) -> Item {
    let mut item = base_item(binding);
    item.capacity = binding
        .subjects
        .as_ref()
        .map(|subjects| subjects.len())
        .unwrap_or(0)
        .to_string();
    item.version = format!("{}/{}", binding.role_ref.kind, binding.role_ref.name);
    item.status = "active".to_string();
    item
}

pub(crate) fn custom_item(object: &DynamicObject) -> Item {
    let mut item = base_item(object);
    let Some(status) = object.data.get("status") else {
        return item;
    };
    if let Some(phase) = status.get("phase").and_then(serde_json::Value::as_str) {
        item.status = phase.to_lowercase();
    } else if let Some(conditions) = status
        .get("conditions")
        .and_then(serde_json::Value::as_array)
    {
        let ready = conditions.iter().find(|condition| {
            condition.get("type").and_then(serde_json::Value::as_str) == Some("Ready")
        });
        if let Some(ready) = ready {
            item.status =
                if ready.get("status").and_then(serde_json::Value::as_str) == Some("True") {
                    "ready"
                } else {
                    "notready"
                }
                .to_string();
        }
    }
    item
}

pub(crate) fn cron_job_item(cron_job: &CronJob) -> Item {
    let mut item = base_item(cron_job);
    let active = cron_job
        .status
        .as_ref()
        .and_then(|status| status.active.as_ref())
        .map(|active| active.len())
        .unwrap_or(0);
    let suspended = cron_job
        .spec
        .as_ref()
        .and_then(|spec| spec.suspend)
        .unwrap_or(false);
    item.capacity = active.to_string();
    item.status = if suspended { "suspended" } else { "active" }.to_string();
    item.version = cron_job
        .spec
        .as_ref()
        .map(|spec| spec.schedule.clone())
        .unwrap_or_default();
    item
}

pub(crate) fn service_account_item(service_account: &ServiceAccount) -> Item {
    let mut item = base_item(service_account);
    let secrets = service_account.secrets.as_ref().map(Vec::len).unwrap_or(0);
    item.status = "account".to_string();
    item.capacity = secrets.to_string();
    item.version = service_account
        .secrets
        .as_ref()
        .and_then(|secrets| secrets.first())
        .and_then(|secret| secret.name.clone())
        .unwrap_or_default();
    item
}

pub(crate) fn storage_class_item(storage_class: &StorageClass) -> Item {
    let mut item = base_item(storage_class);
    item.version = storage_class.provisioner.clone();
    item.capacity = storage_class.reclaim_policy.clone().unwrap_or_default();
    item.ready = storage_class
        .volume_binding_mode
        .clone()
        .unwrap_or_default();
    item
}

pub(crate) fn namespace_item(namespace: &Namespace) -> Item {
    let mut item = base_item(namespace);
    item.status = namespace
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase();
    item
}

pub(crate) fn node_item(node: &Node) -> Item {
    let mut item = base_item(node);
    let status = node.status.as_ref();
    let ready = status
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == "Ready")
        })
        .map(|condition| condition.status == "True")
        .unwrap_or(false);
    item.status = if ready { "ready" } else { "notready" }.to_string();
    item.capacity = status
        .and_then(|status| status.allocatable.as_ref())
        .and_then(|allocatable| allocatable.get("cpu"))
        .map(|quantity| quantity.0.clone())
        .unwrap_or_default();
    item.version = status
        .and_then(|status| status.node_info.as_ref())
        .map(|info| info.kubelet_version.clone())
        .unwrap_or_default();
    item
}
