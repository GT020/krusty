//! Navigation targets shared by the shell and sidebar.

use gpui_kit::component::sidebar::SidebarMenuItem;

pub use crate::view_models::shell::NavTarget;

pub fn menu_item(target: NavTarget) -> SidebarMenuItem {
    SidebarMenuItem::new(target.label()).icon(target.icon())
}
