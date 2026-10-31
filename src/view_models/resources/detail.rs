//! ViewModel for the shared resource detail pane.

use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::ResourceKind;
use crate::models::resources::detail::{ResourceDetailState, ResourceDetailUpdate};
use crate::repository;
use gpui_kit::Context;

pub struct ResourceDetailViewModel {
    state: ResourceDetailState,
    epoch: u64,
    cancel: Option<watch::Sender<bool>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl ResourceDetailViewModel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let epoch = cx.global::<ClusterState>().context_epoch();
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let epoch = cx.global::<ClusterState>().context_epoch();
            if epoch == this.epoch {
                return;
            }
            this.epoch = epoch;
            if this.state.is_open() {
                this.close(cx);
            }
        });
        Self {
            state: ResourceDetailState::Closed,
            epoch,
            cancel: None,
            cluster_sub,
        }
    }

    pub fn state(&self) -> &ResourceDetailState {
        &self.state
    }

    pub fn open(
        &mut self,
        kind: ResourceKind,
        namespace: String,
        name: String,
        cx: &mut Context<Self>,
    ) {
        self.open_inner(kind, namespace, name, false, cx);
    }

    fn open_inner(
        &mut self,
        kind: ResourceKind,
        namespace: String,
        name: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !force
            && self.state.identity() == Some((kind, namespace.as_str(), name.as_str()))
            && !matches!(self.state, ResourceDetailState::Error { .. })
        {
            return;
        }
        self.cancel_detail();
        self.state = ResourceDetailState::Loading {
            kind,
            namespace: namespace.clone(),
            name: name.clone(),
        };
        cx.notify();
        let mut subscription =
            match repository::resources::detail::subscribe_detail(cx, kind, &namespace, &name) {
                Ok(subscription) => subscription,
                Err(error) => {
                    self.state = ResourceDetailState::Error {
                        kind,
                        namespace,
                        name,
                        message: format!("Could not open {} details: {error}", kind.label()),
                    };
                    cx.notify();
                    return;
                }
            };
        self.cancel = Some(subscription.cancel_handle());
        cx.spawn(async move |this, cx| {
            while let Some(update) = subscription.next().await {
                let applied = this.update(cx, |this, cx| {
                    if this.apply_update(update) {
                        cx.notify();
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.cancel_detail();
        if self.state.is_open() {
            self.state = ResourceDetailState::Closed;
            cx.notify();
        }
    }

    /// Refetch the open resource without passing through `Closed`, so hosts do
    /// not tear down and remount their detail widgets.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some((kind, namespace, name)) = self
            .state
            .identity()
            .map(|(kind, namespace, name)| (kind, namespace.to_string(), name.to_string()))
        else {
            return;
        };
        self.open_inner(kind, namespace, name, true, cx);
    }

    fn apply_update(&mut self, update: ResourceDetailUpdate) -> bool {
        if self.state.identity()
            != Some((update.kind, update.namespace.as_str(), update.name.as_str()))
        {
            return false;
        }
        match update.result {
            Ok(detail) => self.state = ResourceDetailState::Ready(detail),
            Err(message) => {
                self.state = ResourceDetailState::Error {
                    kind: update.kind,
                    namespace: update.namespace,
                    name: update.name,
                    message,
                };
            }
        }
        true
    }

    fn cancel_detail(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }
}
