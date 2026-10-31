//! ViewModel for the application shell: the open editor groups, their tabs,
//! and which group has focus.

use crate::cluster;
use crate::models::ResourceKind;
use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Context, Hsla};

/// Navigation targets shared by the shell and sidebar.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NavTarget {
    Overview,
    Resource(ResourceKind),
    Settings,
}

impl NavTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Resource(kind) => kind.label(),
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Overview => IconName::LayoutDashboard,
            Self::Resource(kind) => kind.icon(),
            Self::Settings => IconName::Settings,
        }
    }

    /// Whether this target is a singleton tab like Settings.
    pub fn is_singleton(self) -> bool {
        matches!(self, Self::Overview | Self::Settings)
    }
}

/// Accent colors a tab can be marked with. Resolved from the base theme
/// palette rather than fixed values so both light and dark themes stay legible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabColor {
    Red,
    Green,
    Blue,
    Yellow,
    Magenta,
    Cyan,
}

impl TabColor {
    pub const ALL: [Self; 6] = [
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Yellow,
        Self::Magenta,
        Self::Cyan,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Magenta => "Magenta",
            Self::Cyan => "Cyan",
        }
    }

    pub fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Red => cx.theme().red,
            Self::Green => cx.theme().green,
            Self::Blue => cx.theme().blue,
            Self::Yellow => cx.theme().yellow,
            Self::Magenta => cx.theme().magenta,
            Self::Cyan => cx.theme().cyan,
        }
    }
}

/// Identity of one open tab. Every tab gets its own id even when two tabs show
/// the same resource kind, and ids are unique across all editor groups.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TabId(u64);

impl TabId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Identity of one editor group. A layout has one or two groups.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PaneId(u64);

impl PaneId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// One open workspace tab. A target can appear in several tabs; each tab owns
/// its own view model and namespace scope.
#[derive(Clone, Copy, Debug)]
pub struct TabInfo {
    id: TabId,
    target: NavTarget,
    color: Option<TabColor>,
    locked: bool,
}

impl TabInfo {
    pub fn id(self) -> TabId {
        self.id
    }

    pub fn target(self) -> NavTarget {
        self.target
    }

    pub fn color(self) -> Option<TabColor> {
        self.color
    }

    /// Whether the user has protected this tab from being closed.
    pub fn locked(self) -> bool {
        self.locked
    }
}

/// A render-oriented snapshot of one editor group.
#[derive(Clone, Debug)]
pub struct PaneInfo {
    id: PaneId,
    active: Option<TabId>,
    tabs: Vec<TabInfo>,
}

impl PaneInfo {
    pub fn id(&self) -> PaneId {
        self.id
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }
}

/// Result of closing a tab. The workspace uses this to clean up retained
/// entities and to decide whether it needs to create the next active view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseOutcome {
    Closed,
    PaneCollapsed,
    RefusedLocked,
    NotFound,
}

#[derive(Debug)]
struct TabSet {
    tabs: Vec<TabInfo>,
    active: Option<TabId>,
}

impl TabSet {
    fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
        }
    }

    fn with_tabs(tabs: Vec<TabInfo>, active: Option<TabId>) -> Self {
        let active = active
            .filter(|id| tabs.iter().any(|tab| tab.id == *id))
            .or_else(|| tabs.first().map(|tab| tab.id));
        Self { tabs, active }
    }

    fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    fn active(&self) -> Option<TabId> {
        self.active
    }

    fn tab(&self, id: TabId) -> Option<TabInfo> {
        self.tabs.iter().find(|tab| tab.id == id).copied()
    }

    fn find_target(&self, target: NavTarget) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|tab| tab.target == target)
            .map(|tab| tab.id)
    }

    fn set_active(&mut self, id: TabId) -> bool {
        if self.active == Some(id) || self.tab(id).is_none() {
            return false;
        }
        self.active = Some(id);
        true
    }

    fn push(&mut self, tab: TabInfo) {
        self.active = Some(tab.id);
        self.tabs.push(tab);
    }

    /// Append tabs without changing the current active tab.
    fn extend(&mut self, tabs: impl IntoIterator<Item = TabInfo>) {
        for tab in tabs {
            self.tabs.push(tab);
        }
    }

    fn remove(&mut self, id: TabId) -> Option<TabInfo> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        let was_active = self.active == Some(id);
        let tab = self.tabs.remove(index);
        if was_active {
            self.active = if self.tabs.is_empty() {
                None
            } else {
                Some(self.tabs[index.min(self.tabs.len() - 1)].id)
            };
        }
        Some(tab)
    }

    fn set_locked(&mut self, id: TabId, locked: bool) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return false;
        };
        if tab.locked == locked {
            return false;
        }
        tab.locked = locked;
        true
    }

    fn set_color(&mut self, id: TabId, color: Option<TabColor>) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return false;
        };
        if tab.color == color {
            return false;
        }
        tab.color = color;
        true
    }
}

