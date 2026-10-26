//! Deployment detail actions: scale, restart, edit, and delete.

use std::future::Future;

use kube::Client;

use crate::cluster::ClusterState;
use crate::repository;
use crate::view_models::resources::detail::ResourceDetailViewModel;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Editor, EditorState, Input, InputState};
use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, ParentElement as _, WeakEntity, Window, px,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Pending,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct ActionFeedback {
    status: ActionStatus,
    message: String,
}

impl ActionFeedback {
    fn pending(message: String) -> Self {
        Self {
            status: ActionStatus::Pending,
            message,
        }
    }

    fn success(message: String) -> Self {
        Self {
            status: ActionStatus::Success,
            message,
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: ActionStatus::Error,
            message,
        }
    }

    pub fn status(&self) -> ActionStatus {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub struct DeploymentsDetailViewModel {
    detail: WeakEntity<ResourceDetailViewModel>,
    feedback: Option<ActionFeedback>,
}

impl DeploymentsDetailViewModel {
    pub fn new(detail: WeakEntity<ResourceDetailViewModel>) -> Self {
        Self {
            detail,
            feedback: None,
        }
    }

    pub fn feedback(&self) -> Option<&ActionFeedback> {
        self.feedback.as_ref()
    }

    pub fn scale(
        &mut self,
        namespace: String,
        name: String,
        replicas: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub fn restart(&mut self, namespace: String, name: String, cx: &mut Context<Self>) {
        let pending = format!("Restarting deployment “{name}”…");
        let success = format!("Restarted deployment “{name}”.");
        self.run_action(
            (namespace.clone(), name.clone()),
            pending,
            success,
            false,
            move |client| async move {
                repository::resources::deployments::restart(client, namespace, name).await
            },
            cx,
        );
    }

    pub fn delete(
        &mut self,
        namespace: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().downgrade();
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

    pub fn edit(
        &mut self,
        namespace: String,
        name: String,
        manifest: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let yaml = match serde_saphyr::to_string(manifest) {
            Ok(yaml) => yaml,
            Err(error) => {
                self.feedback = Some(ActionFeedback::error(format!(
                    "Could not render the deployment manifest: {error}"
                )));
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
            (namespace.clone(), name.clone()),
            pending,
            success,
            false,
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
            (namespace.clone(), name.clone()),
            pending,
            success,
            true,
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
            (namespace.clone(), name.clone()),
            pending,
            success,
            false,
            move |client| async move {
                repository::resources::deployments::replace(client, namespace, name, yaml).await
            },
            cx,
        );
    }

    fn reload(&self, cx: &mut Context<Self>) {
        let Some(detail) = self.detail.upgrade() else {
            return;
        };
        let Some((kind, namespace, name)) = detail
            .read(cx)
            .state()
            .identity()
            .map(|(kind, namespace, name)| (kind, namespace.to_string(), name.to_string()))
        else {
            return;
        };
        detail.update(cx, |detail, cx| {
            detail.close(cx);
            detail.open(kind, namespace, name, cx);
        });
    }

    fn run_action<F, Fut>(
        &mut self,
        target: (String, String),
        pending: String,
        success: String,
        close_on_success: bool,
        operation: F,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce(Client) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let Some(client) = cx.global::<ClusterState>().client() else {
            self.feedback = Some(ActionFeedback::error(
                "Not connected to a cluster.".to_string(),
            ));
            cx.notify();
            return;
        };
        let tokio = cx.global::<ClusterState>().tokio();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        tokio.spawn(async move {
            let _ = sender.send(operation(client).await);
        });
        self.feedback = Some(ActionFeedback::pending(pending));
        cx.notify();
        let this = cx.entity().downgrade();
        cx.spawn(async move |_, cx: &mut gpui_kit::AsyncApp| {
            let result = match receiver.await {
                Ok(result) => result,
                Err(_) => Err("The cluster operation was cancelled.".to_string()),
            };
            let _ = this.update(cx, |this, cx| {
                let current = this.detail.upgrade().and_then(|detail| {
                    detail
                        .read(cx)
                        .state()
                        .identity()
                        .map(|(_, namespace, name)| (namespace.to_string(), name.to_string()))
                });
                if current.as_ref() != Some(&target) {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.feedback = Some(ActionFeedback::success(success));
                        if close_on_success {
                            if let Some(detail) = this.detail.upgrade() {
                                detail.update(cx, |detail, cx| detail.close(cx));
                            }
                        } else {
                            this.reload(cx);
                        }
                    }
                    Err(message) => {
                        this.feedback = Some(ActionFeedback::error(message));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
