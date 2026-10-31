//! Persisted workspace session: the open tabs and each tab's namespace scope.
//!
//! The file lives next to the other application preferences, e.g.
//! `~/.config/krusty/session.json`, so the last tab a user was looking at is
//! restored on the next launch, including its selected namespace.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::resources::catalog::{self, CustomResourceDef};
use crate::models::{NamespaceScope, ResourceKind};
use crate::view_models::shell::NavTarget;

const SESSION_FILE: &str = "session.json";
const SESSION_VERSION: u32 = 1;

/// A restorable workspace tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTab {
    pub target: NavTarget,
    pub scope: NamespaceScope,
}

/// The parts of the workspace that survive a restart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceSession {
    pub tabs: Vec<SessionTab>,
    /// Index into `tabs` of the tab that was active.
    pub active: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct StoredWorkspace {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    tabs: Vec<StoredTab>,
    #[serde(default)]
    active: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct StoredTab {
    target: StoredTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
}

#[derive(Serialize, Deserialize)]
enum StoredTarget {
    Resource(String),
    Custom(CustomResourceDef),
    Settings,
}

impl StoredTarget {
    fn from_nav(target: NavTarget) -> Option<Self> {
        match target {
            NavTarget::Settings => Some(Self::Settings),
            NavTarget::Resource(ResourceKind::Custom(index)) => {
                catalog::custom_def(index).map(|def| Self::Custom(def.clone()))
            }
            NavTarget::Resource(kind) => kind.stable_id().map(|id| Self::Resource(id.to_string())),
        }
    }

    fn to_nav(&self) -> Option<NavTarget> {
        match self {
            Self::Settings => Some(NavTarget::Settings),
            Self::Resource(id) => ResourceKind::from_stable_id(id).map(NavTarget::Resource),
            Self::Custom(def) => catalog::custom_resource_entries()
                .into_iter()
                .find(|(_, existing)| {
                    existing.group == def.group
                        && existing.version == def.version
                        && existing.plural == def.plural
                })
                .map(|(index, _)| NavTarget::Resource(ResourceKind::Custom(index))),
        }
    }
}

/// Load the saved workspace, dropping tabs whose resource no longer exists.
pub fn load() -> WorkspaceSession {
    let Some(path) = path() else {
        return WorkspaceSession::default();
    };
    match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<StoredWorkspace>(&bytes) {
            Ok(stored) => {
                let session = decode(stored);
                tracing::debug!(
                    path = %path.display(),
                    tabs = session.tabs.len(),
                    "ui: workspace session loaded"
                );
                session
            }
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "ui: invalid workspace session file, ignoring it"
                );
                WorkspaceSession::default()
            }
        },
        Err(error) => {
            tracing::debug!(
                path = %path.display(),
                %error,
                "ui: no workspace session file yet"
            );
            WorkspaceSession::default()
        }
    }
}

/// Persist the current workspace. Failure is logged and does not affect the UI.
pub fn save(session: &WorkspaceSession) {
    let Some(path) = path() else {
        return;
    };
    let stored = encode(session);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&stored)?)?;
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(
            path = %path.display(),
            %error,
            "ui: could not save workspace session"
        );
    } else {
        tracing::debug!(
            path = %path.display(),
            tabs = session.tabs.len(),
            "ui: workspace session saved"
        );
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("krusty").join(SESSION_FILE))
}

fn encode(session: &WorkspaceSession) -> StoredWorkspace {
    let tabs = session
        .tabs
        .iter()
        .filter_map(|tab| {
            let target = StoredTarget::from_nav(tab.target)?;
            let namespace = match tab.target {
                NavTarget::Resource(kind) if kind.is_namespaced() => {
                    tab.scope.namespace().map(str::to_string)
                }
                _ => None,
            };
            Some(StoredTab { target, namespace })
        })
        .collect();
    StoredWorkspace {
        version: SESSION_VERSION,
        tabs,
        active: session.active,
    }
}

fn decode(stored: StoredWorkspace) -> WorkspaceSession {
    let mut tabs = Vec::with_capacity(stored.tabs.len());
    let mut active = None;
    for (index, tab) in stored.tabs.into_iter().enumerate() {
        let Some(target) = tab.target.to_nav() else {
            continue;
        };
        if stored.active == Some(index) {
            active = Some(tabs.len());
        }
        tabs.push(SessionTab {
            target,
            scope: restore_scope(target, tab.namespace),
        });
    }
    let active = if tabs.is_empty() {
        None
    } else {
        Some(active.unwrap_or_else(|| stored.active.unwrap_or(0).min(tabs.len() - 1)))
    };
    WorkspaceSession { tabs, active }
}

fn restore_scope(target: NavTarget, namespace: Option<String>) -> NamespaceScope {
    match target {
        NavTarget::Resource(kind) if kind.is_namespaced() => namespace
            .filter(|namespace| !namespace.is_empty())
            .map(NamespaceScope::One)
            .unwrap_or(NamespaceScope::All),
        _ => NamespaceScope::All,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(kind: ResourceKind) -> NavTarget {
        NavTarget::Resource(kind)
    }

    #[test]
    fn tabs_and_namespaces_round_trip() {
        let session = WorkspaceSession {
            tabs: vec![
                SessionTab {
                    target: resource(ResourceKind::Pods),
                    scope: NamespaceScope::One("kube-system".to_string()),
                },
                SessionTab {
                    target: resource(ResourceKind::Services),
                    scope: NamespaceScope::All,
                },
                SessionTab {
                    target: NavTarget::Settings,
                    scope: NamespaceScope::All,
                },
            ],
            active: Some(1),
        };

        let decoded = decode(encode(&session));

        assert_eq!(decoded, session);
    }

    #[test]
    fn cluster_scoped_resources_drop_the_namespace() {
        let session = WorkspaceSession {
            tabs: vec![SessionTab {
                target: resource(ResourceKind::Nodes),
                scope: NamespaceScope::One("default".to_string()),
            }],
            active: Some(0),
        };

        let stored = encode(&session);
        assert!(stored.tabs[0].namespace.is_none());

        let decoded = decode(stored);
        assert_eq!(decoded.tabs[0].scope, NamespaceScope::All);
    }

    #[test]
    fn unknown_resources_are_skipped_and_active_stays_in_range() {
        let stored = StoredWorkspace {
            version: SESSION_VERSION,
            tabs: vec![
                StoredTab {
                    target: StoredTarget::Resource("NotAKind".to_string()),
                    namespace: None,
                },
                StoredTab {
                    target: StoredTarget::Resource("Pods".to_string()),
                    namespace: None,
                },
            ],
            active: Some(0),
        };

        let decoded = decode(stored);

        assert_eq!(decoded.tabs.len(), 1);
        assert_eq!(decoded.active, Some(0));
        assert_eq!(decoded.tabs[0].target, resource(ResourceKind::Pods));
    }

    #[test]
    fn empty_sessions_have_no_active_tab() {
        let decoded = decode(StoredWorkspace {
            version: SESSION_VERSION,
            tabs: Vec::new(),
            active: Some(3),
        });

        assert!(decoded.tabs.is_empty());
        assert_eq!(decoded.active, None);
    }
}
