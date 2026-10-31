//! ViewModel for one pinned pod detail section. Owns its own detail stream.

use gpui_kit::Context;
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::pod::detail::{PodDetailState, PodDetailUpdate};
use crate::repository;

pub struct PodSectionViewModel {
    namespace: String,
    name: String,
    state: PodDetailState,
    active: bool,
    epoch: u64,
    generation: u64,
    cancel: Option<watch::Sender<bool>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl PodSectionViewModel {
    pub fn new(namespace: String, name: String, cx: &mut Context<Self>) -> Self {
        let epoch = cx.global::<ClusterState>().context_epoch();
        let cluster_sub = cx.observe_global::<ClusterState>(|this, cx| {
            let epoch = cx.global::<ClusterState>().context_epoch();
            if epoch == this.epoch {
                return;
            }
            this.epoch = epoch;
            this.subscribe(cx);
            cx.notify();
        });
        let mut this = Self {
            state: PodDetailState::Loading {
                namespace: namespace.clone(),
                name: name.clone(),
            },
            namespace,
            name,
            active: true,
            epoch,
            generation: 0,
            cancel: None,
            cluster_sub,
        };
        this.subscribe(cx);
        this
    }

    pub fn state(&self) -> &PodDetailState {
        &self.state
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        if active {
            cx.notify();
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.state = PodDetailState::Loading {
            namespace: self.namespace.clone(),
            name: self.name.clone(),
        };
        cx.notify();

        let mut subscription =
            match repository::resources::pods::subscribe_detail(cx, &self.namespace, &self.name) {
                Ok(subscription) => subscription,
                Err(error) => {
                    self.state = PodDetailState::Error {
                        namespace: self.namespace.clone(),
                        name: self.name.clone(),
                        message: format!("Could not open pod section: {error}"),
                    };
                    cx.notify();
                    return;
                }
            };
        self.cancel = Some(subscription.cancel_handle());
        cx.spawn(async move |this, cx| {
            while let Some(update) = subscription.next().await {
                let _ = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return;
                    }
                    if this.apply_update(update) && this.active {
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn apply_update(&mut self, update: PodDetailUpdate) -> bool {
        match update {
            PodDetailUpdate::Detail {
                namespace,
                name,
                result,
            } => {
                if namespace != self.namespace || name != self.name {
                    return false;
                }
                match result {
                    Ok(detail) => self.state = PodDetailState::Ready(detail),
                    Err(message) => {
                        self.state = PodDetailState::Error {
                            namespace,
                            name,
                            message,
                        };
                    }
                }
                true
            }
        }
    }
}

impl Drop for PodSectionViewModel {
    fn drop(&mut self) {
        self.shutdown();
    }
}
