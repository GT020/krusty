use iced::{Element, Task};
use kube::Client;
use std::sync::Arc;

use crate::view_models::{
    pod::{PodViewModel, Message as PodMessage},
    node::{NodeViewModel, Message as NodeMessage},
    deployment::{DeploymentViewModel, Message as DeploymentMessage},
    secret::{SecretViewModel, Message as SecretMessage},
    event::{EventViewModel, Message as EventMessage},
    service::{ServiceViewModel, Message as ServiceMessage},
    ingress::{IngressViewModel, Message as IngressMessage},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Pods,
    Nodes,
    Deployments,
    Secrets,
    Events,
    Services,
    Ingress,
}

pub struct KrustyApp {
    pub client: Option<Arc<Client>>,
    pub route: Route,
    pub pod_vm: PodViewModel,
    pub node_vm: NodeViewModel,
    pub deployment_vm: DeploymentViewModel,
    pub secret_vm: SecretViewModel,
    pub event_vm: EventViewModel,
    pub service_vm: ServiceViewModel,
    pub ingress_vm: IngressViewModel,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum Message {
    ClientReady(Result<Client, Arc<kube::Error>>),
    RouteChanged(Route),
    RefreshRequested,
    Pod(PodMessage),
    Node(NodeMessage),
    Deployment(DeploymentMessage),
    Secret(SecretMessage),
    Event(EventMessage),
    Service(ServiceMessage),
    Ingress(IngressMessage),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientReady(Ok(_)) => write!(f, "ClientReady(Ok(Client))"),
            Self::ClientReady(Err(e)) => write!(f, "ClientReady(Err({:?}))", e),
            Self::RouteChanged(r) => f.debug_tuple("RouteChanged").field(r).finish(),
            Self::RefreshRequested => write!(f, "RefreshRequested"),
            Self::Pod(msg) => f.debug_tuple("Pod").field(msg).finish(),
            Self::Node(msg) => f.debug_tuple("Node").field(msg).finish(),
            Self::Deployment(msg) => f.debug_tuple("Deployment").field(msg).finish(),
            Self::Secret(msg) => f.debug_tuple("Secret").field(msg).finish(),
            Self::Event(msg) => f.debug_tuple("Event").field(msg).finish(),
            Self::Service(msg) => f.debug_tuple("Service").field(msg).finish(),
            Self::Ingress(msg) => f.debug_tuple("Ingress").field(msg).finish(),
        }
    }
}

impl KrustyApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                client: None,
                route: Route::Pods,
                pod_vm: PodViewModel::new(),
                node_vm: NodeViewModel::new(),
                deployment_vm: DeploymentViewModel::new(),
                secret_vm: SecretViewModel::new(),
                event_vm: EventViewModel::new(),
                service_vm: ServiceViewModel::new(),
                ingress_vm: IngressViewModel::new(),
                error: None,
            },
            Task::perform(
                kube::Client::try_default(),
                |res| Message::ClientReady(res.map_err(Arc::new))
            )
        )
    }

    fn fetch_current_route(&mut self) -> Task<Message> {
        let client = self.client.clone();
        match self.route {
            Route::Pods => self.pod_vm.update(PodMessage::Load, client).map(Message::Pod),
            Route::Nodes => self.node_vm.update(NodeMessage::Load, client).map(Message::Node),
            Route::Deployments => self.deployment_vm.update(DeploymentMessage::Load, client).map(Message::Deployment),
            Route::Secrets => self.secret_vm.update(SecretMessage::Load, client).map(Message::Secret),
            Route::Events => self.event_vm.update(EventMessage::Load, client).map(Message::Event),
            Route::Services => self.service_vm.update(ServiceMessage::Load, client).map(Message::Service),
            Route::Ingress => self.ingress_vm.update(IngressMessage::Load, client).map(Message::Ingress),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        if self.client.is_some() {
            iced::time::every(std::time::Duration::from_secs(5))
                .map(|_| Message::RefreshRequested)
        } else {
            iced::Subscription::none()
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
        Message::Pod(msg) => {
            return app.pod_vm.update(msg, app.client.clone()).map(Message::Pod);
        }
        Message::Node(msg) => {
            return app.node_vm.update(msg, app.client.clone()).map(Message::Node);
        }
        Message::Deployment(msg) => {
            return app.deployment_vm.update(msg, app.client.clone()).map(Message::Deployment);
        }
        Message::Secret(msg) => {
            return app.secret_vm.update(msg, app.client.clone()).map(Message::Secret);
        }
        Message::Event(msg) => {
            return app.event_vm.update(msg, app.client.clone()).map(Message::Event);
        }
        Message::Service(msg) => {
            return app.service_vm.update(msg, app.client.clone()).map(Message::Service);
        }
        Message::Ingress(msg) => {
            return app.ingress_vm.update(msg, app.client.clone()).map(Message::Ingress);
        }
    }
    Task::none()
}

pub fn view(app: &KrustyApp) -> Element<Message> {
    let content: Element<Message> = if let Some(err) = &app.error {
        iced::widget::container(iced::widget::text(err).size(16))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .center_x(iced::Length::Fill)
            .center_y(iced::Length::Fill)
            .into()
    } else {
        match app.route {
            Route::Pods => crate::ui::views::pod::view(&app.pod_vm).map(Message::Pod),
            Route::Nodes => crate::ui::views::node::view(&app.node_vm).map(Message::Node),
            Route::Deployments => crate::ui::views::deployment::view(&app.deployment_vm).map(Message::Deployment),
            Route::Secrets => crate::ui::views::secret::view(&app.secret_vm).map(Message::Secret),
            Route::Events => crate::ui::views::event::view(&app.event_vm).map(Message::Event),
            Route::Services => crate::ui::views::service::view(&app.service_vm).map(Message::Service),
            Route::Ingress => crate::ui::views::ingress::view(&app.ingress_vm).map(Message::Ingress),
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
