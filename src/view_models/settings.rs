use iced::Task;
use crate::models::settings::{FetchMode, SettingsModel};

#[derive(Clone, Debug)]
pub enum Message {
    IntervalChanged(u64),
    SetFetchMode(String, FetchMode),
}

pub struct SettingsViewModel {
    pub settings: SettingsModel,
}

impl SettingsViewModel {
    pub fn new() -> Self {
        Self {
            settings: SettingsModel::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IntervalChanged(secs) => {
                self.settings.refresh_interval = secs.max(1);
            }
            Message::SetFetchMode(resource, mode) => {
                self.settings.set_fetch_mode(&resource, mode);
            }
        }
        Task::none()
    }
}
