use super::configmaps::ConfigMapsPage;
use super::cronjobs::CronJobsPage;
use super::daemonsets::DaemonSetsPage;
use super::deployments::DeploymentsPage;
use super::endpoints::EndpointsPage;
use super::home::Home;
use super::ingresses::IngressesPage;
use super::jobs::JobsPage;
use super::namespaces::NamespacesPage;
use super::persistentvolumeclaims::PersistentVolumeClaimsPage;
use super::pods::PodsPage;
use super::secrets::SecretsPage;
use super::serviceaccounts::ServiceAccountsPage;
use super::services::ServicesPage;
use super::sidebar::Sidebar;
use super::statefulsets::StatefulSetsPage;
use dioxus::prelude::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Sidebar)]
        #[route("/")]
        Home {},
        #[route("/pods")]
        PodsPage {},
        #[route("/deployments")]
        DeploymentsPage {},
        #[route("/services")]
        ServicesPage {},
        #[route("/configmaps")]
        ConfigMapsPage {},
        #[route("/secrets")]
        SecretsPage {},
        #[route("/persistentvolumeclaims")]
        PersistentVolumeClaimsPage {},
        #[route("/statefulsets")]
        StatefulSetsPage {},
        #[route("/daemonsets")]
        DaemonSetsPage {},
        #[route("/ingresses")]
        IngressesPage {},
        #[route("/jobs")]
        JobsPage {},
        #[route("/cronjobs")]
        CronJobsPage {},
        #[route("/serviceaccounts")]
        ServiceAccountsPage {},
        #[route("/endpoints")]
        EndpointsPage {},
        #[route("/namespaces")]
        NamespacesPage {},
}
