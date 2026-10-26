//! Repository for Nodes: subscribe to the live list/watch stream.

use crate::models::ResourceKind;
use super::{Subscription, subscribe_kind};
use gpui_kit::App;

/// Subscribe to the live Nodes stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::Nodes, crate::models::NamespaceScope::All)
}
