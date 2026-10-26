//! UI layer: declarative views + shell composition. No domain logic.

pub mod resource_list;
pub mod shell;
pub mod sidebar;
pub mod status_bar;

pub use shell::Workspace;
