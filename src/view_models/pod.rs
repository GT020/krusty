use std::sync::Arc;
use kube::Client;
use iced::Task;
use crate::models::pod::PodModel;
use crate::repos::pod::PodRepo;

#[derive(Clone, Debug)]
pub enum Message {
    Load,
    Loaded(Result<Vec<PodModel>, Arc<kube::Error>>),
    Select(usize),
    Delete,
    Deleted(Result<(), Arc<kube::Error>>),
}

pub struct PodViewModel {
    pub items: Vec<PodModel>,
    pub selected_index: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
}

impl PodViewModel {
    pub fn new() -> Self {
        Self { items: Vec::new(), selected_index: None, loading: false, error: None }
    }

    pub fn update(&mut self, message: Message, client: Option<Arc<Client>>) -> Task<Message> {
        match message {
            Message::Load => {
                self.loading = true; self.error = None;
                if let Some(c) = client {
                    Task::perform(
                        async move { PodRepo::list(&c, None).await.map(|items| items.into_iter().map(Into::into).collect()) },
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
            Message::Deleted(Ok(_)) => { self.selected_index = None; Task::perform(async {}, |_| Message::Load) }
            Message::Deleted(Err(e)) => { self.error = Some(format!("Delete failed: {}", e)); Task::none() }
        }
    }
}
