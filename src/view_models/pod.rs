use std::sync::Arc;
use kube::Client;
use iced::{Task, Subscription};
use crate::models::pod::PodModel;
use crate::repos::pod::PodRepo;
use crate::models::settings::{SettingsModel, FetchMode};

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
    Tick,
    WatchEvent(crate::kubernetes::KubeWatchEvent<PodModel>),
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

    pub fn subscription(&self, settings: &SettingsModel, _namespace: Option<String>, client: std::sync::Arc<Client>) -> Subscription<Message> {
        let mode = settings.get_fetch_mode("Pods");
        match mode {
            FetchMode::Polling => {
                iced::time::every(std::time::Duration::from_secs(settings.refresh_interval))
                    .map(|_| Message::Tick)
            }
            FetchMode::Watcher => {
                crate::kubernetes::watch_namespaced_resource::<k8s_openapi::api::core::v1::Pod, _, _>(
                    ("pods_watch", _namespace.clone()),
                    client,
                    _namespace,
                    |evt| {
                        let m = match evt {
                            crate::kubernetes::KubeWatchEvent::Added(i) => crate::kubernetes::KubeWatchEvent::Added(i.into()),
                            crate::kubernetes::KubeWatchEvent::Modified(i) => crate::kubernetes::KubeWatchEvent::Modified(i.into()),
                            crate::kubernetes::KubeWatchEvent::Deleted(i) => crate::kubernetes::KubeWatchEvent::Deleted(i.into()),
                        };
                        Message::WatchEvent(m)
                    }
                )
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
                        async move { PodRepo::list(&c, ns.as_deref()).await.map(|items| items.into_iter().map(Into::into).collect()) },
                        |res| Message::Loaded(res.map_err(Arc::new))
                    )
                } else { Task::none() }
            }
            Message::Loaded(Ok(items)) => {
                self.loading = false;
                self.items = items;
                self.items.sort_by(|a, b| a.name.cmp(&b.name));
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
            Message::Tick => Task::none(),
            Message::WatchEvent(evt) => {
                match evt {
                    crate::kubernetes::KubeWatchEvent::Added(item) | crate::kubernetes::KubeWatchEvent::Modified(item) => {
                        if let Some(pos) = self.items.iter().position(|i| i.name == item.name && i.namespace == item.namespace) {
                            if self.items[pos] != item { self.items[pos] = item; }
                        } else {
                            self.items.push(item);
                        }
                    }
                    crate::kubernetes::KubeWatchEvent::Deleted(item) => {
                        self.items.retain(|i| !(i.name == item.name && i.namespace == item.namespace));
                    }
                }
                self.items.sort_by(|a, b| a.name.cmp(&b.name));
                Task::none()
            }
        }
    }
}
