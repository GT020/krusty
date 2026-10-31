//! Pod exec terminal state (no gpui or kube dependencies).

use std::num::NonZeroU16;

pub const SHELLS: [&str; 2] = ["sh", "bash"];
pub const SCROLLBACK_LINES: usize = 5_000;
pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 24;

/// Terminal screen, selected container/shell, and connection status.
pub struct PodExecModel {
    container: String,
    shell: String,
    parser: vt100::Parser,
    status: String,
    error: Option<String>,
}

impl PodExecModel {
    pub fn new(container: String, shell: String) -> Self {
        Self {
            container,
            shell,
            parser: vt100::Parser::new(
                NonZeroU16::new(DEFAULT_ROWS).unwrap_or(NonZeroU16::MIN),
                NonZeroU16::new(DEFAULT_COLS).unwrap_or(NonZeroU16::MIN),
                SCROLLBACK_LINES,
            ),
            status: "Disconnected".to_string(),
            error: None,
        }
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn set_container(&mut self, container: String) {
        self.container = container;
    }

    pub fn shell(&self) -> &str {
        &self.shell
    }

    pub fn set_shell(&mut self, shell: String) {
        self.shell = shell;
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

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn process_error(&mut self, error: &str) {
        let text = format!("\r\n\x1b[31m{error}\x1b[0m\r\n");
        self.parser.process(text.as_bytes());
    }

    /// Reset the screen while keeping the current grid size.
    pub fn reset_screen(&mut self) {
        let (rows, cols) = self.parser.screen().size();
        self.parser = vt100::Parser::new(rows, cols, SCROLLBACK_LINES);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Option<(u16, u16)> {
        if cols == 0 || rows == 0 {
            return None;
        }
        let (current_rows, current_cols) = self.parser.screen().size();
        if current_rows.get() == rows && current_cols.get() == cols {
            return None;
        }
        let rows = NonZeroU16::new(rows).unwrap_or(NonZeroU16::MIN);
        let cols = NonZeroU16::new(cols).unwrap_or(NonZeroU16::MIN);
        self.parser.set_size(rows, cols);
        Some((cols.get(), rows.get()))
    }

    /// Scroll through terminal history. Positive values scroll up.
    pub fn scroll(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }
        let screen = self.parser.screen();
        let current = screen.scrollback() as i32;
        let max = screen.scrollback_len() as i32;
        let target = (current + lines).clamp(0, max);
        self.parser.screen_mut().set_scrollback(target as usize);
    }

    pub fn size(&self) -> (NonZeroU16, NonZeroU16) {
        self.parser.screen().size()
    }
}
