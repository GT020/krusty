//! ViewModel for an interactive TTY exec session in a pod container.

use gpui_kit::Context;
use tokio::sync::{mpsc, watch};

use crate::models::resources::pod::exec::PodExecModel;
use crate::repository::resources::pods;

pub use crate::models::resources::pod::exec::SHELLS;

pub struct PodExecViewModel {
    namespace: String,
    name: String,
    containers: Vec<String>,
    model: PodExecModel,
    auto_connect: bool,
    active: bool,
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    resize_tx: Option<mpsc::UnboundedSender<(u16, u16)>>,
    generation: u64,
    cancel: Option<watch::Sender<bool>>,
}

impl PodExecViewModel {
    #[allow(clippy::too_many_arguments)]
    pub fn new_with(
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: String,
        shell: String,
        auto_connect: bool,
        _window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let model = PodExecModel::new(container, shell);
        let mut this = Self {
            namespace,
            name,
            containers,
            model,
            auto_connect,
            active: true,
            input_tx: None,
            resize_tx: None,
            generation: 0,
            cancel: None,
        };
        if auto_connect {
            this.start_session(cx);
        }
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

    pub fn shell(&self) -> &str {
        self.model.shell()
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.model.screen()
    }

    pub fn application_cursor(&self) -> bool {
        self.model.application_cursor()
    }

    pub fn status(&self) -> &str {
        self.model.status()
    }

    pub fn error(&self) -> Option<&str> {
        self.model.error()
    }

    pub fn is_connected(&self) -> bool {
        self.cancel.is_some()
    }

    pub fn set_container(&mut self, container: String, cx: &mut Context<Self>) {
        if container == self.model.container() {
            return;
        }
        self.model.set_container(container);
        self.restart(cx);
    }

    pub fn set_shell(&mut self, shell: String, cx: &mut Context<Self>) {
        if shell == self.model.shell() {
            return;
        }
        self.model.set_shell(shell);
        self.restart(cx);
    }

    /// Connect a widget that was restored in a disconnected state.
    pub fn connect(&mut self, cx: &mut Context<Self>) {
        if self.is_connected() {
            return;
        }
        self.auto_connect = true;
        self.start_session(cx);
    }

    /// Send raw terminal input bytes to the shell.
    pub fn send_input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(input) = &self.input_tx {
            if input.send(bytes).is_err() {
                self.model
                    .set_error(Some("Exec session is no longer connected.".to_string()));
            }
        } else {
            self.model
                .set_error(Some("Exec session is not connected yet.".to_string()));
        }
        cx.notify();
    }

    /// Resize the terminal grid and tell the remote TTY about it.
    pub fn resize(&mut self, cols: u16, rows: u16, cx: &mut Context<Self>) {
        let Some((cols, rows)) = self.model.resize(cols, rows) else {
            return;
        };
        if let Some(resize) = &self.resize_tx {
            let _ = resize.send((cols, rows));
        }
        cx.notify();
    }

    /// Scroll through terminal history. Positive values scroll up.
    pub fn scroll(&mut self, lines: i32, cx: &mut Context<Self>) {
        self.model.scroll(lines);
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
        self.input_tx = None;
        self.resize_tx = None;
        self.model.set_status("Disconnected".to_string());
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.model.reset_screen();
        if self.auto_connect {
            self.start_session(cx);
        } else {
            self.model.set_status("Disconnected".to_string());
            self.model.set_error(None);
            cx.notify();
        }
    }

    fn start_session(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.input_tx = None;
        self.resize_tx = None;
        self.model.set_error(None);
        self.model.set_status(format!(
            "Starting {} in {}…",
            self.model.shell(),
            self.model.container()
        ));
        cx.notify();

        let session = match pods::exec_shell(
            cx,
            &self.namespace,
            &self.name,
            self.model.container(),
            self.model.shell(),
        ) {
            Ok(session) => session,
            Err(error) => {
                self.model
                    .set_status("Unable to start exec session".to_string());
                self.model.set_error(Some(error.clone()));
                self.model.process_error(&error);
                cx.notify();
                return;
            }
        };
        self.cancel = Some(session.cancel_handle());
        self.input_tx = Some(session.input_handle());
        let resize = session.resize_handle();
        let (rows, cols) = self.model.size();
        let _ = resize.send((cols.get(), rows.get()));
        self.resize_tx = Some(resize);
        self.model.set_status(format!(
            "Connecting · {} · {}",
            self.model.container(),
            self.model.shell()
        ));

        cx.spawn(async move |this, cx| {
            let mut session = session;
            while let Some(update) = session.next().await {
                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    match update {
                        Ok(bytes) => {
                            this.model.process(&bytes);
                            this.model.set_error(None);
                            this.model.set_status(format!(
                                "Connected · {} · {}",
                                this.model.container(),
                                this.model.shell()
                            ));
                        }
                        Err(error) => {
                            this.model.process_error(&error);
                            this.model.set_error(Some(error));
                            this.model.set_status("Exec session error".to_string());
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
                    this.model.process(b"\r\n[session ended]\r\n");
                    this.model.set_status("Exec session ended".to_string());
                    this.input_tx = None;
                    this.resize_tx = None;
                    this.cancel = None;
                    if this.active {
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

impl Drop for PodExecViewModel {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }
}