#[derive(Debug)]
struct PaneState {
    id: PaneId,
    tabs: TabSet,
}

#[derive(Debug)]
struct WorkspaceLayout {
    panes: Vec<PaneState>,
    focused: Option<PaneId>,
    next_tab: u64,
    next_pane: u64,
}

impl WorkspaceLayout {
    fn with_target(initial: NavTarget) -> Self {
        let pane_id = PaneId(0);
        let tab_id = TabId(0);
        let mut tabs = TabSet::new();
        tabs.push(TabInfo {
            id: tab_id,
            target: initial,
            color: None,
            locked: false,
        });
        Self {
            panes: vec![PaneState { id: pane_id, tabs }],
            focused: Some(pane_id),
            next_tab: 1,
            next_pane: 1,
        }
    }

    /// Build the layout from restored targets. `panes` contains each group's
    /// target list and its active index; empty groups are skipped.
    fn from_session(panes: Vec<(Vec<NavTarget>, Option<usize>)>, focused: usize) -> Self {
        let mut next_tab = 0u64;
        let mut next_pane = 0u64;
        let mut restored = Vec::new();
        for (targets, active) in panes.into_iter().take(2) {
            if targets.is_empty() {
                continue;
            }
            let pane_id = PaneId(next_pane);
            next_pane += 1;
            let tabs = targets
                .into_iter()
                .map(|target| {
                    let id = TabId(next_tab);
                    next_tab += 1;
                    TabInfo {
                        id,
                        target,
                        color: None,
                        locked: false,
                    }
                })
                .collect::<Vec<_>>();
            let active_index = active.unwrap_or(0).min(tabs.len() - 1);
            let active = tabs.get(active_index).map(|tab| tab.id());
            restored.push(PaneState {
                id: pane_id,
                tabs: TabSet::with_tabs(tabs, active),
            });
        }

        if restored.is_empty() {
            return Self::with_target(NavTarget::Resource(ResourceKind::Pods));
        }

        let focused = restored
            .get(focused.min(restored.len() - 1))
            .map(|pane| pane.id);
        Self {
            panes: restored,
            focused,
            next_tab,
            next_pane,
        }
    }

    fn allocate_tab(&mut self) -> TabId {
        let id = TabId(self.next_tab);
        self.next_tab = self.next_tab.wrapping_add(1);
        id
    }

    fn allocate_pane(&mut self) -> PaneId {
        let id = PaneId(self.next_pane);
        self.next_pane = self.next_pane.wrapping_add(1);
        id
    }

    fn focused_index(&self) -> Option<usize> {
        self.focused
            .and_then(|id| self.panes.iter().position(|pane| pane.id == id))
    }

    fn active(&self) -> Option<TabId> {
        self.focused_index()
            .and_then(|index| self.panes[index].tabs.active())
    }

    fn active_tab(&self) -> Option<TabInfo> {
        self.active().and_then(|id| self.tab(id))
    }

    fn tab(&self, id: TabId) -> Option<TabInfo> {
        self.panes.iter().find_map(|pane| pane.tabs.tab(id))
    }

    fn pane_index(&self, id: PaneId) -> Option<usize> {
        self.panes.iter().position(|pane| pane.id == id)
    }

    fn pane_of(&self, tab: TabId) -> Option<PaneId> {
        self.panes
            .iter()
            .find(|pane| pane.tabs.tab(tab).is_some())
            .map(|pane| pane.id)
    }

    fn find_target(&self, target: NavTarget) -> Option<(usize, TabId)> {
        self.panes
            .iter()
            .enumerate()
            .find_map(|(index, pane)| pane.tabs.find_target(target).map(|id| (index, id)))
    }

    fn ensure_focused_index(&mut self) -> usize {
        if self.panes.is_empty() {
            let id = self.allocate_pane();
            self.panes.push(PaneState {
                id,
                tabs: TabSet::new(),
            });
            self.focused = Some(id);
            return 0;
        }
        if let Some(index) = self.focused_index() {
            return index;
        }
        self.focused = Some(self.panes[0].id);
        0
    }

