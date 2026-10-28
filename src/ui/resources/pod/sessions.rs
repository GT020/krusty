//! Bottom-panel sessions owned by one Pods tab: live logs and exec terminals.

use crate::ui::resources::pod::exec::PodExecView;
use crate::ui::resources::pod::logs::PodLogsView;
use crate::view_models::resources::pod::exec::PodExecViewModel;
use crate::view_models::resources::pod::logs::PodLogsViewModel;
use gpui_kit::{AnyElement, App, Entity, IntoElement, div};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionKind {
    Logs,
    Exec,
}

impl SessionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::Exec => "exec",
        }
    }
}

/// One pod-scoped logs or exec session. Logs and exec for the same pod are
/// distinct sessions, and each pod gets its own session tab.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SessionKey {
    kind: SessionKind,
    namespace: String,
    name: String,
}

impl SessionKey {
    pub fn logs(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: SessionKind::Logs,
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn exec(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: SessionKind::Exec,
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Stable identity for element ids.
    pub fn id(&self) -> String {
        format!("{}-{}-{}", self.kind.as_str(), self.namespace, self.name)
    }
}

pub enum PodSession {
    Logs {
        key: SessionKey,
        view: Entity<PodLogsView>,
        vm: Entity<PodLogsViewModel>,
    },
    Exec {
        key: SessionKey,
        view: Entity<PodExecView>,
        vm: Entity<PodExecViewModel>,
    },
}

impl PodSession {
    pub fn logs(key: SessionKey, view: Entity<PodLogsView>, vm: Entity<PodLogsViewModel>) -> Self {
        Self::Logs { key, view, vm }
    }

    pub fn exec(key: SessionKey, view: Entity<PodExecView>, vm: Entity<PodExecViewModel>) -> Self {
        Self::Exec { key, view, vm }
    }

    pub fn key(&self) -> &SessionKey {
        match self {
            Self::Logs { key, .. } | Self::Exec { key, .. } => key,
        }
    }

    pub fn title(&self) -> String {
        let key = self.key();
        let label = match self {
            Self::Logs { .. } => "Logs",
            Self::Exec { .. } => "Exec",
        };
        format!("{} · {}/{}", label, key.namespace, key.name)
    }

    pub fn element(&self) -> AnyElement {
        match self {
            Self::Logs { view, .. } => view.clone().into_any_element(),
            Self::Exec { view, .. } => view.clone().into_any_element(),
        }
    }

    /// Cancel the stream or shell before the session is dropped.
    pub fn shutdown(self, cx: &mut App) {
        match self {
            Self::Logs { vm, .. } => vm.update(cx, |vm, _| vm.shutdown()),
            Self::Exec { vm, .. } => vm.update(cx, |vm, _| vm.shutdown()),
        }
    }
}

/// The bottom panel's session tabs. Empty means the panel is hidden.
#[derive(Default)]
pub struct PodSessions {
    sessions: Vec<PodSession>,
    active: usize,
}

impl PodSessions {
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn active_index(&self) -> Option<usize> {
        (!self.sessions.is_empty()).then(|| self.active.min(self.sessions.len() - 1))
    }

    pub fn iter(&self) -> impl Iterator<Item = &PodSession> {
        self.sessions.iter()
    }

    /// Focus an existing session. Returns false when it is not open.
    pub fn focus(&mut self, key: &SessionKey) -> bool {
        match self
            .sessions
            .iter()
            .position(|session| session.key() == key)
        {
            Some(index) => {
                self.active = index;
                true
            }
            None => false,
        }
    }

    pub fn push(&mut self, session: PodSession) {
        self.sessions.push(session);
        self.active = self.sessions.len() - 1;
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
        }
    }

    /// Close a session and return it so its stream can be shut down.
    pub fn close(&mut self, key: &SessionKey) -> Option<PodSession> {
        let index = self
            .sessions
            .iter()
            .position(|session| session.key() == key)?;
        let session = self.sessions.remove(index);
        if self.sessions.is_empty() {
            self.active = 0;
        } else if index < self.active {
            self.active -= 1;
        } else if self.active >= self.sessions.len() {
            self.active = self.sessions.len() - 1;
        }
        Some(session)
    }

    pub fn active_element(&self) -> AnyElement {
        self.sessions
            .get(self.active)
            .map(PodSession::element)
            .unwrap_or_else(|| div().into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_and_exec_for_the_same_pod_are_distinct_sessions() {
        let logs = SessionKey::logs("default", "api");
        let exec = SessionKey::exec("default", "api");

        assert_ne!(logs, exec);
        assert_ne!(logs.id(), exec.id());
    }

    #[test]
    fn the_same_session_is_deduplicated() {
        assert_eq!(
            SessionKey::logs("default", "api"),
            SessionKey::logs("default", "api")
        );
    }

    #[test]
    fn different_pods_get_different_session_tabs() {
        assert_ne!(
            SessionKey::logs("default", "api"),
            SessionKey::logs("default", "worker")
        );
        assert_ne!(
            SessionKey::logs("default", "api"),
            SessionKey::logs("kube-system", "api")
        );
    }
}
