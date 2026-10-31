//! Persisted workspace session: the open editor groups, their tabs, each tab's
//! namespace scope, which group had focus, and the split ratio.
//!
//! The file lives next to the other application preferences, e.g.
//! `~/.config/krusty/session.json`, so the last workspace a user was looking at
//! is restored on the next launch. Version 1 files (a flat tab list) are
//! migrated into a single group.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::resources::catalog::{self, CustomResourceDef};
use crate::models::{NamespaceScope, ResourceKind};
use crate::view_models::shell::NavTarget;

const SESSION_FILE: &str = "session.json";
const SESSION_VERSION: u32 = 2;
const MIN_SPLIT: f32 = 0.2;
const MAX_SPLIT: f32 = 0.8;

/// A restorable workspace tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTab {
    pub target: NavTarget,
    pub scope: NamespaceScope,
}

/// One restorable editor group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPane {
    pub tabs: Vec<SessionTab>,
    /// Index into `tabs` of the tab that was active in this group.
    pub active: Option<usize>,
}

/// The parts of the workspace that survive a restart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceSession {
    pub panes: Vec<SessionPane>,
    /// Index into `panes` of the group that had focus.
    pub focused: Option<usize>,
    /// Left group's fraction of the editor width when split.
    pub split: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct StoredWorkspace {
    #[serde(default)]
    version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    panes: Vec<StoredPane>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    focused: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    split: Option<f32>,
    // Version 1 fields. They are read for migration and never written again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tabs: Vec<StoredTab>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct StoredPane {
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
    Overview,
    Resource(String),
    Custom(CustomResourceDef),
    Settings,
}

impl StoredTarget {
    fn from_nav(target: NavTarget) -> Option<Self> {
        match target {
            NavTarget::Overview => Some(Self::Overview),
            NavTarget::Settings => Some(Self::Settings),
            NavTarget::Resource(ResourceKind::Custom(index)) => {
                catalog::custom_def(index).map(|def| Self::Custom(def.clone()))
            }
            NavTarget::Resource(kind) => kind.stable_id().map(|id| Self::Resource(id.to_string())),
        }
    }

    fn to_nav(&self) -> Option<NavTarget> {
        match self {
            Self::Overview => Some(NavTarget::Overview),
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
                    panes = session.panes.len(),
                    tabs = session.panes.iter().map(|pane| pane.tabs.len()).sum::<usize>(),
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
            panes = session.panes.len(),
            "ui: workspace session saved"
        );
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("krusty").join(SESSION_FILE))
}

fn encode(session: &WorkspaceSession) -> StoredWorkspace {
    let focused_source = session.focused.unwrap_or(0);
    let mut panes = Vec::new();
    let mut focused = 0usize;

    for (source_index, pane) in session.panes.iter().take(2).enumerate() {
        let mut tabs = Vec::new();
        let mut active = None;
        for (index, tab) in pane.tabs.iter().enumerate() {
            let Some(target) = StoredTarget::from_nav(tab.target) else {
                continue;
            };
            let namespace = match tab.target {
                NavTarget::Resource(kind) if kind.is_namespaced() => {
                    tab.scope.namespace().map(str::to_string)
                }
                _ => None,
            };
            if pane.active == Some(index) {
                active = Some(tabs.len());
            }
            tabs.push(StoredTab { target, namespace });
        }
        if tabs.is_empty() {
            continue;
        }
        if source_index == focused_source {
            focused = panes.len();
        }
        panes.push(StoredPane {
            tabs,
            active: Some(active.unwrap_or(0)),
        });
    }

    let focused = if panes.is_empty() {
        None
    } else {
        Some(focused.min(panes.len() - 1))
    };
    let split = if panes.len() == 2 {
        session.split.map(clamp_split)
    } else {
        None
    };

    StoredWorkspace {
        version: SESSION_VERSION,
        panes,
        focused,
        split,
        tabs: Vec::new(),
        active: None,
    }
}

