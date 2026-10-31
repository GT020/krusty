//! ViewModel for the application shell: the open tabs and the active tab.

use crate::cluster;
use crate::models::ResourceKind;
use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Context, Hsla};

/// Navigation targets shared by the shell and sidebar.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NavTarget {
    Resource(ResourceKind),
    Dashboard,
    Settings,
}

impl NavTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Resource(kind) => kind.label(),
            Self::Dashboard => "Dashboard",
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Resource(kind) => kind.icon(),
            Self::Dashboard => IconName::LayoutDashboard,
            Self::Settings => IconName::Settings,
        }
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
/// the same resource kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TabId(u64);

impl TabId {
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

/// The open tab list and active tab. Kept free of GPUI context so the
/// open-once rule and close ordering can be tested directly.
#[derive(Debug)]
struct TabSet {
    tabs: Vec<TabInfo>,
    active: Option<TabId>,
    next_id: u64,
}

impl TabSet {
    fn new(initial: NavTarget) -> Self {
        let id = TabId(0);
        Self {
            tabs: vec![TabInfo {
                id,
                target: initial,
                color: None,
                locked: false,
            }],
            active: Some(id),
            next_id: 1,
        }
    }

    fn active(&self) -> Option<TabId> {
        self.active
    }

    fn active_tab(&self) -> Option<TabInfo> {
        self.active.and_then(|id| self.tab(id))
    }

    fn tab(&self, id: TabId) -> Option<TabInfo> {
        self.tabs.iter().find(|tab| tab.id == id).copied()
    }

    fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    fn activate(&mut self, id: TabId) -> bool {
        if self.active == Some(id) || self.tab(id).is_none() {
            return false;
        }
        self.active = Some(id);
        true
    }

    /// Focus the current or first tab for `target`, opening one when none exists.
    fn focus_or_open(&mut self, target: NavTarget) -> TabId {
        if let Some(active) = self.active_tab()
            && active.target == target
        {
            return active.id;
        }
        match self.tabs.iter().find(|tab| tab.target == target) {
            Some(tab) => {
                self.active = Some(tab.id);
                tab.id
            }
            None => self.open_new(target),
        }
    }

    /// Open a new tab even when the same resource is already open.
    fn open_new(&mut self, target: NavTarget) -> TabId {
        let id = TabId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.tabs.push(TabInfo {
            id,
            target,
            color: None,
            locked: false,
        });
        self.active = Some(id);
        id
    }

    /// Close a tab. Locked tabs refuse to close. When it was active, the tab to
    /// its right becomes active, falling back to the one on its left. Returns
    /// whether the active tab changed.
    fn close(&mut self, id: TabId) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if self.tabs[index].locked {
            return false;
        }
        let was_active = self.active == Some(id);
        self.tabs.remove(index);
        if was_active {
            self.active = if self.tabs.is_empty() {
                None
            } else {
                Some(self.tabs[index.min(self.tabs.len() - 1)].id)
            };
        }
        was_active
    }

    /// Protect a tab from being closed, or allow closing it again.
    /// Returns whether anything changed.
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

