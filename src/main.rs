pub mod app;
pub mod models;
pub mod repos;
pub mod ui;
pub mod view_models;
pub mod kubernetes;

fn main() {
    let _ = iced::application("Krusty - Kubernetes Desktop Client", app::update, app::view)
        .subscription(app::KrustyApp::subscription)
        .theme(|_| iced::Theme::Dark)
        .run_with(app::KrustyApp::new);
}