fn decode(stored: StoredWorkspace) -> WorkspaceSession {
    // Version 1 stored one flat tab list; wrap it into a single group.
    let raw_panes = if !stored.panes.is_empty() {
        stored.panes
    } else if !stored.tabs.is_empty() {
        vec![StoredPane {
            tabs: stored.tabs,
            active: stored.active,
        }]
    } else {
        Vec::new()
    };

    let mut panes = Vec::new();
    for pane in raw_panes.into_iter().take(2) {
        let mut tabs = Vec::new();
        let mut active = None;
        for (index, tab) in pane.tabs.into_iter().enumerate() {
            let Some(target) = tab.target.to_nav() else {
                continue;
            };
            if pane.active == Some(index) {
                active = Some(tabs.len());
            }
            tabs.push(SessionTab {
                target,
                scope: restore_scope(target, tab.namespace),
            });
        }
        if tabs.is_empty() {
            continue;
        }
        let active = active.unwrap_or(0).min(tabs.len() - 1);
        panes.push(SessionPane {
            tabs,
            active: Some(active),
        });
    }

    let focused = if panes.is_empty() {
        None
    } else {
        Some(stored.focused.unwrap_or(0).min(panes.len() - 1))
    };
    let split = if panes.len() == 2 {
        stored.split.map(clamp_split)
    } else {
        None
    };

    WorkspaceSession {
        panes,
        focused,
        split,
    }
}

