//! Shared snapshot types for table views.
//!
//! The authoritative object store lives in `repository::store`. These types are
//! the small, UI-facing snapshot contract that every resource view consumes.

use std::sync::Arc;

use super::Item;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    pub namespace: String,
    pub name: String,
}

impl ObjectKey {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }
}

/// Immutable view of a store handed to the UI. Cloning a snapshot is O(1).
#[derive(Clone, Debug)]
pub struct ResourceSnapshot {
    items: Arc<[Arc<Item>]>,
    phase: SnapshotPhase,
    mode: WatchMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotPhase {
    Loading,
    Ready,
    Error(String),
}

/// The store always uses a live watch stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchMode {
    Watch,
}

impl ResourceSnapshot {
    pub fn loading() -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            phase: SnapshotPhase::Loading,
            mode: WatchMode::Watch,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            items: Arc::from(Vec::<Arc<Item>>::new()),
            phase: SnapshotPhase::Error(message.into()),
            mode: WatchMode::Watch,
        }
    }

    /// Build a snapshot from an already-filtered item set. Used by the shared
    /// store views so views can reuse the existing table/rendering contract.
    pub fn from_items(items: Arc<[Arc<Item>]>, phase: SnapshotPhase, mode: WatchMode) -> Self {
        Self { items, phase, mode }
    }

    pub fn items(&self) -> &[Arc<Item>] {
        &self.items
    }

    pub fn phase(&self) -> &SnapshotPhase {
        &self.phase
    }

    pub fn mode(&self) -> WatchMode {
        self.mode
    }
}