    fn open_new_in_index(&mut self, index: usize, target: NavTarget) -> TabId {
        let id = self.allocate_tab();
        self.panes[index].tabs.push(TabInfo {
            id,
            target,
            color: None,
            locked: false,
        });
        self.focused = Some(self.panes[index].id);
        id
    }

    /// Focus an existing tab for `target` anywhere in the workspace.
    fn focus_existing(&mut self, target: NavTarget) -> Option<TabId> {
        let (index, id) = self.find_target(target)?;
        self.panes[index].tabs.set_active(id);
        self.focused = Some(self.panes[index].id);
        Some(id)
    }

    fn open_new(&mut self, target: NavTarget) -> TabId {
        if target.is_singleton()
            && let Some(id) = self.focus_existing(target)
        {
            return id;
        }
        let index = self.ensure_focused_index();
        self.open_new_in_index(index, target)
    }

    fn focus_or_open(&mut self, target: NavTarget) -> TabId {
        if target.is_singleton()
            && let Some(id) = self.focus_existing(target)
        {
            return id;
        }
        let index = self.ensure_focused_index();
        if let Some(active) = self.panes[index].tabs.active()
            && self.panes[index]
                .tabs
                .tab(active)
                .is_some_and(|tab| tab.target == target)
        {
            return active;
        }
        if let Some(id) = self.panes[index].tabs.find_target(target) {
            self.panes[index].tabs.set_active(id);
            return id;
        }
        self.open_new_in_index(index, target)
    }

    fn open_in(&mut self, pane: PaneId, target: NavTarget, new_tab: bool) -> Option<TabId> {
        if target.is_singleton()
            && let Some(id) = self.focus_existing(target)
        {
            return Some(id);
        }
        let index = self.pane_index(pane).or_else(|| self.focused_index())?;
        self.focused = Some(self.panes[index].id);
        if new_tab {
            return Some(self.open_new_in_index(index, target));
        }
        if let Some(active) = self.panes[index].tabs.active()
            && self.panes[index]
                .tabs
                .tab(active)
                .is_some_and(|tab| tab.target == target)
        {
            return Some(active);
        }
        if let Some(id) = self.panes[index].tabs.find_target(target) {
            self.panes[index].tabs.set_active(id);
            return Some(id);
        }
        Some(self.open_new_in_index(index, target))
    }

    fn split_with_active(&mut self) -> Option<TabId> {
        if self.panes.len() >= 2 {
            return None;
        }
        let source_index = self.focused_index()?;
        let active = self.panes[source_index].tabs.active()?;
        let target = self.panes[source_index].tabs.tab(active)?.target();
        if target.is_singleton() {
            return None;
        }

        let pane_id = self.allocate_pane();
        let tab_id = self.allocate_tab();
        let mut tabs = TabSet::new();
        tabs.push(TabInfo {
            id: tab_id,
            target,
            color: None,
            locked: false,
        });
        self.panes.push(PaneState { id: pane_id, tabs });
        self.focused = Some(pane_id);
        Some(tab_id)
    }

    /// Open a target in the other group, creating the second group when the
    /// workspace is not split yet. Singleton targets fall back to focus/open.
    fn open_to_side(&mut self, target: NavTarget) -> TabId {
        if target.is_singleton() {
            return self.focus_or_open(target);
        }
        if self.panes.len() < 2 {
            let pane_id = self.allocate_pane();
            self.panes.push(PaneState {
                id: pane_id,
                tabs: TabSet::new(),
            });
            let index = self.panes.len() - 1;
            let id = self.open_new_in_index(index, target);
            return id;
        }

        let index = self.focused_index().unwrap_or(0);
        let other = 1 - index.min(1);
        if let Some(id) = self.panes[other].tabs.find_target(target) {
            self.panes[other].tabs.set_active(id);
            self.focused = Some(self.panes[other].id);
            return id;
        }
        let id = self.allocate_tab();
        self.panes[other].tabs.push(TabInfo {
            id,
            target,
            color: None,
            locked: false,
        });
        self.focused = Some(self.panes[other].id);
        id
    }

    fn unsplit(&mut self) -> bool {
        if self.panes.len() < 2 {
            return false;
        }
        let right = self.panes.remove(1);
        let focused_right = self.focused == Some(right.id);
        let left = &mut self.panes[0];

        let preferred = if focused_right {
            right.tabs.active()
        } else {
            left.tabs.active()
        };
        let fallback = left
            .tabs
            .active()
            .or_else(|| right.tabs.active())
            .or_else(|| right.tabs.tabs().first().map(|tab| tab.id));

        left.tabs.extend(right.tabs.tabs().iter().copied());
        if let Some(id) = preferred.or(fallback) {
            left.tabs.set_active(id);
        }
        self.focused = Some(left.id);
        true
    }

