//! ViewModel layer: GPUI entities owning presentation state, bound to models.

pub mod resources;
pub mod shell;

pub use resources::{
    ClusterRolesViewModel, ConfigMapsViewModel, CronJobsViewModel, CustomResourceViewModel,
    DaemonSetsViewModel, DeploymentsViewModel, EndpointSlicesViewModel, EndpointsViewModel,
    EventsViewModel, HorizontalPodAutoscalersViewModel, IngressesViewModel, JobsViewModel,
    LeasesViewModel, NamespacesViewModel, NetworkPoliciesViewModel, NodesViewModel,
    PersistentVolumeClaimsViewModel, PersistentVolumesViewModel, PodDisruptionBudgetsViewModel,
    PodsViewModel, ReplicaSetsViewModel, ReplicationControllersViewModel, RoleBindingsViewModel,
    RolesViewModel, SecretsViewModel, ServiceAccountsViewModel, ServicesViewModel,
    StatefulSetsViewModel, StorageClassesViewModel,
};

use crate::models::{NamespaceScope, ResourceKind, SnapshotPhase};
use gpui_kit::component::table::{DataTable, TableEvent};
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, Subscription, Window};

pub enum ResourceViewModel {
    Pods(Entity<PodsViewModel>),
    Deployments(Entity<DeploymentsViewModel>),
    StatefulSets(Entity<StatefulSetsViewModel>),
    ReplicaSets(Entity<ReplicaSetsViewModel>),
    DaemonSets(Entity<DaemonSetsViewModel>),
    CronJobs(Entity<CronJobsViewModel>),
    Jobs(Entity<JobsViewModel>),
    Services(Entity<ServicesViewModel>),
    Ingresses(Entity<IngressesViewModel>),
    PersistentVolumes(Entity<PersistentVolumesViewModel>),
    PersistentVolumeClaims(Entity<PersistentVolumeClaimsViewModel>),
    ConfigMaps(Entity<ConfigMapsViewModel>),
    Secrets(Entity<SecretsViewModel>),
    ServiceAccounts(Entity<ServiceAccountsViewModel>),
    StorageClasses(Entity<StorageClassesViewModel>),
    Namespaces(Entity<NamespacesViewModel>),
    Nodes(Entity<NodesViewModel>),
    ClusterRoles(Entity<ClusterRolesViewModel>),
    EndpointSlices(Entity<EndpointSlicesViewModel>),
    Endpoints(Entity<EndpointsViewModel>),
    Events(Entity<EventsViewModel>),
    HorizontalPodAutoscalers(Entity<HorizontalPodAutoscalersViewModel>),
    Leases(Entity<LeasesViewModel>),
    NetworkPolicies(Entity<NetworkPoliciesViewModel>),
    PodDisruptionBudgets(Entity<PodDisruptionBudgetsViewModel>),
    ReplicationControllers(Entity<ReplicationControllersViewModel>),
    RoleBindings(Entity<RoleBindingsViewModel>),
    Roles(Entity<RolesViewModel>),
    Custom(Entity<CustomResourceViewModel>),
}

