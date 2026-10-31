//! Pod log buffer state (no gpui or kube dependencies).

use std::collections::VecDeque;

pub const MAX_LOG_LINES: usize = 5_000;

/// Buffered output and controls for one pod/container log stream.
#[derive(Clone, Debug)]
pub struct PodLogsModel {
    lines: VecDeque<String>,
    text: String,
    container: String,
    follow: bool,
    status: String,
    error: Option<String>,
}

impl PodLogsModel {
    pub fn new(container: String) -> Self {
        Self {
            lines: VecDeque::new(),
            text: String::new(),
            container,
            follow: true,
            status: "Connecting…".to_string(),
            error: None,
        }
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn set_container(&mut self, container: String) {
        if self.container == container {
            return;
        }
        self.container = container;
        self.clear();
    }

    pub fn follow(&self) -> bool {
        self.follow
    }

    pub fn set_follow(&mut self, follow: bool) {
        if self.follow == follow {
            return;
        }
        self.follow = follow;
        self.clear();
    }

    pub fn toggle_follow(&mut self) {
        self.set_follow(!self.follow);
    }

    /// Cached joined output, maintained on push/clear so render does not join.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn push_line(&mut self, line: String) {
        if self.text.is_empty() {
            self.text.push_str(&line);
        } else {
            self.text.push('\n');
            self.text.push_str(&line);
        }
        self.lines.push_back(line);
        if self.lines.len() > MAX_LOG_LINES {
            self.lines.pop_front();
            self.rebuild_text();
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.text.clear();
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

    fn rebuild_text(&mut self) {
        self.text = self.lines.iter().cloned().collect::<Vec<_>>().join("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_bounded_buffer_and_cached_text() {
        let mut model = PodLogsModel::new("api".to_string());
        for index in 0..(MAX_LOG_LINES + 3) {
            model.push_line(format!("line-{index}"));
        }
        assert_eq!(model.line_count(), MAX_LOG_LINES);
        assert!(model.text().ends_with("line-5002"));
        assert!(!model.text().contains("line-0\n"));
        assert_eq!(model.text().lines().count(), MAX_LOG_LINES);
    }

    #[test]
    fn clears_on_container_or_follow_change() {
        let mut model = PodLogsModel::new("api".to_string());
        model.push_line("hello".to_string());
        model.set_container("sidecar".to_string());
        assert!(model.text().is_empty());
        assert_eq!(model.container(), "sidecar");

        model.push_line("again".to_string());
        model.toggle_follow();
        assert!(!model.follow());
        assert!(model.text().is_empty());
    }
}