    fn move_tab_to_other(&mut self, id: TabId) -> Option<PaneId> {
        if self.panes.len() < 2 {
            return None;
        }
        let source_index = self
            .panes
            .iter()
            .position(|pane| pane.tabs.tab(id).is_some())?;
        let destination_index = 1 - source_index.min(1);
        let destination_id = self.panes[destination_index].id;
        let tab = self.panes[source_index].tabs.remove(id)?;
        self.panes[destination_index].tabs.push(tab);
        if self.panes[source_index].tabs.tabs().is_empty() {
            self.panes.remove(source_index);
        }
        self.focused = Some(destination_id);
        Some(destination_id)
    }

    fn close(&mut self, id: TabId) -> CloseOutcome {
        let Some(index) = self
            .panes
            .iter()
            .position(|pane| pane.tabs.tab(id).is_some())
        else {
            return CloseOutcome::NotFound;
        };
        if self.panes[index].tabs.tab(id).is_some_and(TabInfo::locked) {
            return CloseOutcome::RefusedLocked;
        }
        let pane_id = self.panes[index].id;
        self.panes[index].tabs.remove(id);
        if self.panes[index].tabs.tabs().is_empty() {
            self.panes.remove(index);
            if self.panes.is_empty() {
                self.focused = None;
            } else if self.focused == Some(pane_id) {
                self.focused = Some(self.panes[0].id);
            }
            return CloseOutcome::PaneCollapsed;
        }
        CloseOutcome::Closed
    }

    fn activate(&mut self, id: TabId) -> bool {
        let Some(index) = self
            .panes
            .iter()
            .position(|pane| pane.tabs.tab(id).is_some())
        else {
            return false;
        };
        let active_changed = self.panes[index].tabs.set_active(id);
        let focus_changed = self.focused != Some(self.panes[index].id);
        self.focused = Some(self.panes[index].id);
        active_changed || focus_changed
    }

    fn focus_pane(&mut self, id: PaneId) -> bool {
        if self.focused == Some(id) || self.pane_index(id).is_none() {
            return false;
        }
        self.focused = Some(id);
        true
    }

    fn set_locked(&mut self, id: TabId, locked: bool) -> bool {
        self.panes
            .iter_mut()
            .any(|pane| pane.tabs.set_locked(id, locked))
    }

    fn set_color(&mut self, id: TabId, color: Option<TabColor>) -> bool {
        self.panes
            .iter_mut()
            .any(|pane| pane.tabs.set_color(id, color))
    }

    fn all_tabs(&self) -> Vec<(PaneId, TabInfo)> {
        self.panes
            .iter()
            .flat_map(|pane| pane.tabs.tabs().iter().map(move |tab| (pane.id, *tab)))
            .collect()
    }

    fn panes(&self) -> Vec<PaneInfo> {
        self.panes
            .iter()
            .map(|pane| PaneInfo {
                id: pane.id,
                active: pane.tabs.active(),
                tabs: pane.tabs.tabs().to_vec(),
            })
            .collect()
    }
}

/// Shell presentation state: the editor groups, their tabs, and which group
/// has focus. Kept free of GPUI context so the layout rules can be tested
/// directly.
pub struct ShellViewModel {
    layout: WorkspaceLayout,
}

