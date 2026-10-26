//! Repository for Ingresses: subscribe to the live list/watch stream.

use crate::models::{NamespaceScope, ResourceKind};
use super::{Subscription, subscribe_kind};
use gpui_kit::App;

/// Subscribe to the live Ingresses stream.
/// Returns a [`Subscription`] that delivers immutable snapshots as the cluster changes.
pub fn subscribe(cx: &mut App, scope: NamespaceScope) -> Result<Subscription, String> {
    subscribe_kind(cx, ResourceKind::Ingresses, scope)
}
