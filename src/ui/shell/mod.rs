//! Tabbed workspace shell: up to two side-by-side editor groups, each with its
//! own tabs, active tab, namespace scope, and live view.

pub mod commands;
pub mod nav;
pub mod session;
pub mod sidebar;
pub mod status_bar;

use crate::cluster::ClusterState;
use crate::models::kubectl::{self, KubectlAction, KubectlCommand};
use crate::models::{NamespaceScope, ResourceKind};
use crate::ui::resources::list::ResourceListView;
use crate::ui::resources::namespace_filter::NamespaceFilterEvent;
use crate::ui::resources::node::NodesView;
use crate::ui::resources::overview::OverviewView;
use crate::ui::resources::pod::PodsView;
use crate::ui::settings::custom_resources::CustomResourcesView;
use crate::ui::settings::render_settings;
use crate::ui::settings::theme::{self, AppearanceState};
use crate::ui::shell::commands::{FocusNextGroup, MoveTabToOtherGroup, SplitRight, Unsplit};
use crate::ui::shell::nav::NavTarget;
use crate::ui::shell::sidebar::render_sidebar;
use crate::ui::shell::status_bar::render_status_bar;
use crate::view_models::shell::{
    CloseOutcome, PaneId, PaneInfo, ShellViewModel, TabColor, TabId, TabInfo,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};
use std::collections::{BTreeMap, BTreeSet};

const MIN_SPLIT_VIEWPORT_WIDTH: f32 = 1080.;
const PANE_MIN_WIDTH: Pixels = px(360.);
const SPLIT_MIN_FRACTION: f32 = 0.2;
const SPLIT_MAX_FRACTION: f32 = 0.8;

/// One tab's retained view. The entity is created with the tab and dropped
/// when the tab closes, so two tabs of the same kind have separate state.
enum WorkspacePane {
    Overview(Entity<OverviewView>),
    Pods(Entity<PodsView>),
    Nodes(Entity<NodesView>),
    List(Entity<ResourceListView>),
}

