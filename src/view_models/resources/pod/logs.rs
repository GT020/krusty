//! ViewModel for live pod logs: container selection, follow, and buffered lines.

use gpui_kit::Context;
use tokio::sync::watch;

use crate::repository::resources::pods;

const MAX_LINES: usize = 5_000;

pub struct PodLogsViewModel {
    namespace: String,
    name: String,
    containers: Vec<String>,
    container: String,
    follow: bool,
    lines: Vec<String>,
    status: String,
    error: Option<String>,
    generation: u64,
    cancel: Option<watch::Sender<bool>>,
}

impl PodLogsViewModel {
    pub fn new(
        namespace: String,
        name: String,
        containers: Vec<String>,
        selected_container: Option<String>,
        follow: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let container = selected_container
            .and_then(|selected| {
                containers
                    .iter()
                    .find(|container| **container == selected)
                    .cloned()
            })
            .or_else(|| containers.first().cloned())
            .unwrap_or_default();
        let mut this = Self {
            namespace,
            name,
            containers,
            container,
            follow,
            lines: Vec::new(),
            status: "Connecting…".to_string(),
            error: None,
            generation: 0,
            cancel: None,
        };
        this.subscribe(cx);
        this
    }

    /// A log panel that could not start, used when the pod itself cannot be
    /// read to discover its containers.
    pub fn failed(namespace: String, name: String, error: String) -> Self {
        Self {
            namespace,
            name,
            containers: Vec::new(),
            container: String::new(),
            follow: false,
            lines: Vec::new(),
            status: "Unable to load pod containers".to_string(),
            error: Some(error),
            generation: 0,
            cancel: None,
        }
    }

    pub fn containers(&self) -> &[String] {
        &self.containers
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn follow(&self) -> bool {
        self.follow
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_container(&mut self, container: String, cx: &mut Context<Self>) {
        if container == self.container {
            return;
        }
        self.container = container;
        self.lines.clear();
        self.subscribe(cx);
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.follow = !self.follow;
        self.lines.clear();
        self.subscribe(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.lines.clear();
        cx.notify();
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
        self.error = None;
        self.status = format!("Connecting to {}…", self.container);
        cx.notify();

        let stream = match pods::subscribe_logs(
            cx,
            &self.namespace,
            &self.name,
            &self.container,
            self.follow,
        ) {
            Ok(stream) => stream,
            Err(error) => {
                self.status = "Unable to stream logs".to_string();
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.cancel = Some(stream.cancel_handle());
        self.status = if self.follow {
            format!("Streaming {} · following", self.container)
        } else {
            format!("Loaded {} · paused", self.container)
        };

        cx.spawn(async move |this, cx| {
            let mut stream = stream;
            while let Some(update) = stream.next().await {
                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    match update {
                        Ok(line) => {
                            this.push_line(line);
                            this.status = if this.follow {
                                format!("Streaming {} · following", this.container)
                            } else {
                                format!("Loaded {} · paused", this.container)
                            };
                            this.error = None;
                        }
                        Err(error) => {
                            this.error = Some(error);
                            this.status = "Log stream error".to_string();
                        }
                    }
                    cx.notify();
                    true
                });
                match current {
                    Ok(true) => {}
                    _ => break,
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.generation == generation {
                    this.status = "Log stream ended".to_string();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn push_line(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            let overflow = self.lines.len() - MAX_LINES;
            self.lines.drain(0..overflow);
        }
    }
}

impl Drop for PodLogsViewModel {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }
}
