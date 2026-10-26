//! ViewModel layer: GPUI entities owning presentation state, bound to models.

pub mod config_maps;
pub mod daemon_sets;
pub mod deployments;
pub mod ingresses;
pub mod jobs;
pub mod node;
pub mod persistent_volume_claims;
pub mod persistent_volumes;
pub mod pod;
pub mod secrets;
pub mod services;

pub use config_maps::ConfigMapsViewModel;
pub use daemon_sets::DaemonSetsViewModel;
pub use deployments::DeploymentsViewModel;
pub use ingresses::IngressesViewModel;
pub use jobs::JobsViewModel;
pub use node::NodesViewModel;
pub use persistent_volume_claims::PersistentVolumeClaimsViewModel;
pub use persistent_volumes::PersistentVolumesViewModel;
pub use pod::PodsViewModel;
pub use secrets::SecretsViewModel;
pub use services::ServicesViewModel;

use gpui_kit::component::table::DataTable;
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, Subscription, Window};
use crate::models::{NamespaceScope, ResourceKind, SnapshotPhase};

pub enum ResourceViewModel {
    Pods(Entity<PodsViewModel>),
    Deployments(Entity<DeploymentsViewModel>),
    DaemonSets(Entity<DaemonSetsViewModel>),
    Jobs(Entity<JobsViewModel>),
    Services(Entity<ServicesViewModel>),
    Ingresses(Entity<IngressesViewModel>),
    PersistentVolumes(Entity<PersistentVolumesViewModel>),
    PersistentVolumeClaims(Entity<PersistentVolumeClaimsViewModel>),
    ConfigMaps(Entity<ConfigMapsViewModel>),
    Secrets(Entity<SecretsViewModel>),
    Nodes(Entity<NodesViewModel>),
}

