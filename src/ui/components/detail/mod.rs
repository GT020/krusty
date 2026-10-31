//! Shared building blocks for resource detail panes.
//!
//! These are presentational leaves: they take plain values, own no resource
//! state, and never subscribe or read application globals. Resource-specific
//! detail views compose them.

mod conditions;
mod empty;
mod fields;
mod format;
mod frame;
mod key_values;
mod manifest;
mod menu;
mod text;
mod usage;

pub(crate) use conditions::{ConditionTone, ConditionView, condition_row};
pub(crate) use empty::empty_row;
pub(crate) use fields::{field, field_colored, field_with_id, value_pair};
pub(crate) use format::{age, format_bytes, format_cpu, format_sample_label, humanize};
pub(crate) use frame::{
    detail_header, detail_shell, error_body, loading_body, section, section_with_app,
};
pub(crate) use key_values::key_values;
pub(crate) use manifest::raw_manifest;
pub(crate) use menu::text_selection_context_menu;
pub(crate) use text::{SelectableTextStyle, selectable_text};
pub(crate) use usage::usage_body;