impl WorkspacePane {
    fn set_active(&self, active: bool, cx: &mut Context<Workspace>) {
        match self {
            Self::Overview(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
            Self::Pods(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
            Self::Nodes(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
            Self::List(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
        }
    }

    fn set_width_share(&self, share: f32, cx: &mut Context<Workspace>) {
        match self {
            Self::Pods(view) => view.update(cx, |view, cx| view.set_width_share(share, cx)),
            Self::Nodes(view) => view.update(cx, |view, cx| view.set_width_share(share, cx)),
            Self::List(view) => view.update(cx, |view, cx| view.set_width_share(share, cx)),
            Self::Overview(_) => {}
        }
    }

    fn scope_label(&self, cx: &App) -> Option<String> {
        match self {
            Self::Overview(_) | Self::Nodes(_) => None,
            Self::Pods(view) => Some(view.read(cx).scope_label(cx)),
            Self::List(view) => Some(view.read(cx).scope_label(cx)),
        }
    }

    fn namespace_filter(
        &self,
        cx: &App,
    ) -> Option<Entity<crate::ui::resources::namespace_filter::NamespaceFilter>> {
        match self {
            Self::Overview(_) | Self::Nodes(_) => None,
            Self::Pods(view) => Some(view.read(cx).namespace_filter().clone()),
            Self::List(view) => Some(view.read(cx).namespace_filter().clone()),
        }
    }

    fn set_scope(&self, scope: NamespaceScope, window: &mut Window, cx: &mut Context<Workspace>) {
        match self {
            Self::Overview(_) | Self::Nodes(_) => {}
            Self::Pods(view) => view.update(cx, |view, cx| {
                view.set_scope(scope, window, cx);
            }),
            Self::List(view) => view.update(cx, |view, cx| {
                view.set_scope(scope, window, cx);
            }),
        }
    }

    fn open_detail(&self, namespace: &str, name: &str, cx: &mut Context<Workspace>) {
        match self {
            Self::Overview(_) => {}
            Self::Pods(view) => view.update(cx, |view, cx| {
                view.open_detail(namespace.to_string(), name.to_string(), cx);
            }),
            Self::Nodes(view) => view.update(cx, |view, cx| {
                view.open_detail(name.to_string(), cx);
            }),
            Self::List(view) => view.update(cx, |view, cx| {
                view.open_detail(namespace.to_string(), name.to_string(), cx);
            }),
        }
    }

    fn open_logs(
        &self,
        namespace: String,
        name: String,
        container: Option<String>,
        follow: bool,
        cx: &mut Context<Workspace>,
    ) {
        if let Self::Pods(view) = self {
            view.update(cx, |view, cx| {
                view.open_logs_for_pod(namespace, name, container, follow, cx);
            });
        }
    }

    fn element(&self) -> AnyElement {
        match self {
            Self::Overview(view) => view.clone().into_any_element(),
            Self::Pods(view) => view.clone().into_any_element(),
            Self::Nodes(view) => view.clone().into_any_element(),
            Self::List(view) => view.clone().into_any_element(),
        }
    }
}

pub struct Workspace {
    shell: Entity<ShellViewModel>,
    panes: BTreeMap<TabId, WorkspacePane>,
    pane_subs: BTreeMap<TabId, Subscription>,
    tab_scopes: BTreeMap<TabId, NamespaceScope>,
    split_state: Entity<ResizableState>,
    split_fraction: f32,
    pending_split_restore: bool,
    window_handle: AnyWindowHandle,
    command_input: Entity<InputState>,
    command_error: Option<String>,
    custom_resources: Entity<CustomResourcesView>,
    _shell_subscription: Subscription,
    _command_subscription: Subscription,
    _cluster_subscription: Subscription,
    _appearance_subscription: Subscription,
    _activation_subscription: Subscription,
    _preference_subscription: Subscription,
    _custom_resources_subscription: Subscription,
    _split_subscription: Subscription,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let started = std::time::Instant::now();
        tracing::info!("ui: workspace construction start");
        let session::WorkspaceSession {
            panes: restored_panes,
            focused: restored_focused,
            split: restored_split,
        } = session::load();
        let split_fraction = restored_split
            .map(|value| value.clamp(SPLIT_MIN_FRACTION, SPLIT_MAX_FRACTION))
            .unwrap_or(0.5);

        let shell = if restored_panes.is_empty() {
            cx.new(ShellViewModel::new)
        } else {
            let targets = restored_panes
                .iter()
                .map(|pane| {
                    (
                        pane.tabs.iter().map(|tab| tab.target).collect::<Vec<_>>(),
                        pane.active,
                    )
                })
                .collect::<Vec<_>>();
            let focused = restored_focused.unwrap_or(0);
            cx.new(|cx| ShellViewModel::from_session(targets, focused, cx))
        };

        // Restore each tab's namespace scope alongside the shell's tab ids.
        let mut tab_scopes = BTreeMap::new();
        let shell_panes = shell.read(cx).panes();
        for (session_pane, pane) in restored_panes.iter().zip(shell_panes.iter()) {
            for (session_tab, tab) in session_pane.tabs.iter().zip(pane.tabs().iter()) {
                tab_scopes.insert(tab.id(), session_tab.scope.clone());
            }
        }

        let shell_subscription = cx.observe(&shell, |_, _, cx| cx.notify());
        let cluster_subscription = cx.observe_global::<ClusterState>(|_, cx| cx.notify());
        let appearance_subscription = cx.observe_window_appearance(window, |_, window, cx| {
            theme::sync_system_appearance(window, cx);
        });
        let activation_subscription = cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                theme::sync_system_appearance(window, cx);
            }
        });
        let preference_subscription = cx.observe_global::<AppearanceState>(|_, cx| cx.notify());
        let command_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste a kubectl command, e.g. kubectl get pods -n default")
        });
        let command_subscription = cx.subscribe_in(
            &command_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.run_command(window, cx),
                InputEvent::Change => {
                    if this.command_error.take().is_some() {
                        cx.notify();
                    }
                }
                InputEvent::Focus | InputEvent::Blur => {}
            },
        );
        let custom_resources = cx.new(|_| CustomResourcesView::new());
        let custom_resources_subscription =
            cx.observe_global::<crate::ui::settings::custom_resources::CustomResourcesState>(
                |this, cx| {
                    let removed = this
                        .shell
                        .read(cx)
                        .all_tabs()
                        .into_iter()
                        .filter(|(_, tab)| {
                            matches!(
                                tab.target(),
                                NavTarget::Resource(ResourceKind::Custom(index))
                                    if crate::models::resources::catalog::custom_def(index).is_none()
                            )
                        })
                        .map(|(_, tab)| tab.id())
                        .collect::<Vec<_>>();
                    let window_handle = this.window_handle;
                    let workspace = cx.weak_entity();
                    for id in removed {
                        // A deleted definition cannot stay usable, so its tab
                        // closes even when the user protected it from closing.
                        this.shell.update(cx, |shell, cx| {
                            _ = shell.set_locked(id, false, cx);
                        });
                        let workspace = workspace.clone();
                        _ = window_handle.update(cx, move |_, window, cx| {
                            window.defer(cx, move |window, cx| {
                                if let Some(workspace) = workspace.upgrade() {
                                    workspace.update(cx, |this, cx| {
                                        this.close_tab(id, window, cx);
                                    });
                                }
                            });
                        });
                    }
                    cx.notify();
                },
            );

        let split_state = cx.new(|_| ResizableState::default());
        let split_subscription =
            cx.subscribe(&split_state, |this, _, event: &ResizablePanelEvent, cx| {
                if matches!(event, ResizablePanelEvent::Resized) {
                    this.split_resized(cx);
                }
            });

        let pending_split_restore = shell.read(cx).is_split();
        let mut this = Self {
            shell,
            panes: BTreeMap::new(),
            pane_subs: BTreeMap::new(),
            tab_scopes,
            split_state,
            split_fraction,
            pending_split_restore,
            window_handle: window.window_handle(),
            command_input,
            command_error: None,
            custom_resources,
            _shell_subscription: shell_subscription,
            _command_subscription: command_subscription,
            _cluster_subscription: cluster_subscription,
            _appearance_subscription: appearance_subscription,
            _activation_subscription: activation_subscription,
            _preference_subscription: preference_subscription,
            _custom_resources_subscription: custom_resources_subscription,
            _split_subscription: split_subscription,
        };
        this.ensure_active_panes(window, cx);
        this.sync_visibility(cx);
        this.sync_width_shares(cx);

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace construction complete"
        );
        this
    }

    /// Focus the target's existing tab in the focused group, or open a new one.
    /// `new_tab` always opens another tab, even when the same resource is
    /// already visible.
    pub fn select(
        &mut self,
        target: NavTarget,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_tab = new_tab && !target.is_singleton();
        let id = self.shell.update(cx, |shell, cx| {
            if new_tab {
                shell.open_new(target, cx)
            } else {
                shell.focus_or_open(target, cx)
            }
        });
        tracing::info!(to = ?id, kind = ?target, new_tab, "ui: open tab");
        self.finish_open(id, window, cx);
    }

    /// Open a target in the other editor group, creating it when needed.
    pub fn open_to_side(&mut self, target: NavTarget, window: &mut Window, cx: &mut Context<Self>) {
        let was_split = self.shell.read(cx).is_split();
        let id = self
            .shell
            .update(cx, |shell, cx| shell.open_to_side(target, cx));
        if !was_split && self.shell.read(cx).is_split() {
            self.pending_split_restore = true;
        }
        tracing::info!(to = ?id, kind = ?target, "ui: open tab to the side");
        self.finish_open(id, window, cx);
    }

    fn finish_open(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_pane(id, window, cx);
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.save_session(cx);
        cx.notify();
    }

    fn focus_tab(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.shell.update(cx, |shell, cx| shell.activate(id, cx)) {
            return;
        }
        self.ensure_pane(id, window, cx);
        self.sync_visibility(cx);
        self.apply_width_share(id, cx);
        self.save_session(cx);
        cx.notify();
    }

    fn focus_pane(&mut self, id: PaneId, cx: &mut Context<Self>) {
        if self.shell.update(cx, |shell, cx| shell.focus_pane(id, cx)) {
            self.save_session(cx);
        }
    }

    fn close_tab(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let outcome = self.shell.update(cx, |shell, cx| shell.close(id, cx));
        if !matches!(outcome, CloseOutcome::Closed | CloseOutcome::PaneCollapsed) {
            return;
        }
        tracing::info!(id = id.get(), ?outcome, "ui: close tab");
        self.panes.remove(&id);
        self.pane_subs.remove(&id);
        self.tab_scopes.remove(&id);
        self.ensure_active_panes(window, cx);
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.save_session(cx);
        cx.notify();
    }

    fn split_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source_scope = self
            .shell
            .read(cx)
            .active()
            .and_then(|id| self.tab_scopes.get(&id).cloned());
        let Some(id) = self
            .shell
            .update(cx, |shell, cx| shell.split_with_active(cx))
        else {
            return;
        };
        self.tab_scopes
            .insert(id, source_scope.unwrap_or(NamespaceScope::All));
        tracing::info!(id = id.get(), "ui: split editor");
        self.ensure_pane(id, window, cx);
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.pending_split_restore = true;
        self.save_session(cx);
        cx.notify();
    }

    fn unsplit(&mut self, cx: &mut Context<Self>) {
        if !self.shell.update(cx, |shell, cx| shell.unsplit(cx)) {
            return;
        }
        tracing::info!("ui: unsplit editor");
        self.pending_split_restore = false;
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.save_session(cx);
        cx.notify();
    }

    fn move_tab_to_other(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .shell
            .update(cx, |shell, cx| shell.move_tab_to_other_pane(id, cx))
            .is_none()
        {
            return;
        }
        tracing::info!(id = id.get(), "ui: move tab to the other group");
        self.ensure_active_panes(window, cx);
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.save_session(cx);
        cx.notify();
    }

    fn set_tab_color(&mut self, id: TabId, color: Option<TabColor>, cx: &mut Context<Self>) {
        if self
            .shell
            .update(cx, |shell, cx| shell.set_color(id, color, cx))
        {
            tracing::info!(id = id.get(), ?color, "ui: tab color changed");
        }
    }

    fn set_tab_locked(&mut self, id: TabId, locked: bool, cx: &mut Context<Self>) {
        if self
            .shell
            .update(cx, |shell, cx| shell.set_locked(id, locked, cx))
        {
            tracing::info!(id = id.get(), locked, "ui: tab lock changed");
        }
    }

    /// Parse and run the command currently in the top-bar input.
    fn run_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.command_input.read(cx).value().to_string();
        if input.trim().is_empty() {
            return;
        }
        match kubectl::parse(&input) {
            Ok(command) => {
                self.command_error = None;
                self.command_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.execute_command(command, window, cx);
            }
            Err(error) => {
                self.command_error = Some(error);
                cx.notify();
            }
        }
    }

    /// Turn a parsed command into a tab, detail pane, or log panel.
    fn execute_command(
        &mut self,
        command: KubectlCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let KubectlCommand { action, namespace } = command;
        let (kind, scope) = match &action {
            KubectlAction::Get { kind, .. } | KubectlAction::Describe { kind, .. } => {
                (*kind, namespace.clone())
            }
            KubectlAction::Logs { .. } => (ResourceKind::Pods, namespace.clone()),
        };
        let scope = if kind.is_namespaced() {
            scope
        } else {
            NamespaceScope::All
        };
        tracing::info!(?action, ?scope, "ui: kubectl command");
        let Some(id) = self.open_resource_target(NavTarget::Resource(kind), scope, window, cx)
        else {
            return;
        };
        let detail_namespace = if kind.is_namespaced() {
            namespace.namespace().unwrap_or("default").to_string()
        } else {
            String::new()
        };
        match action {
            KubectlAction::Get { .. } => {}
            KubectlAction::Describe { name, .. } => {
                if let Some(pane) = self.panes.get(&id) {
                    pane.open_detail(&detail_namespace, &name, cx);
                }
            }
            KubectlAction::Logs {
                pod,
                container,
                follow,
            } => {
                if let Some(pane) = self.panes.get(&id) {
                    pane.open_logs(detail_namespace, pod, container, follow, cx);
                }
            }
        }
    }

    /// Focus or open the tab for a resource and switch it to `scope`.
    fn open_resource_target(
        &mut self,
        target: NavTarget,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let NavTarget::Resource(kind) = target else {
            return None;
        };
        let scope = if kind.is_namespaced() {
            scope
        } else {
            NamespaceScope::All
        };
        let id = self
            .shell
            .update(cx, |shell, cx| shell.focus_or_open(target, cx));
        self.tab_scopes.insert(id, scope.clone());
        if let Some(pane) = self.panes.get(&id) {
            pane.set_scope(scope, window, cx);
        } else {
            self.ensure_pane(id, window, cx);
        }
        self.sync_visibility(cx);
        self.sync_width_shares(cx);
        self.save_session(cx);
        cx.notify();
        Some(id)
    }

    /// Create the view for an open tab when it does not exist yet.
    fn ensure_pane(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if self.panes.contains_key(&id) {
            return;
        }
        let Some(tab) = self.shell.read(cx).tab(id) else {
            return;
        };
        let scope = self
            .tab_scopes
            .entry(id)
            .or_insert(NamespaceScope::All)
            .clone();
        let pane = match tab.target() {
            NavTarget::Overview => {
                WorkspacePane::Overview(cx.new(|cx| OverviewView::new(window, cx)))
            }
            NavTarget::Resource(ResourceKind::Pods) => {
                WorkspacePane::Pods(cx.new(|cx| PodsView::new(scope, window, cx)))
            }
            NavTarget::Resource(ResourceKind::Nodes) => {
                WorkspacePane::Nodes(cx.new(|cx| NodesView::new(NamespaceScope::All, window, cx)))
            }
            NavTarget::Resource(kind) => {
                WorkspacePane::List(cx.new(|cx| ResourceListView::new(kind, scope, window, cx)))
            }
            NavTarget::Settings => return,
        };
        // Re-render the tab labels when this tab's namespace changes, and keep
        // the saved session in sync with the namespace the user picked.
        if let Some(filter) = pane.namespace_filter(cx) {
            let sub = cx.subscribe(&filter, move |this, _, event: &NamespaceFilterEvent, cx| {
                let NamespaceFilterEvent::Changed(scope) = event;
                this.tab_scopes.insert(id, scope.clone());
                this.save_session(cx);
                cx.notify();
            });
            self.pane_subs.insert(id, sub);
        }
        self.panes.insert(id, pane);
    }

    fn ensure_active_panes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self
            .shell
            .read(cx)
            .panes()
            .iter()
            .filter_map(PaneInfo::active)
            .collect::<Vec<_>>();
        for id in ids {
            self.ensure_pane(id, window, cx);
        }
    }

    /// Only the active tab of each visible group is live. Hidden tabs keep
    /// their retained views but stop receiving watch updates until shown again.
    fn sync_visibility(&mut self, cx: &mut Context<Self>) {
        let visible = self
            .shell
            .read(cx)
            .panes()
            .iter()
            .filter_map(PaneInfo::active)
            .collect::<BTreeSet<_>>();
        for (id, pane) in &self.panes {
            pane.set_active(visible.contains(id), cx);
        }
    }

    fn share_for_pane(&self, pane: PaneId, cx: &App) -> f32 {
        let shell = self.shell.read(cx);
        if !shell.is_split() {
            return 1.0;
        }
        match shell.panes().iter().position(|info| info.id() == pane) {
            Some(0) => self.split_fraction,
            Some(_) => (1.0 - self.split_fraction).max(SPLIT_MIN_FRACTION),
            None => 1.0,
        }
    }

    fn apply_width_share(&mut self, id: TabId, cx: &mut Context<Self>) {
        let Some(pane) = self.shell.read(cx).pane_of(id) else {
            return;
        };
        let share = self.share_for_pane(pane, cx);
        if let Some(view) = self.panes.get(&id) {
            view.set_width_share(share, cx);
        }
    }

    fn sync_width_shares(&mut self, cx: &mut Context<Self>) {
        let active = self
            .shell
            .read(cx)
            .panes()
            .iter()
            .filter_map(|pane| pane.active().map(|id| (id, pane.id())))
            .collect::<Vec<_>>();
        for (id, pane) in active {
            let share = self.share_for_pane(pane, cx);
            if let Some(view) = self.panes.get(&id) {
                view.set_width_share(share, cx);
            }
        }
    }

    fn split_resized(&mut self, cx: &mut Context<Self>) {
        let sizes = self.split_state.read(cx).sizes().clone();
        if sizes.len() != 2 {
            return;
        }
        let total = sizes[0] + sizes[1];
        if total <= px(0.) {
            return;
        }
        let fraction = (sizes[0] / total).clamp(SPLIT_MIN_FRACTION, SPLIT_MAX_FRACTION);
        if (fraction - self.split_fraction).abs() > 0.001 {
            self.split_fraction = fraction;
            self.sync_width_shares(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    /// Persist the open groups, their active tabs, focus, scopes, and ratio.
    fn save_session(&self, cx: &App) {
        let shell = self.shell.read(cx);
        let panes = shell
            .panes()
            .iter()
            .map(|pane| session::SessionPane {
                tabs: pane
                    .tabs()
                    .iter()
                    .map(|tab| session::SessionTab {
                        target: tab.target(),
                        scope: self
                            .tab_scopes
                            .get(&tab.id())
                            .cloned()
                            .unwrap_or(NamespaceScope::All),
                    })
                    .collect(),
                active: pane
                    .active()
                    .and_then(|id| pane.tabs().iter().position(|tab| tab.id() == id)),
            })
            .collect::<Vec<_>>();
        let focused = shell
            .focused_pane()
            .and_then(|id| shell.panes().iter().position(|pane| pane.id() == id));
        let split = shell.is_split().then_some(self.split_fraction);
        session::save(&session::WorkspaceSession {
            panes,
            focused,
            split,
        });
    }

    fn tab_bar(&self, pane: PaneInfo, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pane_id = pane.id();
        let tabs = pane.tabs().to_vec();
        let active = pane.active();
        let focused_pane = self.shell.read(cx).focused_pane() == Some(pane_id);
        let selected_index = active
            .and_then(|id| tabs.iter().position(|tab| tab.id() == id))
            .unwrap_or(0);
        let workspace = cx.entity();
        let is_split = self.shell.read(cx).is_split();

        let mut bar = TabBar::new(("workspace-tabs", pane_id.get()))
            .small()
            .menu(true)
            .max_width(px(240.))
            .selected_index(selected_index)
            .on_click(cx.listener(move |this, index, window, cx| {
                let pane = this.shell.read(cx).pane(pane_id);
                let id =
                    pane.and_then(|pane| pane.tabs().get(*index).map(|tab: &TabInfo| tab.id()));
                if let Some(id) = id {
                    this.focus_tab(id, window, cx);
                }
            }));

        for tab in &tabs {
            let id = tab.id();
            let target = tab.target();
            let color = tab.color();
            let locked = tab.locked();
            let lock_workspace = workspace.clone();
            let close_workspace = workspace.clone();
            let tab_label = self.tab_label(tab, cx);
            let lock_button = Button::new(SharedString::from(format!("lock-tab-{}", id.get())))
                .ghost()
                .xsmall()
                .icon(
                    Icon::new(if locked {
                        IconName::Lock
                    } else {
                        IconName::LockOpen
                    })
                    .text_color(if locked {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    }),
                )
                .tooltip(if locked {
                    "Allow closing"
                } else {
                    "Prevent closing"
                })
                .accessibility_label(if locked {
                    format!("Allow closing {} tab", tab_label)
                } else {
                    format!("Prevent closing {} tab", tab_label)
                })
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    lock_workspace.update(cx, |this, cx| this.set_tab_locked(id, !locked, cx));
                });
            let close_button = (!locked).then(|| {
                Button::new(SharedString::from(format!("close-tab-{}", id.get())))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .tooltip("Close tab")
                    .accessibility_label(format!("Close {} tab", tab_label))
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        close_workspace.update(cx, |this, cx| this.close_tab(id, window, cx));
                    })
            });
            bar = bar.child(
                Tab::new()
                    .label(tab_label)
                    .prefix(
                        div().pl(px(6.)).child(
                            Icon::new(target.icon())
                                .when_some(color.map(|color| color.color(cx)), |this, color| {
                                    this.text_color(color)
                                }),
                        ),
                    )
                    .suffix(
                        h_flex()
                            .items_center()
                            .gap_0p5()
                            .child(lock_button)
                            .when_some(close_button, |this, close| this.child(close)),
                    ),
            );
        }

        let tab_actions = active.map(|id| {
            let target = tabs
                .iter()
                .find(|tab| tab.id() == id)
                .map(|tab| tab.target())
                .unwrap_or(NavTarget::Settings);
            let new_tab_workspace = workspace.clone();
            let color_workspace = workspace.clone();
            let move_workspace = workspace.clone();
            let split_workspace = workspace.clone();
            let new_tab_label = format!("Open another {} tab", target.label());
            let new_tab_button = (!target.is_singleton()).then(|| {
                Button::new(SharedString::from(format!("new-tab-{}", pane_id.get())))
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip(new_tab_label.clone())
                    .accessibility_label(new_tab_label)
                    .on_click(move |_, window, cx| {
                        new_tab_workspace.update(cx, |this, cx| {
                            let id = this
                                .shell
                                .update(cx, |shell, cx| shell.open_in(pane_id, target, true, cx));
                            if let Some(id) = id {
                                this.finish_open(id, window, cx);
                            }
                        });
                    })
            });
            let move_button = (is_split && !target.is_singleton()).then(|| {
                Button::new(SharedString::from(format!("move-tab-{}", pane_id.get())))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowRightLeft)
                    .tooltip("Move tab to the other group")
                    .accessibility_label("Move tab to the other group")
                    .on_click(move |_, window, cx| {
                        move_workspace.update(cx, |this, cx| {
                            this.move_tab_to_other(id, window, cx);
                        });
                    })
            });
            let split_button = if is_split {
                Button::new(SharedString::from(format!("unsplit-{}", pane_id.get())))
                    .ghost()
                    .xsmall()
                    .icon(IconName::PanelRightClose)
                    .tooltip("Unsplit editor groups")
                    .accessibility_label("Unsplit editor groups")
                    .on_click(move |_, _window, cx| {
                        split_workspace.update(cx, |this, cx| {
                            this.unsplit(cx);
                        });
                    })
            } else {
                let can_split = !target.is_singleton()
                    && window.viewport_size().width.as_f32() >= MIN_SPLIT_VIEWPORT_WIDTH;
                Button::new(SharedString::from(format!("split-{}", pane_id.get())))
                    .ghost()
                    .xsmall()
                    .icon(IconName::PanelRight)
                    .tooltip("Split editor right")
                    .accessibility_label("Split editor right")
                    .disabled(!can_split)
                    .on_click(move |_, window, cx| {
                        split_workspace.update(cx, |this, cx| {
                            this.split_active(window, cx);
                        });
                    })
            };

            h_flex()
                .gap_1()
                .when_some(new_tab_button, |this, button| this.child(button))
                .when_some(move_button, |this, button| this.child(button))
                .child(
                    Button::new(SharedString::from(format!("tab-color-{}", pane_id.get())))
                        .ghost()
                        .small()
                        .icon(
                            Icon::new(IconName::Palette).when_some(
                                active
                                    .and_then(|id| tabs.iter().find(|tab| tab.id() == id))
                                    .and_then(|tab| tab.color())
                                    .map(|color| color.color(cx)),
                                |this, color| this.text_color(color),
                            ),
                        )
                        .tooltip("Tab color")
                        .accessibility_label("Tab color")
                        .dropdown_menu(move |menu, _, cx| {
                            let current = active
                                .and_then(|id| tabs.iter().find(|tab| tab.id() == id))
                                .and_then(|tab| tab.color());
                            tab_color_menu(menu, id, current, color_workspace.clone(), cx)
                        }),
                )
                .child(split_button)
        });

        bar.when_some(tab_actions, |bar, actions| bar.suffix(actions))
            .when(!focused_pane, |bar| bar.bg(cx.theme().muted))
            .into_any_element()
    }

    /// Show the tab's namespace in its label for namespaced resources.
    fn tab_label(&self, tab: &TabInfo, cx: &App) -> String {
        let target = tab.target();
        if let NavTarget::Resource(kind) = target
            && kind.is_namespaced()
            && let Some(scope) = self
                .panes
                .get(&tab.id())
                .and_then(|pane| pane.scope_label(cx))
                .or_else(|| {
                    self.tab_scopes
                        .get(&tab.id())
                        .filter(|scope| **scope != NamespaceScope::All)
                        .map(NamespaceScope::label)
                })
        {
            return format!("{} · {}", target.label(), scope);
        }
        target.label().to_string()
    }

    fn pane_content(&self, pane: &PaneInfo, cx: &Context<Self>) -> AnyElement {
        let Some(id) = pane.active() else {
            return render_empty_workspace(cx);
        };
        let Some(tab) = self.shell.read(cx).tab(id) else {
            return render_empty_workspace(cx);
        };
        match tab.target() {
            NavTarget::Settings => {
                render_settings(cx, self.custom_resources.clone()).into_any_element()
            }
            NavTarget::Overview | NavTarget::Resource(_) => self
                .panes
                .get(&id)
                .map(WorkspacePane::element)
                .unwrap_or_else(|| render_empty_workspace(cx)),
        }
    }

    fn render_pane(
        &self,
        pane: PaneInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pane_id = pane.id();
        let tab_bar = self.tab_bar(pane.clone(), window, cx);
        let content = self.pane_content(&pane, cx);
        let focus_workspace = cx.entity();
        v_flex()
            .id(("editor-pane", pane_id.get()))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                focus_workspace.update(cx, |this, cx| this.focus_pane(pane_id, cx));
            })
            .child(tab_bar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
            .into_any_element()
    }

    fn editor_area(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (panes, is_split) = {
            let shell = self.shell.read(cx);
            (shell.panes(), shell.is_split())
        };

        if panes.is_empty() {
            return render_empty_workspace(cx);
        }
        if !is_split {
            return self.render_pane(panes[0].clone(), window, cx);
        }

        let left = self.render_pane(panes[0].clone(), window, cx);
        let right = self.render_pane(panes[1].clone(), window, cx);
        h_resizable("workspace-split")
            .with_state(&self.split_state)
            .child(
                resizable_panel()
                    .size_range(PANE_MIN_WIDTH..Pixels::MAX)
                    .child(left),
            )
            .child(
                resizable_panel()
                    .size_range(PANE_MIN_WIDTH..Pixels::MAX)
                    .child(right),
            )
            .into_any_element()
    }
}

