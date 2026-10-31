//! ViewModel for live pod logs: container selection, follow, and buffered lines.
//!
//! Owns its own log stream so the same widget can run in a tab or on the
//! dashboard.

use gpui_kit::Context;
use tokio::sync::watch;

use crate::models::resources::pod::logs::PodLogsModel;
use crate::repository::resources::pods;

pub struct PodLogsViewModel {
    namespace: String,
    name: String,
    containers: Vec<String>,
    model: PodLogsModel,
    active: bool,
    generation: u64,
    cancel: Option<watch::Sender<bool>>,
}

impl PodLogsViewModel {
    pub fn new_with(
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: Option<String>,
        follow: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let container = container
            .filter(|container| containers.iter().any(|name| name == container))
            .or_else(|| containers.first().cloned())
            .unwrap_or_default();
        let mut model = PodLogsModel::new(container);
        model.set_follow(follow);
        let mut this = Self {
            namespace,
            name,
            containers,
            model,
            active: true,
            generation: 0,
            cancel: None,
        };
        this.subscribe(cx);
        this
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn containers(&self) -> &[String] {
        &self.containers
    }

    pub fn container(&self) -> &str {
        self.model.container()
    }

    pub fn follow(&self) -> bool {
        self.model.follow()
    }

    pub fn text(&self) -> &str {
        self.model.text()
    }

    pub fn line_count(&self) -> usize {
        self.model.line_count()
    }

    pub fn status(&self) -> &str {
        self.model.status()
    }

    pub fn error(&self) -> Option<&str> {
        self.model.error()
    }

    pub fn set_container(&mut self, container: String, cx: &mut Context<Self>) {
        if container == self.model.container() {
            return;
        }
        self.model.set_container(container);
        self.subscribe(cx);
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.model.toggle_follow();
        self.subscribe(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.model.clear();
        cx.notify();
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
        self.model.set_error(None);
        self.model
            .set_status(format!("Connecting to {}…", self.model.container()));
        cx.notify();

        let stream = match pods::subscribe_logs(
            cx,
            &self.namespace,
            &self.name,
            self.model.container(),
            self.model.follow(),
        ) {
            Ok(stream) => stream,
            Err(error) => {
                self.model.set_status("Unable to stream logs".to_string());
                self.model.set_error(Some(error));
                cx.notify();
                return;
            }
        };
        self.cancel = Some(stream.cancel_handle());
        self.model.set_status(if self.model.follow() {
            format!("Streaming {} · following", self.model.container())
        } else {
            format!("Loaded {} · paused", self.model.container())
        });

        cx.spawn(async move |this, cx| {
            let mut stream = stream;
            while let Some(update) = stream.next().await {
                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    match update {
                        Ok(line) => {
                            this.model.push_line(line);
                            this.model.set_status(if this.model.follow() {
                                format!("Streaming {} · following", this.model.container())
                            } else {
                                format!("Loaded {} · paused", this.model.container())
                            });
                            this.model.set_error(None);
                        }
                        Err(error) => {
                            this.model.set_error(Some(error));
                            this.model.set_status("Log stream error".to_string());
                        }
                    }
                    if this.active {
                        cx.notify();
                    }
                    true
                });
                match current {
                    Ok(true) => {}
                    _ => break,
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.generation == generation {
                    this.model.set_status("Log stream ended".to_string());
                    if this.active {
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

impl Drop for PodLogsViewModel {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }
}
