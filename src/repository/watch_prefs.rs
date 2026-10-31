//! Persisted per-resource-kind update preferences: watch vs poll, and the
//! poll rate.
//!
//! Lives under the app config folder, e.g. `~/.config/krusty/poll-rates.json`.
//! Kinds without an entry use [`WatchPrefs::default`]: live watch with a
//! five-second poll fallback.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{PollRate, ResourceKind};

/// How the user wants a resource kind kept up to date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchPreference {
    /// Live watch stream; falls back to polling when the API denies watch.
    #[default]
    Watch,
    /// Never watch; re-list on the configured poll rate.
    Poll,
}

/// The full per-kind update preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchPrefs {
    #[serde(default)]
    pub mode: WatchPreference,
    #[serde(default)]
    pub rate: PollRate,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Store {
    version: u32,
    #[serde(default)]
    kinds: HashMap<String, WatchPrefs>,
}

#[derive(Debug, Default)]
pub struct WatchPrefsStore {
    kinds: HashMap<String, WatchPrefs>,
}

/// The factory preference for a kind. Namespaces default to polling: many
/// clusters deny `watch` on them and they change rarely. Picking "Live watch"
/// in the UI saves and sticks across launches.
fn default_prefs_for(kind: ResourceKind) -> WatchPrefs {
    match kind {
        ResourceKind::Namespaces => WatchPrefs {
            mode: WatchPreference::Poll,
            rate: PollRate::DEFAULT,
        },
        _ => WatchPrefs::default(),
    }
}

impl WatchPrefsStore {
    /// Path under the OS app config dir, e.g. ~/.config/krusty/poll-rates.json.
    pub fn path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|d| d.join("krusty").join("poll-rates.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            tracing::debug!("watch prefs: no config directory");
            return Self::default();
        };
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
                Ok(store) => {
                    tracing::debug!(
                        path = %path.display(),
                        kinds = store.kinds.len(),
                        "watch prefs: loaded"
                    );
                    Self { kinds: store.kinds }
                }
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "watch prefs: invalid config file, ignoring"
                    );
                    Self::default()
                }
            },
            Err(error) => {
                tracing::debug!(
                    path = %path.display(),
                    %error,
                    "watch prefs: no config file yet"
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), &'static str> {
        let Some(path) = Self::path() else {
            tracing::warn!("watch prefs: save failed, no config directory");
            return Err("no config directory");
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "failed to create config directory")?;
        }
        let store = Store {
            version: 2,
            kinds: self.kinds.clone(),
        };
        let bytes =
            serde_json::to_vec_pretty(&store).map_err(|_| "failed to serialize watch prefs")?;
        std::fs::write(&path, bytes).map_err(|_| "failed to write watch prefs")?;
        tracing::debug!(path = %path.display(), kinds = self.kinds.len(), "watch prefs: saved");
        Ok(())
    }

    /// The saved preference for a kind, or its factory default.
    pub fn prefs_for(&self, kind: ResourceKind) -> WatchPrefs {
        self.kinds
            .get(kind.label())
            .copied()
            .unwrap_or_else(|| default_prefs_for(kind))
    }

    /// Change a kind's preference. Entries equal to the kind's factory
    /// default are dropped.
    pub fn update(&mut self, kind: ResourceKind, change: impl FnOnce(&mut WatchPrefs)) {
        let mut prefs = self.prefs_for(kind);
        change(&mut prefs);
        if prefs == default_prefs_for(kind) {
            self.kinds.remove(kind.label());
        } else {
            self.kinds.insert(kind.label().to_string(), prefs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_live_watch_with_five_second_fallback() {
        let store = WatchPrefsStore::default();
        assert_eq!(
            store.prefs_for(ResourceKind::Pods),
            WatchPrefs {
                mode: WatchPreference::Watch,
                rate: PollRate::Every(5),
            }
        );
    }

    #[test]
    fn namespaces_default_to_polling_until_watch_is_chosen() {
        let mut store = WatchPrefsStore::default();
        assert_eq!(
            store.prefs_for(ResourceKind::Namespaces),
            WatchPrefs {
                mode: WatchPreference::Poll,
                rate: PollRate::Every(5),
            }
        );

        // Choosing live watch is saved and sticks.
        store.update(ResourceKind::Namespaces, |prefs| {
            prefs.mode = WatchPreference::Watch;
        });
        assert_eq!(
            store.prefs_for(ResourceKind::Namespaces).mode,
            WatchPreference::Watch
        );
        assert!(store.kinds.contains_key("Namespaces"));

        // Switching back to the factory default drops the entry.
        store.update(ResourceKind::Namespaces, |prefs| {
            prefs.mode = WatchPreference::Poll;
        });
        assert!(!store.kinds.contains_key("Namespaces"));
    }

    #[test]
    fn remembers_mode_and_rate_independently() {
        let mut store = WatchPrefsStore::default();
        store.update(ResourceKind::Pods, |prefs| prefs.rate = PollRate::Every(30));
        store.update(ResourceKind::Events, |prefs| {
            prefs.mode = WatchPreference::Poll;
            prefs.rate = PollRate::Off;
        });

        // A custom rate survives switching between watch and poll.
        store.update(ResourceKind::Pods, |prefs| {
            prefs.mode = WatchPreference::Poll
        });
        assert_eq!(
            store.prefs_for(ResourceKind::Pods),
            WatchPrefs {
                mode: WatchPreference::Poll,
                rate: PollRate::Every(30),
            }
        );
        store.update(ResourceKind::Pods, |prefs| {
            prefs.mode = WatchPreference::Watch;
        });
        assert_eq!(
            store.prefs_for(ResourceKind::Pods),
            WatchPrefs {
                mode: WatchPreference::Watch,
                rate: PollRate::Every(30),
            }
        );

        assert_eq!(
            store.prefs_for(ResourceKind::Events),
            WatchPrefs {
                mode: WatchPreference::Poll,
                rate: PollRate::Off,
            }
        );
        assert_eq!(store.prefs_for(ResourceKind::Nodes), WatchPrefs::default());
    }

    #[test]
    fn resetting_to_default_drops_the_entry() {
        let mut store = WatchPrefsStore::default();
        store.update(ResourceKind::Pods, |prefs| prefs.rate = PollRate::Every(30));
        store.update(ResourceKind::Pods, |prefs| prefs.rate = PollRate::DEFAULT);
        assert!(!store.kinds.contains_key("Pods"));
    }

    #[test]
    fn serializes_as_kind_labels() {
        let mut store = WatchPrefsStore::default();
        store.update(ResourceKind::Deployments, |prefs| {
            prefs.mode = WatchPreference::Poll;
            prefs.rate = PollRate::Every(10);
        });
        let json = serde_json::to_string(&Store {
            version: 2,
            kinds: store.kinds.clone(),
        })
        .expect("serialize");
        assert!(json.contains("\"Deployments\""));
        let parsed: Store = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed.kinds["Deployments"],
            WatchPrefs {
                mode: WatchPreference::Poll,
                rate: PollRate::Every(10),
            }
        );
    }
}
