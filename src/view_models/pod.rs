use std::sync::Arc;
use kube::Client;
use iced::Task;
use crate::models::pod::PodModel;
use crate::repos::pod::PodRepo;

#[derive(Clone, Debug)]
pub enum Message {
    Load(Option<String>),
    Loaded(Result<Vec<PodModel>, Arc<kube::Error>>),
    Select(usize),
    Delete,
    Deleted(Result<(), Arc<kube::Error>>),
    ViewLogs,
    LogsLoaded(Result<String, Arc<kube::Error>>),
    ClearLogs,
}

pub struct PodViewModel {
    pub items: Vec<PodModel>,
    pub selected_index: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub logs: Option<String>,
    pub logs_loading: bool,
}

impl PodViewModel {
    pub fn new() -> Self {
        Self { items: Vec::new(), selected_index: None, loading: false, error: None, logs: None, logs_loading: false }
    }

    pub fn update(&mut self, message: Message, client: Option<Arc<Client>>) -> Task<Message> {
        match message {
            Message::Load(namespace) => {
                self.loading = true; self.error = None;
                if let Some(c) = client {
                    let ns = namespace.clone();
                    Task::perform(
                        async move { PodRepo::list(&c, ns.as_deref()).await.map(|items| items.into_iter().map(Into::into).collect()) },
                        |res| Message::Loaded(res.map_err(Arc::new))
                    )
                } else { Task::none() }
            }
            Message::Loaded(Ok(items)) => {
                self.loading = false; self.items = items;
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
                            async move { PodRepo::delete(&c, &item.name, &item.namespace).await },
                            |res| Message::Deleted(res.map_err(Arc::new))
                        );
                    }
                }
                Task::none()
            }
            Message::Deleted(Ok(_)) => { self.selected_index = None; Task::perform(async {}, |_| Message::Load(None)) }
            Message::Deleted(Err(e)) => { self.error = Some(format!("Delete failed: {}", e)); Task::none() }
            Message::ViewLogs => {
                if let (Some(idx), Some(c)) = (self.selected_index, client) {
                    if let Some(item) = self.items.get(idx) {
                        self.logs_loading = true;
                        let name = item.name.clone();
                        let namespace = item.namespace.clone();
                        return Task::perform(
                            async move { PodRepo::logs(&c, &name, &namespace, None).await },
                            |res| Message::LogsLoaded(res.map_err(Arc::new))
                        );
                    }
                }
                Task::none()
            }
            Message::LogsLoaded(Ok(logs)) => { self.logs_loading = false; self.logs = Some(logs); Task::none() }
            Message::LogsLoaded(Err(e)) => { self.logs_loading = false; self.logs = Some(format!("Failed to fetch logs: {}", e)); Task::none() }
            Message::ClearLogs => { self.logs = None; Task::none() }
        }
    }
}
