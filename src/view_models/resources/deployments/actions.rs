//! Standalone deployment actions: scale, restart, edit, and delete.
//!
//! This view model has a fixed target and no dependency on a detail pane. The
//! deployment detail and the dashboard both mount it through
//! `DeploymentActionsWidget`.

use std::future::Future;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Editor, EditorState, Input, InputState};
use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, EventEmitter, ParentElement as _, Window, px,
};
use kube::Client;

use crate::cluster::ClusterState;
use crate::models::resources::deployments::actions::{
    ActionFeedback, DeploymentActionEvent, DeploymentActionsModel,
};
use crate::repository;

pub struct DeploymentActionsViewModel {
    namespace: String,
    name: String,
    model: DeploymentActionsModel,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    generation: u64,
}

impl EventEmitter<DeploymentActionEvent> for DeploymentActionsViewModel {}

impl DeploymentActionsViewModel {
    pub fn new(namespace: String, name: String, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            namespace,
            name,
            model: DeploymentActionsModel::new(),
            cancel: None,
            generation: 0,
        };
        this.refresh_summary(cx);
        this
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn feedback(&self) -> Option<&ActionFeedback> {
        self.model.feedback()
    }

    pub fn replicas(&self) -> Option<i32> {
        self.model.replicas()
    }

    pub fn scale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let replicas = self.model.replicas().unwrap_or(1);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Replicas")
                .default_value(replicas.to_string())
                .validate(|value, _| {
                    value
                        .trim()
                        .parse::<i32>()
                        .map(|replicas| replicas >= 0)
                        .unwrap_or(false)
                })
        });
        let this = cx.entity().downgrade();
        let namespace = self.namespace.clone();
        let name = self.name.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let input = input.clone();
            let this = this.clone();
            let namespace = namespace.clone();
            let name = name.clone();
            alert
                .title("Scale deployment")
                .description(format!("Set the desired replica count for “{name}”."))
                .child(Input::new(&input))
                .ok_text("Scale")
                .show_cancel(true)
                .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    let value = input.read(cx).value();
                    let Ok(replicas) = value.trim().parse::<i32>() else {
                        return false;
                    };
                    if replicas < 0 {
                        return false;
                    }
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |this, cx| {
                            this.perform_scale(namespace.clone(), name.clone(), replicas, cx)
                        });
                    }
                    true
                })
        });
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        let pending = format!("Restarting deployment “{}”…", self.name);
        let success = format!("Restarted deployment “{}”.", self.name);
        let namespace = self.namespace.clone();
        let name = self.name.clone();
        self.run_action(
            pending,
            success,
            DeploymentActionEvent::Changed,
            move |client| async move {
                repository::resources::deployments::restart(client, namespace, name).await
            },
            cx,
        );
    }

    pub fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let namespace = self.namespace.clone();
        let name = self.name.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let this = this.clone();
            let namespace = namespace.clone();
            let name = name.clone();
            alert
                .title(format!("Delete “{name}”?"))
                .description(
                    "The deployment and the pods it manages will be deleted. \
                     This cannot be undone.",
                )
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |this, cx| {
                            this.perform_delete(namespace.clone(), name.clone(), cx)
                        });
                    }
                    true
                })
        });
    }

    pub fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manifest) = self.model.manifest().cloned() else {
            self.model.set_feedback(Some(ActionFeedback::error(
                "The deployment manifest is not loaded yet.".to_string(),
            )));
            cx.notify();
            return;
        };
        let yaml = match serde_saphyr::to_string(&manifest) {
            Ok(yaml) => yaml,
            Err(error) => {
                self.model.set_feedback(Some(ActionFeedback::error(format!(
                    "Could not render the deployment manifest: {error}"
                ))));
                cx.notify();
                return;
            }
        };
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(yaml)
                .language("yaml")
        });
        let this = cx.entity().downgrade();
        let namespace = self.namespace.clone();
        let name = self.name.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let editor = editor.clone();
            let this = this.clone();
            let namespace = namespace.clone();
            let name = name.clone();
            alert
                .title("Edit deployment")
                .description(format!(
                    "Update the manifest for “{name}”. Saving replaces the deployment."
                ))
                .width(px(760.))
                .child(Editor::new(&editor).h(px(420.)))
                .ok_text("Save")
                .show_cancel(true)
                .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    let yaml = editor.read(cx).value().to_string();
                    if yaml.trim().is_empty() {
                        return false;
                    }
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |this, cx| {
                            this.perform_replace(namespace.clone(), name.clone(), yaml, cx)
                        });
                    }
                    true
                })
        });
    }

    pub fn shutdown(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }

    fn refresh_summary(&mut self, cx: &mut Context<Self>) {
        self.shutdown();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let subscription = match repository::resources::deployments::summary::subscribe_summary(
            cx,
            &self.namespace,
            &self.name,
        ) {
            Ok(subscription) => subscription,
            Err(error) => {
                self.model.set_feedback(Some(ActionFeedback::error(format!(
                    "Could not load deployment: {error}"
                ))));
                cx.notify();
                return;
            }
        };
        self.cancel = Some(subscription.cancel_handle());
        let mut subscription = subscription;
        cx.spawn(async move |this, cx| {
            if let Some(result) = subscription.next().await {
                let _ = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return;
                    }
                    match result {
                        Ok(summary) => {
                            this.model.set_replicas(summary.replicas);
                            this.model.set_manifest(summary.manifest);
                        }
                        Err(error) => {
                            this.model.set_feedback(Some(ActionFeedback::error(error)));
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn perform_scale(
        &mut self,
        namespace: String,
        name: String,
        replicas: i32,
        cx: &mut Context<Self>,
    ) {
        let pending = format!("Scaling deployment “{name}” to {replicas} replicas…");
        let success = format!("Scaled deployment “{name}” to {replicas} replicas.");
        self.run_action(
            pending,
            success,
            DeploymentActionEvent::Changed,
            move |client| async move {
                repository::resources::deployments::scale(client, namespace, name, replicas).await
            },
            cx,
        );
    }

    fn perform_delete(&mut self, namespace: String, name: String, cx: &mut Context<Self>) {
        let pending = format!("Deleting deployment “{name}”…");
        let success = format!("Deleted deployment “{name}”.");
        self.run_action(
            pending,
            success,
            DeploymentActionEvent::Deleted,
            move |client| async move {
                repository::resources::deployments::delete(client, namespace, name).await
            },
            cx,
        );
    }

    fn perform_replace(
        &mut self,
        namespace: String,
        name: String,
        yaml: String,
        cx: &mut Context<Self>,
    ) {
        let pending = format!("Updating deployment “{name}”…");
        let success = format!("Updated deployment “{name}”.");
        self.run_action(
            pending,
            success,
            DeploymentActionEvent::Changed,
            move |client| async move {
                repository::resources::deployments::replace(client, namespace, name, yaml).await
            },
            cx,
        );
    }

    fn run_action<F, Fut>(
        &mut self,
        pending: String,
        success: String,
        event: DeploymentActionEvent,
        operation: F,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce(Client) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let Some(client) = cx.global::<ClusterState>().client() else {
            self.model.set_feedback(Some(ActionFeedback::error(
                "Not connected to a cluster.".to_string(),
            )));
            cx.notify();
            return;
        };
        let tokio = cx.global::<ClusterState>().tokio();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        tokio.spawn(async move {
            let _ = sender.send(operation(client).await);
        });
        self.model
            .set_feedback(Some(ActionFeedback::pending(pending)));
        cx.notify();
        let this = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = match receiver.await {
                Ok(result) => result,
                Err(_) => Err("The cluster operation was cancelled.".to_string()),
            };
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.model
                        .set_feedback(Some(ActionFeedback::success(success)));
                    cx.emit(event);
                    if event != DeploymentActionEvent::Deleted {
                        this.refresh_summary(cx);
                    }
                }
                Err(message) => {
                    this.model
                        .set_feedback(Some(ActionFeedback::error(message)));
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Drop for DeploymentActionsViewModel {
    fn drop(&mut self) {
        self.shutdown();
    }
}
