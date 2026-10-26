//! Repository for StorageClasses: subscribe to the live list/watch stream.

use crate::models::{NamespaceScope, ResourceKind};
use crate::repository::{Subscription, subscribe_kind};
use gpui_kit::App;

/// Subscribe to the live StorageClasses stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::StorageClasses, NamespaceScope::All)
}
