use iced::{Element, Task};
use k8s_openapi::api::core::v1::{Pod, Node};
use k8s_openapi::api::apps::v1::Deployment;
use kube::Client;
use std::sync::Arc;

use crate::kubernetes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Pods,
    Nodes,
    Deployments,
}

pub struct KrustyApp {
    pub client: Option<Arc<Client>>,
    pub route: Route,
    
    pub pods: Vec<Pod>,
    pub nodes: Vec<Node>,
    pub deployments: Vec<Deployment>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum Message {
    ClientReady(Result<Client, Arc<kube::Error>>),
    RouteChanged(Route),
    RefreshRequested,
    PodsLoaded(Result<Vec<Pod>, Arc<kube::Error>>),
    NodesLoaded(Result<Vec<Node>, Arc<kube::Error>>),
    DeploymentsLoaded(Result<Vec<Deployment>, Arc<kube::Error>>),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientReady(Ok(_)) => write!(f, "ClientReady(Ok(Client))"),
            Self::ClientReady(Err(e)) => write!(f, "ClientReady(Err({:?}))", e),
            Self::RouteChanged(r) => f.debug_tuple("RouteChanged").field(r).finish(),
            Self::RefreshRequested => write!(f, "RefreshRequested"),
            Self::PodsLoaded(res) => f.debug_tuple("PodsLoaded").field(res).finish(),
            Self::NodesLoaded(res) => f.debug_tuple("NodesLoaded").field(res).finish(),
            Self::DeploymentsLoaded(res) => f.debug_tuple("DeploymentsLoaded").field(res).finish(),
        }
    }
}

impl KrustyApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                client: None,
                route: Route::Pods,
                pods: Vec::new(),
                nodes: Vec::new(),
                deployments: Vec::new(),
                error: None,
            },
            Task::perform(
                kubernetes::create_client(),
                |res| Message::ClientReady(res.map_err(Arc::new))
            )
        )
    }

    fn fetch_current_route(&self) -> Task<Message> {
        if let Some(client) = self.client.clone() {
            match self.route {
                Route::Pods => {
                    Task::perform(
                        async move { kubernetes::fetch_pods(&client, None).await },
                        |res| Message::PodsLoaded(res.map_err(Arc::new))
                    )
                }
                Route::Nodes => {
                    Task::perform(
                        async move { kubernetes::fetch_nodes(&client).await },
                        |res| Message::NodesLoaded(res.map_err(Arc::new))
                    )
                }
                Route::Deployments => {
                    Task::perform(
                        async move { kubernetes::fetch_deployments(&client, None).await },
                        |res| Message::DeploymentsLoaded(res.map_err(Arc::new))
                    )
                }
            }
        } else {
            Task::none()
        }
    }
}

pub fn update(app: &mut KrustyApp, message: Message) -> Task<Message> {
    match message {
        Message::ClientReady(Ok(client)) => {
            app.client = Some(Arc::new(client));
            app.error = None;
            return app.fetch_current_route();
        }
        Message::ClientReady(Err(e)) => {
            app.error = Some(format!("Failed to connect to Kubernetes: {}", e));
        }
        Message::RouteChanged(route) => {
            app.route = route;
            return app.fetch_current_route();
        }
        Message::RefreshRequested => {
            return app.fetch_current_route();
        }
        Message::PodsLoaded(Ok(pods)) => {
            app.pods = pods;
            app.error = None;
        }
        Message::PodsLoaded(Err(e)) => {
            app.error = Some(format!("Failed to load Pods: {}", e));
        }
        Message::NodesLoaded(Ok(nodes)) => {
            app.nodes = nodes;
            app.error = None;
        }
        Message::NodesLoaded(Err(e)) => {
            app.error = Some(format!("Failed to load Nodes: {}", e));
        }
        Message::DeploymentsLoaded(Ok(deps)) => {
            app.deployments = deps;
            app.error = None;
        }
        Message::DeploymentsLoaded(Err(e)) => {
            app.error = Some(format!("Failed to load Deployments: {}", e));
        }
    }
    Task::none()
}

pub fn view(app: &KrustyApp) -> Element<Message> {
    let content = if let Some(err) = &app.error {
        iced::widget::container(iced::widget::text(err).size(16))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
    } else {
        match app.route {
            Route::Pods => crate::ui::views::pods::view(&app.pods),
            Route::Nodes => crate::ui::views::nodes::view(&app.nodes),
            Route::Deployments => crate::ui::views::deployments::view(&app.deployments),
        }
    };
    
    let main_area = iced::widget::container(content)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding(20);
    
    iced::widget::row![
        crate::ui::sidebar::view(&app.route),
        main_area,
    ].into()
}
