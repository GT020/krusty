use iced::{Element, Task};
use kube::Client;
use std::sync::Arc;

use crate::view_models::{
    deployment::{DeploymentViewModel, Message as DeploymentMessage},
    event::{EventViewModel, Message as EventMessage},
    ingress::{IngressViewModel, Message as IngressMessage},
    node::{Message as NodeMessage, NodeViewModel},
    pod::{Message as PodMessage, PodViewModel},
    secret::{Message as SecretMessage, SecretViewModel},
    service::{Message as ServiceMessage, ServiceViewModel},
    settings::{Message as SettingsMessage, SettingsViewModel},
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
    Settings,
}

pub struct KrustyApp {
    pub client: Option<Arc<Client>>,
    pub route: Route,
    pub namespace: Option<String>,
    pub namespaces: Vec<String>,
    pub pod_vm: PodViewModel,
    pub node_vm: NodeViewModel,
    pub deployment_vm: DeploymentViewModel,
    pub secret_vm: SecretViewModel,
    pub event_vm: EventViewModel,
    pub service_vm: ServiceViewModel,
    pub ingress_vm: IngressViewModel,
    pub settings_vm: SettingsViewModel,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum Message {
    ClientReady(Result<Client, Arc<kube::Error>>),
    RouteChanged(Route),
    NamespaceChanged(Option<String>),
    NamespacesLoaded(Result<Vec<String>, Arc<kube::Error>>),
    RefreshRequested,
    Pod(PodMessage),
    Node(NodeMessage),
    Deployment(DeploymentMessage),
    Secret(SecretMessage),
    Event(EventMessage),
    Service(ServiceMessage),
    Ingress(IngressMessage),
    Settings(SettingsMessage),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientReady(Ok(_)) => write!(f, "ClientReady(Ok(Client))"),
            Self::ClientReady(Err(e)) => write!(f, "ClientReady(Err({:?}))", e),
            Self::RouteChanged(r) => f.debug_tuple("RouteChanged").field(r).finish(),
            Self::NamespaceChanged(ns) => f.debug_tuple("NamespaceChanged").field(ns).finish(),
            Self::NamespacesLoaded(Ok(ns)) => f.debug_tuple("NamespacesLoaded").field(ns).finish(),
            Self::NamespacesLoaded(Err(e)) => f.debug_tuple("NamespacesLoaded").field(e).finish(),
            Self::RefreshRequested => write!(f, "RefreshRequested"),
            Self::Pod(msg) => f.debug_tuple("Pod").field(msg).finish(),
            Self::Node(msg) => f.debug_tuple("Node").field(msg).finish(),
            Self::Deployment(msg) => f.debug_tuple("Deployment").field(msg).finish(),
            Self::Secret(msg) => f.debug_tuple("Secret").field(msg).finish(),
            Self::Event(msg) => f.debug_tuple("Event").field(msg).finish(),
            Self::Service(msg) => f.debug_tuple("Service").field(msg).finish(),
            Self::Ingress(msg) => f.debug_tuple("Ingress").field(msg).finish(),
            Self::Settings(msg) => f.debug_tuple("Settings").field(msg).finish(),
        }
    }
}

impl KrustyApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                client: None,
                route: Route::Pods,
                namespace: None,
                namespaces: vec!["default".to_string()],
                pod_vm: PodViewModel::new(),
                node_vm: NodeViewModel::new(),
                deployment_vm: DeploymentViewModel::new(),
                secret_vm: SecretViewModel::new(),
                event_vm: EventViewModel::new(),
                service_vm: ServiceViewModel::new(),
                ingress_vm: IngressViewModel::new(),
                settings_vm: SettingsViewModel::new(),
                error: None,
            },
            Task::perform(kube::Client::try_default(), |res| {
                Message::ClientReady(res.map_err(Arc::new))
            }),
        )
    }

    fn fetch_current_route(&mut self) -> Task<Message> {
        let client = self.client.clone();
        let namespace = self.namespace.clone();
        match self.route {
            Route::Pods => self
                .pod_vm
                .update(PodMessage::Load(namespace), client)
                .map(Message::Pod),
            Route::Nodes => self
                .node_vm
                .update(NodeMessage::Load, client)
                .map(Message::Node),
            Route::Deployments => self
                .deployment_vm
                .update(DeploymentMessage::Load(namespace.clone()), client)
                .map(Message::Deployment),
            Route::Secrets => self
                .secret_vm
                .update(SecretMessage::Load(namespace.clone()), client)
                .map(Message::Secret),
            Route::Events => self
                .event_vm
                .update(EventMessage::Load(namespace.clone()), client)
                .map(Message::Event),
            Route::Services => self
                .service_vm
                .update(ServiceMessage::Load(namespace.clone()), client)
                .map(Message::Service),
            Route::Ingress => self
                .ingress_vm
                .update(IngressMessage::Load(namespace.clone()), client)
                .map(Message::Ingress),
            Route::Settings => Task::none(),
        }
    }

    fn load_namespaces(&self) -> Task<Message> {
        if self.client.is_some() {
            Task::perform(
                async move {
                    let c = kube::Client::try_default().await?;
                    let api: kube::Api<k8s_openapi::api::core::v1::Namespace> = kube::Api::all(c);
                    let list = api.list(&kube::api::ListParams::default()).await?;
                    Ok(list.items.into_iter().filter_map(|ns| ns.metadata.name).collect())
                },
                |res| Message::NamespacesLoaded(res.map_err(Arc::new))
            )
        } else {
            Task::none()
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        if self.client.is_none() {
            return iced::Subscription::none();
        }
        let settings = &self.settings_vm.settings;
        let ns = self.namespace.clone();
        let client = self.client.clone().unwrap();
        
        match self.route {
            Route::Pods => self.pod_vm.subscription(settings, ns, client.clone()).map(Message::Pod),
            Route::Nodes => self.node_vm.subscription(settings, ns, client.clone()).map(Message::Node),
            Route::Deployments => self.deployment_vm.subscription(settings, ns, client.clone()).map(Message::Deployment),
            Route::Secrets => self.secret_vm.subscription(settings, ns, client.clone()).map(Message::Secret),
            Route::Events => self.event_vm.subscription(settings, ns, client.clone()).map(Message::Event),
            Route::Services => self.service_vm.subscription(settings, ns, client.clone()).map(Message::Service),
            Route::Ingress => self.ingress_vm.subscription(settings, ns, client.clone()).map(Message::Ingress),
            Route::Settings => iced::Subscription::none(),
        }
    }
}

