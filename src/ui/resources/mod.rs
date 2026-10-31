//! Resource list, shared detail infrastructure, and per-kind detail views.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;

pub mod config_maps;
pub mod cron_jobs;
pub mod daemon_sets;
pub mod deployments;
pub mod detail;
pub mod detail_common;
pub mod ingresses;
pub mod jobs;
pub mod list;
pub mod namespace_filter;
pub mod namespaces;
pub mod node;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pod;
pub mod replica_sets;
pub mod secrets;
pub mod section;
pub mod service_accounts;
pub mod services;
pub mod stateful_sets;
pub mod storage_classes;

#[allow(clippy::too_many_arguments)]
pub fn render_detail<T: 'static>(
    kind: ResourceKind,
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    match kind {
        ResourceKind::StatefulSets => {
            stateful_sets::detail::render(state, view, compact, close, cx)
        }
        ResourceKind::ReplicaSets => replica_sets::detail::render(state, view, compact, close, cx),
        ResourceKind::DaemonSets => daemon_sets::detail::render(state, view, compact, close, cx),
        ResourceKind::CronJobs => cron_jobs::detail::render(state, view, compact, close, cx),
        ResourceKind::Jobs => jobs::detail::render(state, view, compact, close, cx),
        ResourceKind::Services => services::detail::render(state, view, compact, close, cx),
        ResourceKind::Ingresses => ingresses::detail::render(state, view, compact, close, cx),
        ResourceKind::PersistentVolumes => {
            persistent_volumes::detail::render(state, view, compact, close, cx)
        }
        ResourceKind::PersistentVolumeClaims => {
            persistent_volume_claims::detail::render(state, view, compact, close, cx)
        }
        ResourceKind::StorageClasses => {
            storage_classes::detail::render(state, view, compact, close, cx)
        }
        ResourceKind::ConfigMaps => config_maps::detail::render(state, view, compact, close, cx),
        ResourceKind::Secrets => secrets::detail::render(state, view, compact, close, cx),
        ResourceKind::ServiceAccounts => {
            service_accounts::detail::render(state, view, compact, close, cx)
        }
        ResourceKind::Namespaces => namespaces::detail::render(state, view, compact, close, cx),
        _ => detail::render_detail(kind, state, view, compact, close, cx),
    }
}
