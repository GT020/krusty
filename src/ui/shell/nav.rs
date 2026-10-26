//! Navigation targets shared by the shell and sidebar.

use crate::models::ResourceKind;
use gpui_kit::assets::IconName;
use gpui_kit::component::sidebar::SidebarMenuItem;

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

    pub fn menu_item(self) -> SidebarMenuItem {
        SidebarMenuItem::new(self.label()).icon(self.icon())
    }
}
