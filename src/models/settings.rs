use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchMode {
    #[default]
    Polling,
    Watcher,
}

#[derive(Debug, Clone)]
pub struct SettingsModel {
    pub refresh_interval: u64,
    // Resource map key -> FetchMode (Applies to whatever namespace is currently selected)
    // To avoid coupling to `Route` enum in models, we use String for resource type, e.g. "Pods".
    pub fetch_modes: HashMap<String, FetchMode>,
}

impl Default for SettingsModel {
    fn default() -> Self {
        Self {
            refresh_interval: 5,
            fetch_modes: HashMap::new(),
        }
    }
}

impl SettingsModel {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn get_fetch_mode(&self, resource: &str) -> FetchMode {
        self.fetch_modes.get(resource).cloned().unwrap_or_default()
    }

    pub fn set_fetch_mode(&mut self, resource: &str, mode: FetchMode) {
        self.fetch_modes.insert(resource.to_string(), mode);
    }
}
