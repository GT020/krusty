mod app;
mod kubernetes;
mod ui;

use app::{update, view, KrustyApp};

pub fn main() -> iced::Result {
    iced::application("Krusty - Kubernetes Dashboard", update, view)
        .run_with(KrustyApp::new)
}