    /// Assign or clear a tab's accent color. Returns whether anything changed.
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

/// Shell presentation state: which tabs are open and which one is shown.
pub struct ShellViewModel {
    tabs: TabSet,
}

impl ShellViewModel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            tabs: TabSet::new(NavTarget::Resource(ResourceKind::Pods)),
        }
    }

    /// The tab currently shown, or `None` when every tab has been closed.
    pub fn active(&self) -> Option<TabId> {
        self.tabs.active()
    }

    pub fn active_tab(&self) -> Option<TabInfo> {
        self.tabs.active_tab()
    }

    pub fn tab(&self, id: TabId) -> Option<TabInfo> {
        self.tabs.tab(id)
    }

    pub fn tabs(&self) -> &[TabInfo] {
        self.tabs.tabs()
    }

    /// Focus the first tab for `target`, opening a new one when needed.
    pub fn focus_or_open(&mut self, target: NavTarget, cx: &mut Context<Self>) -> TabId {
        let previous = self.tabs.active();
        let id = self.tabs.focus_or_open(target);
        if previous != Some(id) {
            cx.notify();
        }
        id
    }

    /// Always open a new tab, even when the same resource is already open.
    pub fn open_new(&mut self, target: NavTarget, cx: &mut Context<Self>) -> TabId {
        let id = self.tabs.open_new(target);
        cx.notify();
        id
    }

    pub fn activate(&mut self, id: TabId, cx: &mut Context<Self>) -> bool {
        let changed = self.tabs.activate(id);
        if changed {
            cx.notify();
        }
        changed
    }

    /// Close a tab. Returns whether the active tab changed.
    pub fn close(&mut self, id: TabId, cx: &mut Context<Self>) -> bool {
        let changed = self.tabs.close(id);
        if changed {
            cx.notify();
        }
        changed
    }

    /// Assign or clear a tab's accent color. Returns whether anything changed.
    pub fn set_color(
        &mut self,
        id: TabId,
        color: Option<TabColor>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.tabs.set_color(id, color);
        if changed {
            cx.notify();
        }
        changed
    }

    /// Protect a tab from being closed, or allow closing it again.
    /// Returns whether anything changed.
    pub fn set_locked(&mut self, id: TabId, locked: bool, cx: &mut Context<Self>) -> bool {
        let changed = self.tabs.set_locked(id, locked);
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

    #[test]
    fn opening_an_open_tab_focuses_it_without_duplicating() {
        let mut tabs = TabSet::new(pods());
        tabs.focus_or_open(deployments());
        tabs.focus_or_open(pods());

        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active(), Some(TabId(0)));
    }

    #[test]
    fn focusing_a_kind_keeps_the_active_tab_of_that_kind() {
        let mut tabs = TabSet::new(pods());
        let second = tabs.open_new(pods());
        let first = tabs.tabs()[0].id();

        assert_eq!(tabs.active(), Some(second));
        assert_eq!(tabs.focus_or_open(pods()), second);
        assert_eq!(tabs.active(), Some(second));

        let deployments_tab = tabs.focus_or_open(deployments());
        assert_eq!(tabs.active(), Some(deployments_tab));
        assert_eq!(tabs.active_tab().map(TabInfo::target), Some(deployments()));
        assert_eq!(tabs.focus_or_open(pods()), first);
        assert_eq!(tabs.active(), Some(first));
    }

    #[test]
    fn the_same_resource_can_be_opened_in_two_tabs_at_once() {
        let mut tabs = TabSet::new(pods());
        let first = tabs.active().unwrap();
        let second = tabs.open_new(pods());

        assert_ne!(first, second);
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active(), Some(second));
        assert!(tabs.tabs().iter().all(|tab| tab.target() == pods()));
        assert_eq!(tabs.tab(first).unwrap().id(), first);
    }

    #[test]
    fn closing_the_active_tab_activates_the_next_tab() {
        let mut tabs = TabSet::new(pods());
        tabs.focus_or_open(deployments());
        tabs.focus_or_open(services());
        let deployments = tabs
            .tabs()
            .iter()
            .find(|tab| tab.target() == deployments())
            .unwrap()
            .id();
        tabs.activate(deployments);

        assert!(tabs.close(deployments));
        assert_eq!(tabs.active_tab().map(TabInfo::target), Some(services()));
        assert_eq!(tabs.tabs().len(), 2);
    }

    #[test]
    fn closing_the_last_active_tab_activates_the_previous_tab() {
        let mut tabs = TabSet::new(pods());
        let deployments = tabs.focus_or_open(deployments());

        assert!(tabs.close(deployments));
        assert_eq!(tabs.active_tab().map(TabInfo::target), Some(pods()));
    }

    #[test]
    fn closing_an_inactive_tab_keeps_the_active_tab() {
        let mut tabs = TabSet::new(pods());
        let deployments = tabs.focus_or_open(deployments());
        let inactive = tabs.tabs()[0].id();

        assert!(!tabs.close(TabId(99)));
        assert!(!tabs.close(inactive));
        assert_eq!(tabs.active(), Some(deployments));
        assert_eq!(tabs.tabs().len(), 1);
    }

    #[test]
    fn closing_every_tab_leaves_no_active_tab() {
        let mut tabs = TabSet::new(pods());
        let id = tabs.active().unwrap();

        assert!(tabs.close(id));
        assert_eq!(tabs.active(), None);
        assert!(tabs.tabs().is_empty());
    }

    #[test]
    fn tab_colors_can_be_set_and_cleared() {
        let mut tabs = TabSet::new(pods());
        let id = tabs.active().unwrap();

        assert!(tabs.set_color(id, Some(TabColor::Green)));
        assert_eq!(tabs.tab(id).unwrap().color(), Some(TabColor::Green));
        assert!(!tabs.set_color(id, Some(TabColor::Green)));
        assert!(tabs.set_color(id, None));
        assert_eq!(tabs.tab(id).unwrap().color(), None);
        assert!(!tabs.set_color(TabId(99), Some(TabColor::Red)));
    }

    #[test]
    fn locked_tabs_refuse_to_close_until_unlocked() {
        let mut tabs = TabSet::new(pods());
        let id = tabs.focus_or_open(deployments());

        assert!(tabs.set_locked(id, true));
        assert!(tabs.tab(id).unwrap().locked());
        assert!(!tabs.close(id));
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.active(), Some(id));

        assert!(tabs.set_locked(id, false));
        assert!(tabs.close(id));
        assert_eq!(tabs.tabs().len(), 1);
        assert_eq!(tabs.active_tab().map(TabInfo::target), Some(pods()));
    }

    #[test]
    fn locking_an_unknown_tab_changes_nothing() {
        let mut tabs = TabSet::new(pods());

        assert!(!tabs.set_locked(TabId(99), true));
        assert!(!tabs.tabs()[0].locked());
    }
}
