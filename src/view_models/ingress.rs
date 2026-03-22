use std::sync::Arc;
use kube::Client;
use iced::{Task, Subscription};
use crate::models::ingress::IngressModel;
use crate::repos::ingress::IngressRepo;
use crate::models::settings::{SettingsModel, FetchMode};

#[derive(Clone, Debug)]
pub enum Message {
    Load(Option<String>),
    Loaded(Result<Vec<IngressModel>, Arc<kube::Error>>),
    Select(usize),
    Delete,
    Deleted(Result<(), Arc<kube::Error>>),
    Tick,
}

pub struct IngressViewModel {
    pub items: Vec<IngressModel>,
    pub selected_index: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
}

impl IngressViewModel {
    pub fn new() -> Self {
        Self { items: Vec::new(), selected_index: None, loading: false, error: None }
    }

    pub fn subscription(&self, settings: &SettingsModel, _namespace: Option<String>, _client: std::sync::Arc<Client>) -> Subscription<Message> {
        let mode = settings.get_fetch_mode("Ingress");
        match mode {
            FetchMode::Polling => {
                iced::time::every(std::time::Duration::from_secs(settings.refresh_interval))
                    .map(|_| Message::Tick)
            }
            FetchMode::Watcher => {
                iced::time::every(std::time::Duration::from_millis(1500))
                    .map(|_| Message::Tick)
            }
        }
    }

    pub fn update(&mut self, message: Message, client: Option<Arc<Client>>) -> Task<Message> {
        match message {
            Message::Load(namespace) => {
                self.loading = true; self.error = None;
                if let Some(c) = client {
                    let ns = namespace.clone();
                    Task::perform(
                        async move { IngressRepo::list(&c, ns.as_deref()).await.map(|items| items.into_iter().map(Into::into).collect()) },
                        |res| Message::Loaded(res.map_err(Arc::new))
                    )
                } else { Task::none() }
            }
            Message::Loaded(Ok(items)) => {
                self.loading = false;
                if self.items != items {
                    self.items = items;
                }
                if let Some(idx) = self.selected_index { if idx >= self.items.len() { self.selected_index = None; } }
                Task::none()
            }
            Message::Loaded(Err(e)) => { self.loading = false; self.error = Some(e.to_string()); Task::none() }
            Message::Select(idx) => {
                if Some(idx) == self.selected_index { self.selected_index = None; } else { self.selected_index = Some(idx); }
                Task::none()
            }
            Message::Delete => {
                if let (Some(idx), Some(c)) = (self.selected_index, client) {
                    if let Some(item) = self.items.get(idx).cloned() {
                        return Task::perform(
                            async move { IngressRepo::delete(&c, &item.name, &item.namespace).await },
                            |res| Message::Deleted(res.map_err(Arc::new))
                        );
                    }
                }
                Task::none()
            }
            Message::Deleted(Ok(_)) => { self.selected_index = None; Task::perform(async {}, |_| Message::Load(None)) }
            Message::Deleted(Err(e)) => { self.error = Some(format!("Delete failed: {}", e)); Task::none() }
            Message::Tick => Task::none(),
        }
    }
}