impl ResourceViewModel {
    pub fn new<V: 'static>(kind: ResourceKind, window: &mut Window, cx: &mut Context<V>) -> Self {
        match kind {
            ResourceKind::Pods => Self::Pods(cx.new(|cx| PodsViewModel::new(window, cx))),
            ResourceKind::Deployments => Self::Deployments(cx.new(|cx| DeploymentsViewModel::new(window, cx))),
            ResourceKind::DaemonSets => Self::DaemonSets(cx.new(|cx| DaemonSetsViewModel::new(window, cx))),
            ResourceKind::Jobs => Self::Jobs(cx.new(|cx| JobsViewModel::new(window, cx))),
            ResourceKind::Services => Self::Services(cx.new(|cx| ServicesViewModel::new(window, cx))),
            ResourceKind::Ingresses => Self::Ingresses(cx.new(|cx| IngressesViewModel::new(window, cx))),
            ResourceKind::PersistentVolumes => Self::PersistentVolumes(cx.new(|cx| PersistentVolumesViewModel::new(window, cx))),
            ResourceKind::PersistentVolumeClaims => Self::PersistentVolumeClaims(cx.new(|cx| PersistentVolumeClaimsViewModel::new(window, cx))),
            ResourceKind::ConfigMaps => Self::ConfigMaps(cx.new(|cx| ConfigMapsViewModel::new(window, cx))),
            ResourceKind::Secrets => Self::Secrets(cx.new(|cx| SecretsViewModel::new(window, cx))),
            ResourceKind::Nodes => Self::Nodes(cx.new(|cx| NodesViewModel::new(window, cx))),
        }
    }

    pub fn observe<V: 'static>(&self, cx: &mut Context<V>, on_notify: impl Fn(&mut V, &mut Context<V>) + 'static) -> Subscription {
        match self {
            Self::Pods(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Deployments(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::DaemonSets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Jobs(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Services(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Ingresses(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::PersistentVolumes(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::PersistentVolumeClaims(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::ConfigMaps(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Secrets(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
            Self::Nodes(vm) => cx.observe(vm, move |this, _, cx| on_notify(this, cx)),
        }
    }

    pub fn set_active<V: 'static>(&mut self, active: bool, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Deployments(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::DaemonSets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Jobs(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Services(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Ingresses(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::PersistentVolumes(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::PersistentVolumeClaims(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::ConfigMaps(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Secrets(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.set_active(active, cx)),
        }
    }

    pub fn refresh<V: 'static>(&self, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Deployments(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::DaemonSets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Jobs(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Services(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Ingresses(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::PersistentVolumes(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::PersistentVolumeClaims(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::ConfigMaps(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Secrets(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
            Self::Nodes(vm) => vm.update(cx, |vm, cx| vm.refresh(cx)),
        }
    }

    pub fn scroll_to_top<V: 'static>(&self, cx: &mut Context<V>) {
        match self {
            Self::Pods(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Deployments(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::DaemonSets(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Jobs(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Services(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Ingresses(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::PersistentVolumes(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::PersistentVolumeClaims(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::ConfigMaps(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Secrets(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
            Self::Nodes(vm) => {
                let table = vm.read(cx).table().clone();
                table.update(cx, |t, cx| t.scroll_to_row(0, cx));
            }
        }
    }

    pub fn scope(&self, cx: &App) -> NamespaceScope {
        match self {
            Self::Pods(vm) => vm.read(cx).scope().clone(),
            Self::Deployments(vm) => vm.read(cx).scope().clone(),
            Self::DaemonSets(vm) => vm.read(cx).scope().clone(),
            Self::Jobs(vm) => vm.read(cx).scope().clone(),
            Self::Services(vm) => vm.read(cx).scope().clone(),
            Self::Ingresses(vm) => vm.read(cx).scope().clone(),
            Self::PersistentVolumes(vm) => vm.read(cx).scope().clone(),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).scope().clone(),
            Self::ConfigMaps(vm) => vm.read(cx).scope().clone(),
            Self::Secrets(vm) => vm.read(cx).scope().clone(),
            Self::Nodes(vm) => vm.read(cx).scope().clone(),
        }
    }

    pub fn phase(&self, cx: &App) -> SnapshotPhase {
        match self {
            Self::Pods(vm) => vm.read(cx).phase(cx),
            Self::Deployments(vm) => vm.read(cx).phase(cx),
            Self::DaemonSets(vm) => vm.read(cx).phase(cx),
            Self::Jobs(vm) => vm.read(cx).phase(cx),
            Self::Services(vm) => vm.read(cx).phase(cx),
            Self::Ingresses(vm) => vm.read(cx).phase(cx),
            Self::PersistentVolumes(vm) => vm.read(cx).phase(cx),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).phase(cx),
            Self::ConfigMaps(vm) => vm.read(cx).phase(cx),
            Self::Secrets(vm) => vm.read(cx).phase(cx),
            Self::Nodes(vm) => vm.read(cx).phase(cx),
        }
    }

    pub fn item_count(&self, cx: &App) -> usize {
        match self {
            Self::Pods(vm) => vm.read(cx).item_count(cx),
            Self::Deployments(vm) => vm.read(cx).item_count(cx),
            Self::DaemonSets(vm) => vm.read(cx).item_count(cx),
            Self::Jobs(vm) => vm.read(cx).item_count(cx),
            Self::Services(vm) => vm.read(cx).item_count(cx),
            Self::Ingresses(vm) => vm.read(cx).item_count(cx),
            Self::PersistentVolumes(vm) => vm.read(cx).item_count(cx),
            Self::PersistentVolumeClaims(vm) => vm.read(cx).item_count(cx),
            Self::ConfigMaps(vm) => vm.read(cx).item_count(cx),
            Self::Secrets(vm) => vm.read(cx).item_count(cx),
            Self::Nodes(vm) => vm.read(cx).item_count(cx),
        }
    }

    pub fn render_table(&self, cx: &App) -> gpui_kit::AnyElement {
        match self {
            Self::Pods(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Deployments(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::DaemonSets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Jobs(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Services(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Ingresses(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::PersistentVolumes(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::PersistentVolumeClaims(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::ConfigMaps(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Secrets(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
            Self::Nodes(vm) => DataTable::new(vm.read(cx).table()).into_any_element(),
        }
    }
}