impl ShellViewModel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            layout: WorkspaceLayout::with_target(NavTarget::Resource(ResourceKind::Pods)),
        }
    }

    /// Restore the shell from saved pane targets and active indices.
    pub fn from_session(
        panes: Vec<(Vec<NavTarget>, Option<usize>)>,
        focused: usize,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            layout: WorkspaceLayout::from_session(panes, focused),
        }
    }

    /// The tab currently shown in the focused pane, or `None` when every tab
    /// has been closed.
    pub fn active(&self) -> Option<TabId> {
        self.layout.active()
    }

    pub fn active_tab(&self) -> Option<TabInfo> {
        self.layout.active_tab()
    }

    pub fn focused_pane(&self) -> Option<PaneId> {
        self.layout.focused
    }

    pub fn is_split(&self) -> bool {
        self.layout.panes.len() == 2
    }

    pub fn pane(&self, id: PaneId) -> Option<PaneInfo> {
        self.layout
            .panes
            .iter()
            .find(|pane| pane.id == id)
            .map(|pane| PaneInfo {
                id: pane.id,
                active: pane.tabs.active(),
                tabs: pane.tabs.tabs().to_vec(),
            })
    }

    pub fn panes(&self) -> Vec<PaneInfo> {
        self.layout.panes()
    }

    pub fn tab(&self, id: TabId) -> Option<TabInfo> {
        self.layout.panes.iter().find_map(|pane| pane.tabs.tab(id))
    }

    pub fn all_tabs(&self) -> Vec<(PaneId, TabInfo)> {
        self.layout.all_tabs()
    }

    pub fn pane_of(&self, id: TabId) -> Option<PaneId> {
        self.layout.pane_of(id)
    }

    /// Focus the first tab for `target` in the focused pane, opening a new one
    /// when needed.
    pub fn focus_or_open(&mut self, target: NavTarget, cx: &mut Context<Self>) -> TabId {
        let id = self.layout.focus_or_open(target);
        cx.notify();
        id
    }

    /// Always open a new tab in the focused pane, except for singleton targets.
    pub fn open_new(&mut self, target: NavTarget, cx: &mut Context<Self>) -> TabId {
        let id = self.layout.open_new(target);
        cx.notify();
        id
    }

    /// Open a target in a specific group.
    pub fn open_in(
        &mut self,
        pane: PaneId,
        target: NavTarget,
        new_tab: bool,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let id = self.layout.open_in(pane, target, new_tab);
        if id.is_some() {
            cx.notify();
        }
        id
    }

    /// Split the editor area and duplicate the focused pane's active target
    /// into the new right group.
    pub fn split_with_active(&mut self, cx: &mut Context<Self>) -> Option<TabId> {
        let id = self.layout.split_with_active();
        if id.is_some() {
            cx.notify();
        }
        id
    }

    /// Open a target in the other group, creating it first when unsplit.
    pub fn open_to_side(&mut self, target: NavTarget, cx: &mut Context<Self>) -> TabId {
        let id = self.layout.open_to_side(target);
        cx.notify();
        id
    }

    /// Merge the right group's tabs into the left group without closing tabs.
    pub fn unsplit(&mut self, cx: &mut Context<Self>) -> bool {
        let changed = self.layout.unsplit();
        if changed {
            cx.notify();
        }
        changed
    }

    /// Move a tab into the other group. Returns the destination group id.
    pub fn move_tab_to_other_pane(&mut self, id: TabId, cx: &mut Context<Self>) -> Option<PaneId> {
        let pane = self.layout.move_tab_to_other(id);
        if pane.is_some() {
            cx.notify();
        }
        pane
    }

    pub fn activate(&mut self, id: TabId, cx: &mut Context<Self>) -> bool {
        let changed = self.layout.activate(id);
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn focus_pane(&mut self, id: PaneId, cx: &mut Context<Self>) -> bool {
        let changed = self.layout.focus_pane(id);
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn focus_next_pane(&mut self, cx: &mut Context<Self>) -> bool {
        if self.layout.panes.len() < 2 {
            return false;
        }
        let index = self.layout.focused_index().unwrap_or(0);
        let next = self.layout.panes[1 - index.min(1)].id;
        self.focus_pane(next, cx)
    }

    /// Close a tab. Locked tabs refuse to close.
    pub fn close(&mut self, id: TabId, cx: &mut Context<Self>) -> CloseOutcome {
        let outcome = self.layout.close(id);
        if matches!(outcome, CloseOutcome::Closed | CloseOutcome::PaneCollapsed) {
            cx.notify();
        }
        outcome
    }

    pub fn set_color(
        &mut self,
        id: TabId,
        color: Option<TabColor>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.layout.set_color(id, color);
        if changed {
            cx.notify();
        }
        changed
    }

    /// Protect a tab from being closed, or allow closing it again.
    pub fn set_locked(&mut self, id: TabId, locked: bool, cx: &mut Context<Self>) -> bool {
        let changed = self.layout.set_locked(id, locked);
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn switch_context(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), &'static str> {
        cluster::switch_context(cx, id)?;
        cx.notify();
        Ok(())
    }

    pub fn context_label(&self, cx: &App) -> String {
        cx.global::<cluster::ClusterState>()
            .context()
            .unwrap_or("No context")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pods() -> NavTarget {
        NavTarget::Resource(ResourceKind::Pods)
    }

    fn deployments() -> NavTarget {
        NavTarget::Resource(ResourceKind::Deployments)
    }

    fn services() -> NavTarget {
        NavTarget::Resource(ResourceKind::Services)
    }

    fn layout() -> WorkspaceLayout {
        WorkspaceLayout::with_target(pods())
    }

    #[test]
    fn opening_an_open_tab_focuses_it_without_duplicating() {
        let mut layout = layout();
        layout.focus_or_open(deployments());
        layout.focus_or_open(pods());

        assert_eq!(layout.panes[0].tabs.tabs().len(), 2);
        assert_eq!(layout.active(), Some(TabId(0)));
    }

    #[test]
    fn focusing_a_kind_keeps_the_active_tab_of_that_kind() {
        let mut layout = layout();
        let second = layout.open_new(pods());
        let first = layout.panes[0].tabs.tabs()[0].id();

        assert_eq!(layout.active(), Some(second));
        assert_eq!(layout.focus_or_open(pods()), second);
        assert_eq!(layout.active(), Some(second));

        let deployments_tab = layout.focus_or_open(deployments());
        assert_eq!(layout.active(), Some(deployments_tab));
        assert_eq!(
            layout.active_tab().map(TabInfo::target),
            Some(deployments())
        );
        assert_eq!(layout.focus_or_open(pods()), first);
        assert_eq!(layout.active(), Some(first));
    }

    #[test]
    fn the_same_resource_can_be_opened_in_two_tabs_at_once() {
        let mut layout = layout();
        let first = layout.active().unwrap();
        let second = layout.open_new(pods());

        assert_ne!(first, second);
        assert_eq!(layout.panes[0].tabs.tabs().len(), 2);
        assert_eq!(layout.active(), Some(second));
        assert!(
            layout.panes[0]
                .tabs
                .tabs()
                .iter()
                .all(|tab| tab.target() == pods())
        );
        assert_eq!(layout.panes[0].tabs.tab(first).unwrap().id(), first);
    }

    #[test]
    fn closing_the_active_tab_activates_the_next_tab() {
        let mut layout = layout();
        layout.focus_or_open(deployments());
        layout.focus_or_open(services());
        let deployments = layout.panes[0]
            .tabs
            .tabs()
            .iter()
            .find(|tab| tab.target() == deployments())
            .unwrap()
            .id();
        layout.activate(deployments);

        assert_eq!(layout.close(deployments), CloseOutcome::Closed);
        assert_eq!(layout.active_tab().map(TabInfo::target), Some(services()));
        assert_eq!(layout.panes[0].tabs.tabs().len(), 2);
    }

    #[test]
    fn closing_the_last_active_tab_activates_the_previous_tab() {
        let mut layout = layout();
        let deployments = layout.focus_or_open(deployments());

        assert_eq!(layout.close(deployments), CloseOutcome::Closed);
        assert_eq!(layout.active_tab().map(TabInfo::target), Some(pods()));
    }

    #[test]
    fn closing_an_inactive_tab_keeps_the_active_tab() {
        let mut layout = layout();
        let deployments = layout.focus_or_open(deployments());
        let inactive = layout.panes[0].tabs.tabs()[0].id();

        assert_eq!(layout.close(TabId(99)), CloseOutcome::NotFound);
        assert_eq!(layout.close(inactive), CloseOutcome::Closed);
        assert_eq!(layout.active(), Some(deployments));
        assert_eq!(layout.panes[0].tabs.tabs().len(), 1);
    }

    #[test]
    fn closing_every_tab_leaves_no_active_tab() {
        let mut layout = layout();
        let id = layout.active().unwrap();

        assert_eq!(layout.close(id), CloseOutcome::PaneCollapsed);
        assert_eq!(layout.active(), None);
        assert!(layout.panes.is_empty());
        assert_eq!(layout.focused, None);
    }

    #[test]
    fn tab_colors_can_be_set_and_cleared() {
        let mut layout = layout();
        let id = layout.active().unwrap();

        assert!(layout.set_color(id, Some(TabColor::Green)));
        assert_eq!(
            layout.panes[0].tabs.tab(id).unwrap().color(),
            Some(TabColor::Green)
        );
        assert!(!layout.set_color(id, Some(TabColor::Green)));
        assert!(layout.set_color(id, None));
        assert_eq!(layout.panes[0].tabs.tab(id).unwrap().color(), None);
        assert!(!layout.set_color(TabId(99), Some(TabColor::Red)));
    }

    #[test]
    fn locked_tabs_refuse_to_close_until_unlocked() {
        let mut layout = layout();
        let id = layout.focus_or_open(deployments());

        assert!(layout.set_locked(id, true));
        assert!(layout.panes[0].tabs.tab(id).unwrap().locked());
        assert_eq!(layout.close(id), CloseOutcome::RefusedLocked);
        assert_eq!(layout.panes[0].tabs.tabs().len(), 2);
        assert_eq!(layout.active(), Some(id));

        assert!(layout.set_locked(id, false));
        assert_eq!(layout.close(id), CloseOutcome::Closed);
        assert_eq!(layout.panes[0].tabs.tabs().len(), 1);
        assert_eq!(layout.active_tab().map(TabInfo::target), Some(pods()));
    }

    #[test]
    fn locking_an_unknown_tab_changes_nothing() {
        let mut layout = layout();

        assert!(!layout.set_locked(TabId(99), true));
        assert!(!layout.panes[0].tabs.tabs()[0].locked());
    }

    #[test]
    fn restoring_tabs_keeps_their_order_and_active_tab() {
        let layout = WorkspaceLayout::from_session(
            vec![(vec![pods(), deployments(), services()], Some(1))],
            0,
        );

        assert_eq!(layout.panes[0].tabs.tabs().len(), 3);
        assert_eq!(layout.panes[0].tabs.tabs()[0].target(), pods());
        assert_eq!(layout.panes[0].tabs.tabs()[1].target(), deployments());
        assert_eq!(layout.panes[0].tabs.tabs()[2].target(), services());
        assert_eq!(layout.active(), Some(TabId(1)));

        let mut layout = layout;
        assert_eq!(layout.open_new(pods()), TabId(3));
    }

    #[test]
    fn an_out_of_range_restored_active_tab_clamps_to_the_last() {
        let layout = WorkspaceLayout::from_session(vec![(vec![pods(), deployments()], Some(9))], 0);

        assert_eq!(layout.active(), Some(TabId(1)));
    }

    #[test]
    fn split_duplicates_the_active_target_in_a_second_pane() {
        let mut layout = layout();
        let source = layout.active().unwrap();

        let duplicate = layout.split_with_active().unwrap();

        assert!(duplicate != source);
        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.active(), Some(duplicate));
        assert_eq!(layout.focused, Some(layout.panes[1].id));
        assert_eq!(
            layout.panes[1].tabs.tab(duplicate).unwrap().target(),
            pods()
        );
        assert_eq!(layout.pane_of(source), Some(layout.panes[0].id));
        assert_eq!(layout.pane_of(duplicate), Some(layout.panes[1].id));
    }

    #[test]
    fn split_is_refused_for_singletons_or_when_already_split() {
        let mut overview = WorkspaceLayout::with_target(NavTarget::Overview);
        assert_eq!(overview.split_with_active(), None);

        let mut layout = layout();
        layout.split_with_active();
        assert_eq!(layout.split_with_active(), None);
    }

    #[test]
    fn opening_uses_the_focused_pane_and_does_not_jump_to_the_other() {
        let mut layout = layout();
        let right = layout.split_with_active().unwrap();
        assert_eq!(layout.focused, Some(layout.panes[1].id));

        let deployments = layout.focus_or_open(deployments());
        assert_eq!(layout.pane_of(deployments), Some(layout.panes[1].id));
        assert_eq!(layout.pane_of(right), Some(layout.panes[1].id));

        assert!(layout.focus_pane(layout.panes[0].id));
        let services = layout.focus_or_open(services());
        assert_eq!(layout.pane_of(services), Some(layout.panes[0].id));
        assert_eq!(layout.active(), Some(services));
    }

    #[test]
    fn singleton_targets_focus_the_existing_tab_in_the_other_pane() {
        let mut layout = layout();
        layout.split_with_active();
        assert!(layout.focus_pane(layout.panes[0].id));
        let overview = layout.focus_or_open(NavTarget::Overview);
        assert_eq!(layout.focused, Some(layout.panes[0].id));

        assert!(layout.focus_pane(layout.panes[1].id));
        assert_eq!(layout.focus_or_open(NavTarget::Overview), overview);
        assert_eq!(layout.focused, Some(layout.panes[0].id));
        assert_eq!(
            layout
                .panes
                .iter()
                .map(|p| p.tabs.tabs().len())
                .sum::<usize>(),
            3
        );
    }

    #[test]
    fn closing_the_last_tab_in_the_right_pane_collapses_the_split() {
        let mut layout = layout();
        let duplicate = layout.split_with_active().unwrap();
        let left_active = layout.panes[0].tabs.active().unwrap();

        assert_eq!(layout.close(duplicate), CloseOutcome::PaneCollapsed);
        assert_eq!(layout.panes.len(), 1);
        assert_eq!(layout.focused, Some(layout.panes[0].id));
        assert_eq!(layout.active(), Some(left_active));
    }

    #[test]
    fn closing_the_last_tab_in_the_left_pane_promotes_the_right_pane() {
        let mut layout = layout();
        let left = layout.active().unwrap();
        let right = layout.split_with_active().unwrap();

        assert_eq!(layout.close(left), CloseOutcome::PaneCollapsed);
        assert_eq!(layout.panes.len(), 1);
        assert_eq!(layout.focused, Some(layout.panes[0].id));
        assert_eq!(layout.active(), Some(right));
        assert_eq!(layout.pane_of(right), Some(layout.panes[0].id));
    }

    #[test]
    fn unsplit_merges_tabs_and_keeps_the_focused_groups_active_tab() {
        let mut layout = layout();
        layout.focus_or_open(deployments());
        let right = layout.split_with_active().unwrap();
        let left_tabs = layout.panes[0].tabs.tabs().len();

        assert!(layout.unsplit());
        assert_eq!(layout.panes.len(), 1);
        assert_eq!(layout.panes[0].tabs.tabs().len(), left_tabs + 1);
        assert_eq!(layout.active(), Some(right));
        assert_eq!(layout.focused, Some(layout.panes[0].id));
    }

    #[test]
    fn moving_a_tab_to_the_other_pane_keeps_its_identity_and_collapses_an_emptied_pane() {
        let mut layout = layout();
        let right = layout.split_with_active().unwrap();
        let destination = layout.panes[1].id;
        let left_tab = layout.panes[0].tabs.tabs()[0].id();

        assert_eq!(layout.move_tab_to_other(left_tab), Some(destination));
        assert_eq!(layout.panes.len(), 1);
        assert_eq!(layout.pane_of(left_tab), Some(layout.panes[0].id));
        assert_eq!(layout.pane_of(right), Some(layout.panes[0].id));
        assert_eq!(layout.active(), Some(left_tab));
    }

    #[test]
    fn moving_a_tab_into_a_non_empty_pane_keeps_both_panes() {
        let mut layout = layout();
        layout.focus_or_open(deployments());
        let right = layout.split_with_active().unwrap();
        let pod = layout.panes[0]
            .tabs
            .tabs()
            .iter()
            .find(|tab| tab.target() == pods())
            .unwrap()
            .id();
        let right_pane = layout.panes[1].id;

        assert_eq!(layout.move_tab_to_other(pod), Some(right_pane));
        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.pane_of(pod), Some(right_pane));
        assert_eq!(layout.pane_of(right), Some(right_pane));
        assert_eq!(layout.panes[0].tabs.tabs().len(), 1);
        assert_eq!(layout.panes[1].tabs.tabs().len(), 2);
        assert_eq!(layout.active(), Some(pod));
    }

    #[test]
    fn open_to_side_focuses_an_existing_tab_in_the_other_pane() {
        let mut layout = layout();
        let deployments_tab = layout.open_to_side(deployments());
        assert!(layout.focus_pane(layout.panes[0].id));

        assert_eq!(layout.open_to_side(deployments()), deployments_tab);
        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.pane_of(deployments_tab), Some(layout.panes[1].id));
        assert_eq!(layout.focused, Some(layout.panes[1].id));
    }

    #[test]
    fn open_to_side_creates_a_second_pane_and_reuses_it() {
        let mut layout = layout();
        let deployments = layout.open_to_side(deployments());

        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.pane_of(deployments), Some(layout.panes[1].id));
        assert_eq!(layout.focused, Some(layout.panes[1].id));

        let services = layout.open_to_side(services());
        assert_eq!(layout.pane_of(services), Some(layout.panes[0].id));
        assert_eq!(layout.focused, Some(layout.panes[0].id));
    }

    #[test]
    fn focus_next_pane_alternates_groups() {
        let mut layout = layout();
        layout.split_with_active();
        let left = layout.panes[0].id;
        let right = layout.panes[1].id;
        assert_eq!(layout.focused, Some(right));

        assert!(layout.focus_pane(left));
        assert!(!layout.focus_pane(left));
        layout.focused = Some(right);
        assert_eq!(layout.focused_index(), Some(1));
    }

    #[test]
    fn tab_ids_are_global_across_panes_after_restore() {
        let layout = WorkspaceLayout::from_session(
            vec![
                (vec![pods(), deployments()], Some(1)),
                (vec![services(), pods()], Some(0)),
            ],
            1,
        );
        let ids = layout
            .all_tabs()
            .into_iter()
            .map(|(_, tab)| tab.id())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![TabId(0), TabId(1), TabId(2), TabId(3)]);
        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.focused, Some(PaneId(1)));
    }
}
