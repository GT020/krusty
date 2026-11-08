//! Resource list, shared detail infrastructure, and per-kind detail views.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetail, ResourceDetailState};
use crate::ui::resources::detail_common::render_resource_detail;

pub mod config_maps;
pub mod cron_jobs;
pub mod daemon_sets;
pub mod deployments;
pub mod detail;
pub mod detail_common;
pub mod ingresses;
pub mod jobs;
pub mod list;
pub mod list_header;
pub mod namespace_filter;
pub mod namespaces;
pub mod node;
pub mod overview;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pod;
pub mod replica_sets;
pub mod search_filter;
pub mod secrets;
pub mod service_accounts;
pub mod services;
pub mod stateful_sets;
pub mod storage_classes;
pub mod watch_mode;

fn kind_sections<T: 'static>(
    kind: ResourceKind,
    detail: &ResourceDetail,
    cx: &mut Context<T>,
) -> Vec<AnyElement> {
    match kind {
        ResourceKind::StatefulSets => stateful_sets::detail::sections(detail, cx),
        ResourceKind::ReplicaSets => replica_sets::detail::sections(detail, cx),
        ResourceKind::DaemonSets => daemon_sets::detail::sections(detail, cx),
        ResourceKind::CronJobs => cron_jobs::detail::sections(detail, cx),
        ResourceKind::Jobs => jobs::detail::sections(detail, cx),
        ResourceKind::Services => services::detail::sections(detail, cx),
        ResourceKind::Ingresses => ingresses::detail::sections(detail, cx),
        ResourceKind::PersistentVolumes => persistent_volumes::detail::sections(detail, cx),
        ResourceKind::PersistentVolumeClaims => {
            persistent_volume_claims::detail::sections(detail, cx)
        }
        ResourceKind::StorageClasses => storage_classes::detail::sections(detail, cx),
        ResourceKind::ConfigMaps => config_maps::detail::sections(detail, cx),
        ResourceKind::Secrets => secrets::detail::sections(detail, cx),
        ResourceKind::ServiceAccounts => service_accounts::detail::sections(detail, cx),
        ResourceKind::Namespaces => namespaces::detail::sections(detail, cx),
        _ => Vec::new(),
    }
}

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
        ResourceKind::StatefulSets
        | ResourceKind::ReplicaSets
        | ResourceKind::DaemonSets
        | ResourceKind::CronJobs
        | ResourceKind::Jobs
        | ResourceKind::Services
        | ResourceKind::Ingresses
        | ResourceKind::PersistentVolumes
        | ResourceKind::PersistentVolumeClaims
        | ResourceKind::StorageClasses
        | ResourceKind::ConfigMaps
        | ResourceKind::Secrets
        | ResourceKind::ServiceAccounts
        | ResourceKind::Namespaces => {
            render_resource_detail(state, view, compact, close, cx, |detail, cx| {
                kind_sections(kind, detail, cx)
            })
        }
        _ => detail::render_detail(state, view, compact, close, cx),
    }
}
