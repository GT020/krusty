//! App-owned cluster configuration store.
//!
//! Clusters defined here are independent of kubectl's kubeconfig: the user can
//! paste/create a kubeconfig inside krusty and credentials live in this store
//! under the app config folder, e.g. `~/.config/krusty/clusters.json`.
//! Kubeconfig entries map 1:1 to kubectl contexts so switching later stays
//! transparent.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClusterEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "kubeconfig")]
    pub kubeconfig_yaml: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct Store {
    version: u32,
    clusters: Vec<ClusterEntry>,
    default: Option<String>,
    #[serde(default)]
    last_context: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigStore {
    entries: Vec<ClusterEntry>,
    default: Option<String>,
    last_context: Option<String>,
}

impl ConfigStore {
    /// Path under the OS app config dir, e.g. ~/.config/krusty/clusters.json.
    pub fn path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|d| d.join("krusty").join("clusters.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            tracing::debug!("store: no config directory");
            return Self::default();
        };
        let started = std::time::Instant::now();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
                Ok(store) => {
                    tracing::debug!(
                        path = %path.display(),
                        clusters = store.clusters.len(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "store: loaded"
                    );
                    Self {
                        entries: store.clusters,
                        default: store.default,
                        last_context: store.last_context,
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "store: invalid config file, ignoring"
                    );
                    Self::default()
                }
            },
            Err(error) => {
                tracing::debug!(
                    path = %path.display(),
                    %error,
                    "store: no config file yet"
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), &'static str> {
        let started = std::time::Instant::now();
        let Some(path) = Self::path() else {
            tracing::warn!("store: save failed, no config directory");
            return Err("no config directory");
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "failed to create config directory")?;
        }
        let store = Store {
            version: 1,
            clusters: self.entries.clone(),
            default: self.default.clone(),
            last_context: self.last_context.clone(),
        };
        let bytes =
            serde_json::to_vec_pretty(&store).map_err(|_| "failed to serialize cluster store")?;
        std::fs::write(&path, bytes).map_err(|_| "failed to write cluster store")?;
        tracing::debug!(
            path = %path.display(),
            clusters = self.entries.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "store: saved"
        );
        Ok(())
    }

    pub fn entries(&self) -> &[ClusterEntry] {
        &self.entries
    }

    pub fn default_id(&self) -> Option<&str> {
        self.default.as_deref()
    }

    pub fn last_context(&self) -> Option<&str> {
        self.last_context.as_deref()
    }

    pub fn set_last_context(&mut self, id: Option<&str>) {
        self.last_context = id.map(str::to_string);
    }

    pub fn upsert(&mut self, id: &str, name: &str, kubeconfig_yaml: &str) -> String {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == id) {
            existing.name = name.to_string();
            existing.kubeconfig_yaml = kubeconfig_yaml.to_string();
            return id.to_string();
        }
        let id = id.to_string();
        self.entries.push(ClusterEntry {
            id: id.clone(),
            name: name.to_string(),
            kubeconfig_yaml: kubeconfig_yaml.to_string(),
        });
        id
    }

    pub fn set_default(&mut self, id: Option<&str>) {
        self.default = id.map(str::to_string);
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        if self.default.as_deref() == Some(id) {
            self.default = None;
        }
    }
}
