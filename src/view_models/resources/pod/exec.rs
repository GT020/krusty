//! ViewModel for an interactive TTY exec session in a pod container.
//!
//! The session itself lives in [`crate::repository::terminal`]. This view model
//! only attaches to it, so recreating the view does not kill the shell.

use std::num::NonZeroU16;
use std::sync::Arc;

use gpui_kit::Context;
use tokio::sync::{broadcast, mpsc};

use crate::repository::terminal::{self, TerminalEvent, TerminalSession};

const SCROLLBACK_LINES: usize = 5_000;
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;

pub const SHELLS: [&str; 2] = ["sh", "bash"];

pub struct PodExecViewModel {
    namespace: String,
    name: String,
    containers: Vec<String>,
    container: String,
    shell: String,
    parser: vt100::Parser,
    status: String,
    error: Option<String>,
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    resize_tx: Option<mpsc::UnboundedSender<(u16, u16)>>,
    session: Option<Arc<TerminalSession>>,
    generation: u64,
}

impl PodExecViewModel {
    pub fn new(
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: String,
        shell: String,
        _window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let parser = vt100::Parser::new(
            NonZeroU16::new(DEFAULT_ROWS).unwrap_or(NonZeroU16::MIN),
            NonZeroU16::new(DEFAULT_COLS).unwrap_or(NonZeroU16::MIN),
            SCROLLBACK_LINES,
        );
        let mut this = Self {
            namespace,
            name,
            containers,
            container,
            shell,
            parser,
            status: "Connecting…".to_string(),
            error: None,
            input_tx: None,
            resize_tx: None,
            session: None,
            generation: 0,
        };
        this.start_session(cx);
        this
    }

    pub fn containers(&self) -> &[String] {
        &self.containers
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn shell(&self) -> &str {
        &self.shell
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
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
        self.restart(cx);
    }

    pub fn set_shell(&mut self, shell: String, cx: &mut Context<Self>) {
        if shell == self.shell {
            return;
        }
        self.shell = shell;
        self.restart(cx);
    }

    /// Send raw terminal input bytes to the shell.
    pub fn send_input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(input) = &self.input_tx {
            if input.send(bytes).is_err() {
                self.error = Some("Exec session is no longer connected.".to_string());
            }
        } else {
            self.error = Some("Exec session is not connected yet.".to_string());
        }
        cx.notify();
    }

    /// Resize the terminal grid and tell the remote TTY about it.
    pub fn resize(&mut self, cols: u16, rows: u16, cx: &mut Context<Self>) {
        if cols == 0 || rows == 0 {
            return;
        }
        let (current_rows, current_cols) = self.parser.screen().size();
        if current_rows.get() == rows && current_cols.get() == cols {
            return;
        }
        let rows = NonZeroU16::new(rows).unwrap_or(NonZeroU16::MIN);
        let cols = NonZeroU16::new(cols).unwrap_or(NonZeroU16::MIN);
        self.parser.set_size(rows, cols);
        if let Some(resize) = &self.resize_tx {
            let _ = resize.send((cols.get(), rows.get()));
        }
        cx.notify();
    }

    /// Scroll through terminal history. Positive values scroll up.
    pub fn scroll(&mut self, lines: i32, cx: &mut Context<Self>) {
        if lines == 0 {
            return;
        }
        let screen = self.parser.screen();
        let current = screen.scrollback() as i32;
        let max = screen.scrollback_len() as i32;
        let target = (current + lines).clamp(0, max);
        self.parser.screen_mut().set_scrollback(target as usize);
        cx.notify();
    }

    /// Destroy the stored session. Called when the terminal tab is closed.
    pub fn shutdown(&mut self, cx: &mut gpui_kit::App) {
        if let Some(session) = self.session.take() {
            terminal::destroy(cx, session.key());
        }
        self.input_tx = None;
        self.resize_tx = None;
        self.generation = self.generation.wrapping_add(1);
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        let (rows, cols) = self.parser.screen().size();
        self.parser = vt100::Parser::new(rows, cols, SCROLLBACK_LINES);
        self.start_session(cx);
    }

    fn start_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.session.take() {
            terminal::destroy(cx, session.key());
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.input_tx = None;
        self.resize_tx = None;
        self.error = None;
        self.status = format!("Starting {} in {}…", self.shell, self.container);
        cx.notify();

        let session = match terminal::open(
            cx,
            &self.namespace,
            &self.name,
            &self.container,
            &self.shell,
        ) {
            Ok(session) => session,
            Err(error) => {
                self.status = "Unable to start exec session".to_string();
                self.error = Some(error.clone());
                self.process_error(&error);
                cx.notify();
                return;
            }
        };

        let attachment = session.attach();
        if !attachment.history.is_empty() {
            self.parser.process(&attachment.history);
        }
        self.input_tx = Some(session.input_handle());
        let resize = session.resize_handle();
        let (rows, cols) = self.parser.screen().size();
        let _ = resize.send((cols.get(), rows.get()));
        self.resize_tx = Some(resize);
        self.session = Some(session);
        self.status = format!("Connecting · {} · {}", self.container, self.shell);

        let mut updates = attachment.updates;
        cx.spawn(async move |this, cx| {
            loop {
                let event = match updates.recv().await {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    match event {
                        TerminalEvent::Data(bytes) => {
                            this.parser.process(&bytes);
                            this.error = None;
                            this.status =
                                format!("Connected · {} · {}", this.container, this.shell);
                        }
                        TerminalEvent::Error(error) => {
                            this.process_error(&error);
                            this.error = Some(error);
                            this.status = "Exec session error".to_string();
                        }
                        TerminalEvent::Ended => {
                            if this.error.is_none() {
                                this.parser.process(b"\r\n[session ended]\r\n");
                                this.status = "Exec session ended".to_string();
                            }
                            this.input_tx = None;
                            this.resize_tx = None;
                            this.session = None;
                            cx.notify();
                            return false;
                        }
                    }
                    cx.notify();
                    true
                });
                if !matches!(current, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn process_error(&mut self, error: &str) {
        let text = format!("\r\n\x1b[31m{error}\x1b[0m\r\n");
        self.parser.process(text.as_bytes());
    }
}

impl Drop for PodExecViewModel {
    fn drop(&mut self) {
        // Detaching from the view must not destroy the stored session. Closing
        // the tab calls `shutdown`, which performs the explicit destroy.
        self.input_tx = None;
        self.resize_tx = None;
    }
}