fn clamp_split(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_SPLIT, MAX_SPLIT)
    } else {
        0.5
    }
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

    fn tab(target: NavTarget, scope: NamespaceScope) -> SessionTab {
        SessionTab { target, scope }
    }

    #[test]
    fn panes_tabs_namespaces_and_split_round_trip() {
        let session = WorkspaceSession {
            panes: vec![
                SessionPane {
                    tabs: vec![
                        tab(NavTarget::Overview, NamespaceScope::All),
                        tab(
                            resource(ResourceKind::Pods),
                            NamespaceScope::One("kube-system".to_string()),
                        ),
                    ],
                    active: Some(1),
                },
                SessionPane {
                    tabs: vec![
                        tab(resource(ResourceKind::Services), NamespaceScope::All),
                        tab(NavTarget::Settings, NamespaceScope::All),
                    ],
                    active: Some(0),
                },
            ],
            focused: Some(1),
            split: Some(0.6),
        };

        let decoded = decode(encode(&session));

        assert_eq!(decoded, session);
    }

    #[test]
    fn version_one_files_migrate_into_one_pane() {
        let stored = StoredWorkspace {
            version: 1,
            panes: Vec::new(),
            focused: None,
            split: None,
            tabs: vec![
                StoredTab {
                    target: StoredTarget::Overview,
                    namespace: None,
                },
                StoredTab {
                    target: StoredTarget::Resource("Pods".to_string()),
                    namespace: Some("kube-system".to_string()),
                },
            ],
            active: Some(1),
        };

        let decoded = decode(stored);

        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.focused, Some(0));
        assert_eq!(decoded.split, None);
        assert_eq!(decoded.panes[0].tabs.len(), 2);
        assert_eq!(decoded.panes[0].active, Some(1));
        assert_eq!(
            decoded.panes[0].tabs[1].scope,
            NamespaceScope::One("kube-system".to_string())
        );
    }

    #[test]
    fn cluster_scoped_resources_drop_the_namespace() {
        let session = WorkspaceSession {
            panes: vec![SessionPane {
                tabs: vec![tab(
                    resource(ResourceKind::Nodes),
                    NamespaceScope::One("default".to_string()),
                )],
                active: Some(0),
            }],
            focused: Some(0),
            split: None,
        };

        let stored = encode(&session);
        assert!(stored.panes[0].tabs[0].namespace.is_none());

        let decoded = decode(stored);
        assert_eq!(decoded.panes[0].tabs[0].scope, NamespaceScope::All);
    }

    #[test]
    fn unknown_resources_are_skipped_and_active_stays_in_range_per_pane() {
        let stored = StoredWorkspace {
            version: SESSION_VERSION,
            panes: vec![
                StoredPane {
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
                },
                StoredPane {
                    tabs: vec![StoredTab {
                        target: StoredTarget::Resource("Services".to_string()),
                        namespace: None,
                    }],
                    active: Some(1),
                },
            ],
            focused: Some(0),
            split: Some(0.5),
            tabs: Vec::new(),
            active: None,
        };

        let decoded = decode(stored);

        assert_eq!(decoded.panes.len(), 2);
        assert_eq!(decoded.panes[0].tabs.len(), 1);
        assert_eq!(
            decoded.panes[0].tabs[0].target,
            resource(ResourceKind::Pods)
        );
        assert_eq!(decoded.panes[0].active, Some(0));
        assert_eq!(decoded.panes[1].active, Some(0));
    }

    #[test]
    fn empty_panes_are_dropped_and_focus_is_clamped() {
        let stored = StoredWorkspace {
            version: SESSION_VERSION,
            panes: vec![
                StoredPane {
                    tabs: Vec::new(),
                    active: Some(0),
                },
                StoredPane {
                    tabs: vec![StoredTab {
                        target: StoredTarget::Resource("Pods".to_string()),
                        namespace: None,
                    }],
                    active: Some(0),
                },
            ],
            focused: Some(1),
            split: Some(0.5),
            tabs: Vec::new(),
            active: None,
        };

        let decoded = decode(stored);

        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.focused, Some(0));
        assert_eq!(decoded.split, None);
    }

    #[test]
    fn split_ratio_is_clamped() {
        let stored = StoredWorkspace {
            version: SESSION_VERSION,
            panes: vec![
                StoredPane {
                    tabs: vec![StoredTab {
                        target: StoredTarget::Resource("Pods".to_string()),
                        namespace: None,
                    }],
                    active: Some(0),
                },
                StoredPane {
                    tabs: vec![StoredTab {
                        target: StoredTarget::Resource("Services".to_string()),
                        namespace: None,
                    }],
                    active: Some(0),
                },
            ],
            focused: Some(0),
            split: Some(0.95),
            tabs: Vec::new(),
            active: None,
        };

        assert_eq!(decode(stored).split, Some(MAX_SPLIT));
    }

    #[test]
    fn more_than_two_stored_panes_are_ignored() {
        let pane = || StoredPane {
            tabs: vec![StoredTab {
                target: StoredTarget::Resource("Pods".to_string()),
                namespace: None,
            }],
            active: Some(0),
        };
        let stored = StoredWorkspace {
            version: SESSION_VERSION,
            panes: vec![pane(), pane(), pane()],
            focused: Some(2),
            split: Some(0.5),
            tabs: Vec::new(),
            active: None,
        };

        let decoded = decode(stored);

        assert_eq!(decoded.panes.len(), 2);
        assert_eq!(decoded.focused, Some(1));
    }

    #[test]
    fn empty_sessions_have_no_active_tab() {
        let decoded = decode(StoredWorkspace {
            version: SESSION_VERSION,
            panes: Vec::new(),
            focused: Some(3),
            split: Some(0.5),
            tabs: Vec::new(),
            active: Some(3),
        });

        assert!(decoded.panes.is_empty());
        assert_eq!(decoded.focused, None);
        assert_eq!(decoded.split, None);
    }

    #[test]
    fn version_two_files_do_not_write_legacy_fields() {
        let session = WorkspaceSession {
            panes: vec![SessionPane {
                tabs: vec![tab(resource(ResourceKind::Pods), NamespaceScope::All)],
                active: Some(0),
            }],
            focused: Some(0),
            split: None,
        };

        let stored = encode(&session);

        assert_eq!(stored.version, 2);
        assert!(stored.tabs.is_empty());
        assert_eq!(stored.active, None);
    }
}