pub fn update(app: &mut KrustyApp, message: Message) -> Task<Message> {
    match message {
        Message::ClientReady(Ok(client)) => {
            app.client = Some(Arc::new(client));
            app.error = None;
            let load_namespaces = app.load_namespaces();
            return Task::batch([app.fetch_current_route(), load_namespaces]);
        }
        Message::ClientReady(Err(e)) => {
            app.error = Some(format!("Failed to connect to Kubernetes: {}", e));
        }
        Message::RouteChanged(route) => {
            app.route = route;
            return app.fetch_current_route();
        }
        Message::NamespaceChanged(ns) => {
            app.namespace = ns;
            return app.fetch_current_route();
        }
        Message::NamespacesLoaded(Ok(namespaces)) => {
            app.namespaces = namespaces;
        }
        Message::NamespacesLoaded(Err(e)) => {
            app.error = Some(format!("Failed to load namespaces: {}", e));
        }
        Message::RefreshRequested => {
            return app.fetch_current_route();
        }
        Message::Pod(msg) => {
            let m = match msg {
                PodMessage::Tick => PodMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app.pod_vm.update(m, app.client.clone()).map(Message::Pod);
        }
        Message::Node(msg) => {
            return app
                .node_vm
                .update(msg, app.client.clone())
                .map(Message::Node);
        }
        Message::Deployment(msg) => {
            let m = match msg {
                DeploymentMessage::Tick => DeploymentMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app
                .deployment_vm
                .update(m, app.client.clone())
                .map(Message::Deployment);
        }
        Message::Secret(msg) => {
            let m = match msg {
                SecretMessage::Tick => SecretMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app
                .secret_vm
                .update(m, app.client.clone())
                .map(Message::Secret);
        }
        Message::Event(msg) => {
            let m = match msg {
                EventMessage::Tick => EventMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app
                .event_vm
                .update(m, app.client.clone())
                .map(Message::Event);
        }
        Message::Service(msg) => {
            let m = match msg {
                ServiceMessage::Tick => ServiceMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app
                .service_vm
                .update(m, app.client.clone())
                .map(Message::Service);
        }
        Message::Ingress(msg) => {
            let m = match msg {
                IngressMessage::Tick => IngressMessage::Load(app.namespace.clone()),
                _ => msg,
            };
            return app
                .ingress_vm
                .update(m, app.client.clone())
                .map(Message::Ingress);
        }
        Message::Settings(msg) => {
            return app.settings_vm.update(msg).map(Message::Settings);
        }
    }
    Task::none()
}

pub fn view(app: &KrustyApp) -> Element<Message> {
    let title = match app.route {
        Route::Pods => "Pods",
        Route::Nodes => "Nodes",
        Route::Deployments => "Deployments",
        Route::Secrets => "Secrets",
        Route::Events => "Events",
        Route::Services => "Services",
        Route::Ingress => "Ingress",
        Route::Settings => "Settings",
    };

    let is_loading = match app.route {
        Route::Pods => app.pod_vm.loading,
        Route::Nodes => app.node_vm.loading,
        Route::Deployments => app.deployment_vm.loading,
        Route::Secrets => app.secret_vm.loading,
        Route::Events => app.event_vm.loading,
        Route::Services => app.service_vm.loading,
        Route::Ingress => app.ingress_vm.loading,
        Route::Settings => false,
    };

    let header = crate::ui::header::view(title, &app.namespace, &app.namespaces, is_loading);

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
            Route::Deployments => {
                crate::ui::views::deployment::view(&app.deployment_vm).map(Message::Deployment)
            }
            Route::Secrets => crate::ui::views::secret::view(&app.secret_vm).map(Message::Secret),
            Route::Events => crate::ui::views::event::view(&app.event_vm).map(Message::Event),
            Route::Services => {
                crate::ui::views::service::view(&app.service_vm).map(Message::Service)
            }
            Route::Ingress => {
                crate::ui::views::ingress::view(&app.ingress_vm).map(Message::Ingress)
            }
            Route::Settings => {
                crate::ui::views::settings::view(&app.settings_vm, &app.namespaces).map(Message::Settings)
            }
        }
    };

    let main_area = iced::widget::container(
        iced::widget::column![header, content]
            .spacing(10)
    )
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding(20)
        .style(|_| iced::widget::container::Style {
            // #F2EAE0
            background: Some(iced::Background::Color(iced::Color::from_rgb8(242, 234, 224))),
            text_color: Some(iced::Color::BLACK),
            ..Default::default()
        });

    iced::widget::row![crate::ui::sidebar::view(&app.route), main_area,].into()
}
