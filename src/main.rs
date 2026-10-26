mod cluster;
mod logging;
mod models;
mod repository;
mod ui;
mod view_models;

fn window_options() -> gpui_kit::WindowOptions {
    let mut options = gpui_kit::component::TitleBar::window_options();
    options.icon = window_icon();
    options.app_id = Some("io.github.GT020.Krusty".to_string());
    options
}

fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../assets/app-icon.png");
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    Some(std::sync::Arc::new(image::imageops::resize(
        &image,
        256,
        256,
        image::imageops::FilterType::Lanczos3,
    )))
}

fn main() {
    logging::init();
    let started = std::time::Instant::now();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "krusty: process starting"
    );

    // embedded Lucide icons via asset source below
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "krusty: gpui app loop entered"
            );

            let init_started = std::time::Instant::now();
            gpui_kit::init(cx);
            tracing::debug!(
                elapsed_ms = init_started.elapsed().as_millis() as u64,
                "krusty: gpui_kit initialized"
            );

            ui::settings::theme::init(cx);
            ui::settings::custom_resources::init(cx);
            repository::init(cx);
            let cluster_started = std::time::Instant::now();
            cluster::init(cx);
            tracing::debug!(
                elapsed_ms = cluster_started.elapsed().as_millis() as u64,
                "krusty: cluster initialized"
            );

            use gpui_kit::AppContext as _;
            match gpui_kit::open_window(window_options(), cx, |window, cx| {
                cx.new(|cx| ui::Workspace::new(window, cx))
            }) {
                Ok(_window) => {
                    tracing::info!("krusty: main window opened");
                }
                Err(error) => {
                    tracing::error!(%error, "krusty: failed to open main window");
                    panic!("failed to open window");
                }
            }
            cx.activate(true);
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "krusty: startup complete"
            );
        });

    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "krusty: process exiting"
    );
}
