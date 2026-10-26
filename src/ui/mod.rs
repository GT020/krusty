//! UI layer: declarative views + shell composition. No domain logic.

pub mod nav;
pub mod resource_list;
pub mod settings;
pub mod shell;
pub mod sidebar;
pub mod status_bar;
pub mod theme;

pub use shell::Workspace;