/// Menu items that assign or clear a tab's accent color.
fn tab_color_menu(
    mut menu: PopupMenu,
    id: TabId,
    current: Option<TabColor>,
    workspace: Entity<Workspace>,
    cx: &mut App,
) -> PopupMenu {
    let clear_workspace = workspace.clone();
    menu = menu.item(
        PopupMenuItem::new("No color")
            .checked(current.is_none())
            .on_click(move |_, _, cx| {
                clear_workspace.update(cx, |this, cx| this.set_tab_color(id, None, cx));
            }),
    );

    for color in TabColor::ALL {
        let item_workspace = workspace.clone();
        menu = menu.item(
            PopupMenuItem::new(color.label())
                .icon(Icon::new(IconName::Circle).text_color(color.color(cx)))
                .checked(current == Some(color))
                .on_click(move |_, _, cx| {
                    item_workspace.update(cx, |this, cx| this.set_tab_color(id, Some(color), cx));
                }),
        );
    }

    menu
}

fn render_empty_workspace(cx: &Context<Workspace>) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            Icon::new(IconName::LayoutPanelTop)
                .size_8()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child("No open views"),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Choose a resource in the sidebar to open a tab."),
        )
        .into_any_element()
}

fn render_context_selector(
    cx: &Context<Workspace>,
    shell: &Entity<ShellViewModel>,
) -> impl IntoElement {
    let label = shell.read(cx).context_label(cx);
    let shell = shell.clone();
    Button::new("context")
        .ghost()
        .small()
        .label(label)
        .icon(IconName::ChevronsUpDown)
        .dropdown_menu(move |mut menu, _, cx| {
            let state = cx.global::<ClusterState>();
            let contexts = state.contexts().to_vec();
            let active = state.active_id().map(str::to_string);
            if contexts.is_empty() {
                return menu.item(PopupMenuItem::new("No contexts found").disabled(true));
            }
            for context in &contexts {
                let id = context.id().to_string();
                let selected = active.as_deref() == Some(context.id());
                let shell = shell.clone();
                menu = menu.item(
                    PopupMenuItem::new(context.label().to_string())
                        .checked(selected)
                        .on_click(move |_, _, cx| {
                            if let Err(error) =
                                shell.update(cx, |shell, cx| shell.switch_context(&id, cx))
                            {
                                tracing::error!(
                                    context = %id,
                                    %error,
                                    "ui: switch context failed"
                                );
                            }
                        }),
                );
            }
            menu
        })
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = std::time::Instant::now();
        if self.pending_split_restore {
            self.pending_split_restore = false;
            if self.shell.read(cx).is_split() {
                let fraction = self.split_fraction;
                let state = self.split_state.clone();
                window.defer(cx, move |window, cx| {
                    let width = state.read(cx).container_size();
                    if width > px(0.) {
                        state.update(cx, |state, cx| {
                            state.resize_panel(0, px(width.as_f32() * fraction), window, cx);
                        });
                    }
                });
            }
        }
        let active_target = self.shell.read(cx).active_tab().map(TabInfo::target);
        tracing::trace!(panes = self.panes.len(), "ui: workspace render start");
        let sidebar = render_sidebar(active_target, cx.entity(), cx);
        let editor = self.editor_area(window, cx);

        let root = v_flex()
            .size_full()
            .key_context("Workspace")
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.split_active(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Unsplit, _window, cx| {
                this.unsplit(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusNextGroup, _window, cx| {
                if this.shell.update(cx, |shell, cx| shell.focus_next_pane(cx)) {
                    this.save_session(cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &MoveTabToOtherGroup, window, cx| {
                if let Some(id) = this.shell.read(cx).active() {
                    this.move_tab_to_other(id, window, cx);
                }
            }))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("krusty"),
                        )
                        .child(render_context_selector(cx, &self.shell))
                        .child(
                            h_flex().flex_1().min_w_0().justify_center().px_6().child(
                                div().w_full().max_w(px(520.)).child(
                                    Input::new(&self.command_input)
                                        .small()
                                        .cleanable(true)
                                        .prefix(
                                            Icon::new(IconName::Terminal)
                                                .small()
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .aria_label("kubectl command"),
                                ),
                            ),
                        )
                        .when_some(self.command_error.clone(), |this, error| {
                            this.child(
                                div()
                                    .max_w(px(260.))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_xs()
                                    .text_color(cx.theme().danger)
                                    .child(error),
                            )
                        }),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(
                        v_flex().flex_1().min_w_0().h_full().child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .child(editor),
                        ),
                    ),
            )
            .child(render_status_bar(cx));
        tracing::trace!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ui: workspace render end"
        );
        root
    }
}