impl ResourceViewModel {
    pub fn new<V: 'static>(
        kind: ResourceKind,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Self {
        match kind {
            ResourceKind::Pods => {
                Self::Pods(cx.new(|cx| PodsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Deployments => {
                Self::Deployments(cx.new(|cx| DeploymentsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::StatefulSets => Self::StatefulSets(
                cx.new(|cx| StatefulSetsViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::ReplicaSets => {
                Self::ReplicaSets(cx.new(|cx| ReplicaSetsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::DaemonSets => {
                Self::DaemonSets(cx.new(|cx| DaemonSetsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::CronJobs => {
                Self::CronJobs(cx.new(|cx| CronJobsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Jobs => {
                Self::Jobs(cx.new(|cx| JobsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Services => {
                Self::Services(cx.new(|cx| ServicesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Ingresses => {
                Self::Ingresses(cx.new(|cx| IngressesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::PersistentVolumes => Self::PersistentVolumes(
                cx.new(|cx| PersistentVolumesViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::PersistentVolumeClaims => Self::PersistentVolumeClaims(
                cx.new(|cx| PersistentVolumeClaimsViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::ConfigMaps => {
                Self::ConfigMaps(cx.new(|cx| ConfigMapsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Secrets => {
                Self::Secrets(cx.new(|cx| SecretsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::ServiceAccounts => Self::ServiceAccounts(
                cx.new(|cx| ServiceAccountsViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::StorageClasses => Self::StorageClasses(
                cx.new(|cx| StorageClassesViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::Namespaces => {
                Self::Namespaces(cx.new(|cx| NamespacesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Nodes => {
                Self::Nodes(cx.new(|cx| NodesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::ClusterRoles => Self::ClusterRoles(
                cx.new(|cx| ClusterRolesViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::EndpointSlices => Self::EndpointSlices(
                cx.new(|cx| EndpointSlicesViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::Endpoints => {
                Self::Endpoints(cx.new(|cx| EndpointsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Events => {
                Self::Events(cx.new(|cx| EventsViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::HorizontalPodAutoscalers => Self::HorizontalPodAutoscalers(
                cx.new(|cx| HorizontalPodAutoscalersViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::Leases => {
                Self::Leases(cx.new(|cx| LeasesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::NetworkPolicies => Self::NetworkPolicies(
                cx.new(|cx| NetworkPoliciesViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::PodDisruptionBudgets => Self::PodDisruptionBudgets(
                cx.new(|cx| PodDisruptionBudgetsViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::ReplicationControllers => Self::ReplicationControllers(
                cx.new(|cx| ReplicationControllersViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::RoleBindings => Self::RoleBindings(
                cx.new(|cx| RoleBindingsViewModel::new(scope.clone(), window, cx)),
            ),
            ResourceKind::Roles => {
                Self::Roles(cx.new(|cx| RolesViewModel::new(scope.clone(), window, cx)))
            }
            ResourceKind::Custom(_) => Self::Custom(
                cx.new(|cx| CustomResourceViewModel::new(kind, scope.clone(), window, cx)),
            ),
        }
    }

    pub fn observe<V: 'static>(
        &self,
        cx: &mut Context<V>,
        on_notify: impl Fn(&mut V, &mut Context<V>) + 'static,
    ) -> Subscription {
        match self {
            Self::Pods(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Deployments(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::StatefulSets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::ReplicaSets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::CronJobs(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::ServiceAccounts(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::StorageClasses(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Namespaces(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::DaemonSets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Jobs(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Services(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Ingresses(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::PersistentVolumes(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::PersistentVolumeClaims(vm) => {
                cx.observe(vm, move |this, _, cx| on_notify(this, cx))
            }
            Self::ConfigMaps(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Secrets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Nodes(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::ClusterRoles(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::EndpointSlices(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Endpoints(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Events(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::HorizontalPodAutoscalers(vm) => {
                cx.observe(vm, move |this, _, cx| on_notify(this, cx))
            }
            Self::Leases(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::NetworkPolicies(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::PodDisruptionBudgets(vm) => {
                cx.observe(vm, move |this, _, cx| on_notify(this, cx))
            }
            Self::ReplicationControllers(vm) => {
                cx.observe(vm, move |this, _, cx| on_notify(this, cx))
            }
            Self::RoleBindings(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Roles(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Custom(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
        }
    }

    pub fn item_at(&self, row: usize, cx: &App) -> Option<crate::models::Item> {
        match self {
            Self::Pods(vm) => vm.read(cx).item_at(row, cx),
            Self::Deployments(vm) => vm.read(cx).item_at(row, cx),
            Self::StatefulSets(vm) => vm.read(cx).item_at(row, cx),
            Self::ReplicaSets(vm) => vm.read(cx).item_at(row, cx),
            Self::CronJobs(vm) => vm.read(cx).item_at(row, cx),
            Self::ServiceAccounts(vm) => vm.read(cx).item_at(row, cx),
            Self::StorageClasses(vm) => vm.read(cx).item_at(row, cx),
            Self::Namespaces(vm) => vm.read(cx).item_at(row, cx),
            Self::DaemonSets(vm) => vm.read(cx).item_at(row, cx),
            Self::Jobs(vm) => vm.read(cx).item_at(row, cx),
            Self::Services(vm) => vm.read(cx).item_at(row, cx),
            Self::Ingresses(vm) => vm.read(cx).item_at(row, cx),
            Self::PersistentVolumes(vm) => vm.read(cx).item_at(row, cx),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).item_at(row, cx),
            Self::ConfigMaps(vm) => vm.read(cx).item_at(row, cx),
            Self::Secrets(vm) => vm.read(cx).item_at(row, cx),
            Self::Nodes(vm) => vm.read(cx).item_at(row, cx),
            Self::ClusterRoles(vm) => vm.read(cx).item_at(row, cx),
            Self::EndpointSlices(vm) => vm.read(cx).item_at(row, cx),
            Self::Endpoints(vm) => vm.read(cx).item_at(row, cx),
            Self::Events(vm) => vm.read(cx).item_at(row, cx),
            Self::HorizontalPodAutoscalers(vm) => vm.read(cx).item_at(row, cx),
            Self::Leases(vm) => vm.read(cx).item_at(row, cx),
            Self::NetworkPolicies(vm) => vm.read(cx).item_at(row, cx),
            Self::PodDisruptionBudgets(vm) => vm.read(cx).item_at(row, cx),
            Self::ReplicationControllers(vm) => vm.read(cx).item_at(row, cx),
            Self::RoleBindings(vm) => vm.read(cx).item_at(row, cx),
            Self::Roles(vm) => vm.read(cx).item_at(row, cx),
            Self::Custom(vm) => vm.read(cx).item_at(row, cx),
        }
    }

    pub fn observe_row_selected<V: 'static>(
        &self,
        cx: &mut Context<V>,
        mut on_select: impl FnMut(&mut V, usize, &mut Context<V>) + 'static,
    ) -> Subscription {
        match self {
            Self::Pods(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Deployments(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::StatefulSets(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::ReplicaSets(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::CronJobs(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::ServiceAccounts(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::StorageClasses(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Namespaces(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::DaemonSets(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Jobs(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Services(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Ingresses(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::PersistentVolumes(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::PersistentVolumeClaims(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::ConfigMaps(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Secrets(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Nodes(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::ClusterRoles(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::EndpointSlices(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Endpoints(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Events(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::HorizontalPodAutoscalers(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Leases(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::NetworkPolicies(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::PodDisruptionBudgets(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::ReplicationControllers(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::RoleBindings(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Roles(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
            Self::Custom(vm) => {
                let table = vm.read(cx).table().clone();
                cx.subscribe(&table, move |this, _, event: &TableEvent, cx| {
                    if let TableEvent::SelectRow(row) = event {
                        on_select(this, *row, cx);
                    }
                })
            }
        }
    }

    pub fn set_active<V: 'static>(&mut self, active: bool, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Deployments(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::StatefulSets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ReplicaSets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::CronJobs(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ServiceAccounts(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::StorageClasses(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Namespaces(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::DaemonSets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Jobs(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Services(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Ingresses(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::PersistentVolumes(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::PersistentVolumeClaims(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ConfigMaps(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Secrets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ClusterRoles(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::EndpointSlices(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Endpoints(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Events(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::HorizontalPodAutoscalers(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Leases(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::NetworkPolicies(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::PodDisruptionBudgets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ReplicationControllers(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::RoleBindings(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Roles(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Custom(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
        }
    }

    pub fn set_scope<V: 'static>(&self, scope: NamespaceScope, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Deployments(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::StatefulSets(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::ReplicaSets(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::CronJobs(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::ServiceAccounts(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::StorageClasses(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Namespaces(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::DaemonSets(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Jobs(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Services(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Ingresses(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::PersistentVolumes(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::PersistentVolumeClaims(vm) => {
                vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx))
            }
            Self::ConfigMaps(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Secrets(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::ClusterRoles(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::EndpointSlices(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Endpoints(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Events(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::HorizontalPodAutoscalers(vm) => {
                vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx))
            }
            Self::Leases(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::NetworkPolicies(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::PodDisruptionBudgets(vm) => {
                vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx))
            }
            Self::ReplicationControllers(vm) => {
                vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx))
            }
            Self::RoleBindings(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Roles(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
            Self::Custom(vm) => vm.update(cx, |vm, cx| vm.set_scope(scope.clone(), cx)),
        }
    }

    pub fn refresh<V: 'static>(&self, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Deployments(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::StatefulSets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ReplicaSets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::CronJobs(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ServiceAccounts(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::StorageClasses(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Namespaces(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::DaemonSets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Jobs(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Services(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Ingresses(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::PersistentVolumes(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::PersistentVolumeClaims(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ConfigMaps(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Secrets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ClusterRoles(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::EndpointSlices(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Endpoints(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Events(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::HorizontalPodAutoscalers(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Leases(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::NetworkPolicies(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::PodDisruptionBudgets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ReplicationControllers(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::RoleBindings(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Roles(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Custom(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
        }
    }

    pub fn scope(&self, cx: &App) -> NamespaceScope {
        match self {
            Self::Pods(vm) => vm.read(cx).scope().clone(),
            Self::Deployments(vm) => vm.read(cx).scope().clone(),
            Self::StatefulSets(vm) => vm.read(cx).scope().clone(),
            Self::ReplicaSets(vm) => vm.read(cx).scope().clone(),
            Self::CronJobs(vm) => vm.read(cx).scope().clone(),
            Self::ServiceAccounts(vm) => vm.read(cx).scope().clone(),
            Self::StorageClasses(vm) => vm.read(cx).scope().clone(),
            Self::Namespaces(vm) => vm.read(cx).scope().clone(),
            Self::DaemonSets(vm) => vm.read(cx).scope().clone(),
            Self::Jobs(vm) => vm.read(cx).scope().clone(),
            Self::Services(vm) => vm.read(cx).scope().clone(),
            Self::Ingresses(vm) => vm.read(cx).scope().clone(),
            Self::PersistentVolumes(vm) => vm.read(cx).scope().clone(),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).scope().clone(),
            Self::ConfigMaps(vm) => vm.read(cx).scope().clone(),
            Self::Secrets(vm) => vm.read(cx).scope().clone(),
            Self::Nodes(vm) => vm.read(cx).scope().clone(),
            Self::ClusterRoles(vm) => vm.read(cx).scope().clone(),
            Self::EndpointSlices(vm) => vm.read(cx).scope().clone(),
            Self::Endpoints(vm) => vm.read(cx).scope().clone(),
            Self::Events(vm) => vm.read(cx).scope().clone(),
            Self::HorizontalPodAutoscalers(vm) => vm.read(cx).scope().clone(),
            Self::Leases(vm) => vm.read(cx).scope().clone(),
            Self::NetworkPolicies(vm) => vm.read(cx).scope().clone(),
            Self::PodDisruptionBudgets(vm) => vm.read(cx).scope().clone(),
            Self::ReplicationControllers(vm) => vm.read(cx).scope().clone(),
            Self::RoleBindings(vm) => vm.read(cx).scope().clone(),
            Self::Roles(vm) => vm.read(cx).scope().clone(),
            Self::Custom(vm) => vm.read(cx).scope().clone(),
        }
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        match self {
            Self::Pods(vm) => vm.read(cx).phase(cx),
            Self::Deployments(vm) => vm.read(cx).phase(cx),
            Self::StatefulSets(vm) => vm.read(cx).phase(cx),
            Self::ReplicaSets(vm) => vm.read(cx).phase(cx),
            Self::CronJobs(vm) => vm.read(cx).phase(cx),
            Self::ServiceAccounts(vm) => vm.read(cx).phase(cx),
            Self::StorageClasses(vm) => vm.read(cx).phase(cx),
            Self::Namespaces(vm) => vm.read(cx).phase(cx),
            Self::DaemonSets(vm) => vm.read(cx).phase(cx),
            Self::Jobs(vm) => vm.read(cx).phase(cx),
            Self::Services(vm) => vm.read(cx).phase(cx),
            Self::Ingresses(vm) => vm.read(cx).phase(cx),
            Self::PersistentVolumes(vm) => vm.read(cx).phase(cx),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).phase(cx),
            Self::ConfigMaps(vm) => vm.read(cx).phase(cx),
            Self::Secrets(vm) => vm.read(cx).phase(cx),
            Self::Nodes(vm) => vm.read(cx).phase(cx),
            Self::ClusterRoles(vm) => vm.read(cx).phase(cx),
            Self::EndpointSlices(vm) => vm.read(cx).phase(cx),
            Self::Endpoints(vm) => vm.read(cx).phase(cx),
            Self::Events(vm) => vm.read(cx).phase(cx),
            Self::HorizontalPodAutoscalers(vm) => vm.read(cx).phase(cx),
            Self::Leases(vm) => vm.read(cx).phase(cx),
            Self::NetworkPolicies(vm) => vm.read(cx).phase(cx),
            Self::PodDisruptionBudgets(vm) => vm.read(cx).phase(cx),
            Self::ReplicationControllers(vm) => vm.read(cx).phase(cx),
            Self::RoleBindings(vm) => vm.read(cx).phase(cx),
            Self::Roles(vm) => vm.read(cx).phase(cx),
            Self::Custom(vm) => vm.read(cx).phase(cx),
        }
    }

    pub fn item_count(&self, cx: &App) -> usize {
        match self {
            Self::Pods(vm) => vm.read(cx).item_count(cx),
            Self::Deployments(vm) => vm.read(cx).item_count(cx),
            Self::StatefulSets(vm) => vm.read(cx).item_count(cx),
            Self::ReplicaSets(vm) => vm.read(cx).item_count(cx),
            Self::CronJobs(vm) => vm.read(cx).item_count(cx),
            Self::ServiceAccounts(vm) => vm.read(cx).item_count(cx),
            Self::StorageClasses(vm) => vm.read(cx).item_count(cx),
            Self::Namespaces(vm) => vm.read(cx).item_count(cx),
            Self::DaemonSets(vm) => vm.read(cx).item_count(cx),
            Self::Jobs(vm) => vm.read(cx).item_count(cx),
            Self::Services(vm) => vm.read(cx).item_count(cx),
            Self::Ingresses(vm) => vm.read(cx).item_count(cx),
            Self::PersistentVolumes(vm) => vm.read(cx).item_count(cx),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).item_count(cx),
            Self::ConfigMaps(vm) => vm.read(cx).item_count(cx),
            Self::Secrets(vm) => vm.read(cx).item_count(cx),
            Self::Nodes(vm) => vm.read(cx).item_count(cx),
            Self::ClusterRoles(vm) => vm.read(cx).item_count(cx),
            Self::EndpointSlices(vm) => vm.read(cx).item_count(cx),
            Self::Endpoints(vm) => vm.read(cx).item_count(cx),
            Self::Events(vm) => vm.read(cx).item_count(cx),
            Self::HorizontalPodAutoscalers(vm) => vm.read(cx).item_count(cx),
            Self::Leases(vm) => vm.read(cx).item_count(cx),
            Self::NetworkPolicies(vm) => vm.read(cx).item_count(cx),
            Self::PodDisruptionBudgets(vm) => vm.read(cx).item_count(cx),
            Self::ReplicationControllers(vm) => vm.read(cx).item_count(cx),
            Self::RoleBindings(vm) => vm.read(cx).item_count(cx),
            Self::Roles(vm) => vm.read(cx).item_count(cx),
            Self::Custom(vm) => vm.read(cx).item_count(cx),
        }
    }

    pub fn render_table(&self, cx: &App) -> gpui_kit::AnyElement {
        match self {
            Self::Pods(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Deployments(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::StatefulSets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::ReplicaSets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::CronJobs(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::ServiceAccounts(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::StorageClasses(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Namespaces(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::DaemonSets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Jobs(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Services(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Ingresses(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::PersistentVolumes(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::PersistentVolumeClaims(vm) => {
                DataTable::new(vm.read(cx).table()).into_any_element()
            }
            Self::ConfigMaps(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Secrets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Nodes(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::ClusterRoles(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::EndpointSlices(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Endpoints(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Events(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::HorizontalPodAutoscalers(vm) => {
                DataTable::new(vm.read(cx).table()).into_any_element()
            }
            Self::Leases(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::NetworkPolicies(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::PodDisruptionBudgets(vm) => {
                DataTable::new(vm.read(cx).table()).into_any_element()
            }
            Self::ReplicationControllers(vm) => {
                DataTable::new(vm.read(cx).table()).into_any_element()
            }
            Self::RoleBindings(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Roles(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Custom(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
        }
    }
}
