//! ViewModel for the application shell: active navigation and namespace scope.

use crate::cluster::{self, ClusterState};
use crate::models::{NamespaceScope, ResourceKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectItem, SelectState};
use gpui_kit::{App, AppContext as _, Context, Entity, SharedString, Subscription, Window};

/// Navigation targets shared by the shell and sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavTarget {
    Resource(ResourceKind),
    Settings,
}

impl NavTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Resource(kind) => kind.label(),
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Resource(kind) => kind.icon(),
            Self::Settings => IconName::Settings,
        }
    }
}

/// One row in the namespace selector. `name` is empty for the "All namespaces"
/// sentinel; `value()` distinguishes the scope.
#[derive(Clone, Debug)]
pub struct NamespaceItem {
    name: String,
}

impl NamespaceItem {
    fn all() -> Self {
        Self {
            name: String::new(),
        }
    }

    fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    fn is_all(&self) -> bool {
        self.name.is_empty()
    }
}

impl SelectItem for NamespaceItem {
    type Value = String;

    fn title(&self) -> SharedString {
        if self.is_all() {
            "All namespaces".into()
        } else {
            self.name.clone().into()
        }
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

/// Build the select items: an "All namespaces" sentinel first, then namespaces.
fn namespace_items(namespaces: &[String]) -> SearchableVec<NamespaceItem> {
    let mut items = Vec::with_capacity(namespaces.len() + 1);
    items.push(NamespaceItem::all());
    items.extend(namespaces.iter().cloned().map(NamespaceItem::named));
    SearchableVec::new(items)
}

pub type NamespaceSelect = SelectState<SearchableVec<NamespaceItem>>;

/// Shell presentation state. The active navigation target and the namespace
/// dropdown live here rather than in the workspace view; the cluster global
/// remains the source of truth for the selected namespace scope.
pub struct ShellViewModel {
    selected: NavTarget,
    namespace_select: Entity<NamespaceSelect>,
    /// Last-synced values mirrored into `namespace_select`; used to avoid
    /// rebuilding the list on every update, which would discard the user's
    /// in-progress search filter.
    synced_namespaces: Vec<String>,
    synced_scope: NamespaceScope,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the select event
    namespace_sub: Subscription,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the cluster observer
    cluster_sub: Subscription,
}

impl ShellViewModel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let namespace_select = cx.new(|cx| {
            let state = cx.global::<ClusterState>();
            let items = namespace_items(state.namespaces());
            let selected = state.scope().namespace().map(str::to_string);
            let mut select = SelectState::new(items, None, window, cx).searchable(true);
            if let Some(selected) = selected {
                select.set_selected_value(&selected, window, cx);
            }
            select
        });
        let namespace_sub = cx.subscribe(
            &namespace_select,
            |_, _, event: &SelectEvent<SearchableVec<NamespaceItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                let scope = match value {
                    Some(namespace) if !namespace.is_empty() => {
                        NamespaceScope::One(namespace.clone())
                    }
                    _ => NamespaceScope::All,
                };
                cluster::set_namespace_scope(cx, scope);
            },
        );
        let cluster_sub = cx.observe_global_in::<ClusterState>(window, |this, window, cx| {
            this.sync_namespace_select(window, cx);
            cx.notify();
        });
        let state = cx.global::<ClusterState>();
        Self {
            selected: NavTarget::Resource(ResourceKind::Pods),
            namespace_select,
            synced_namespaces: state.namespaces().to_vec(),
            synced_scope: state.scope().clone(),
            namespace_sub,
            cluster_sub,
        }
    }

    pub fn selected(&self) -> NavTarget {
        self.selected
    }

    /// Switch the active navigation target. Returns whether it changed.
    pub fn select(&mut self, target: NavTarget, cx: &mut Context<Self>) -> bool {
        if target == self.selected {
            return false;
        }
        self.selected = target;
        cx.notify();
        true
    }

    pub fn namespace_select(&self) -> &Entity<NamespaceSelect> {
        &self.namespace_select
    }

    pub fn switch_context(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), &'static str> {
        cluster::switch_context(cx, id)?;
        cx.notify();
        Ok(())
    }

    /// Sync the namespace select from the cluster global, but only when the
    /// namespaces list or the scope actually changed. `set_items` rebuilds the
    /// delegate and would drop the in-progress search filter, so we avoid
    /// calling it on every update.
    fn sync_namespace_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = cx.global::<ClusterState>();
        let namespaces = state.namespaces().to_vec();
        let scope = state.scope().clone();

        let namespaces_changed = namespaces != self.synced_namespaces;
        let scope_changed = scope != self.synced_scope;
        if !namespaces_changed && !scope_changed {
            return;
        }

        self.synced_namespaces = namespaces.clone();
        self.synced_scope = scope.clone();

        self.namespace_select.update(cx, |select, cx| {
            if namespaces_changed {
                select.set_items(namespace_items(&namespaces), window, cx);
            }
            let value = scope.namespace().map(str::to_string).unwrap_or_default();
            if select.selected_value() != Some(&value) {
                select.set_selected_value(&value, window, cx);
            }
        });
    }

    pub fn context_label(&self, cx: &App) -> String {
        cx.global::<ClusterState>()
            .context()
            .unwrap_or("No context")
            .to_string()
    }
}
