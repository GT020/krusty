//! ViewModel for one pod's live usage. Owns its own metrics subscription.

use gpui_kit::Context;
use tokio::sync::watch;

use crate::cluster::ClusterState;
use crate::models::resources::pod::usage::{PodUsageModel, PodUsageUpdate};
use crate::repository;

pub struct PodUsageViewModel {
    namespace: String,
    name: String,
    model: PodUsageModel,
    active: bool,
    epoch: u64,
    generation: u64,
    cancel: Option<watch::Sender<bool>>,
    #[allow(dead_code)] // held for its Drop side-effect: cancels the GPUI observer
    cluster_sub: gpui_kit::Subscription,
}

impl PodUsageViewModel {
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
            namespace,
            name,
            model: PodUsageModel::new(),
            active: true,
            epoch,
            generation: 0,
            cancel: None,
            cluster_sub,
        };
        this.subscribe(cx);
        this
    }

    pub fn history(&self) -> &crate::models::resources::usage::UsageHistory {
        self.model.history()
    }

    pub fn error(&self) -> Option<&str> {
        self.model.error()
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

    fn cancel_stream(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.cancel_stream();
        self.model = PodUsageModel::new();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let subscription = match repository::resources::pods::usage::subscribe_usage(
            cx,
            &self.namespace,
            &self.name,
        ) {
            Ok(subscription) => subscription,
            Err(error) => {
                self.model
                    .set_error(format!("Could not open pod usage: {error}"));
                cx.notify();
                return;
            }
        };
        self.cancel = Some(subscription.cancel_handle());
        let mut subscription = subscription;
        cx.spawn(async move |this, cx: &mut gpui_kit::AsyncApp| {
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

    fn apply_update(&mut self, update: PodUsageUpdate) -> bool {
        match update {
            PodUsageUpdate::Sample {
                namespace,
                name,
                result,
            } => {
                if namespace != self.namespace || name != self.name {
                    return false;
                }
                match result {
                    Ok(sample) => self.model.apply_sample(sample),
                    Err(message) => self.model.set_error(message),
                }
                true
            }
        }
    }
}

impl Drop for PodUsageViewModel {
    fn drop(&mut self) {
        self.cancel_stream();
    }
}
