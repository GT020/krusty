//! Process-wide logging setup.
//!
//! Logs go to stderr with timestamps and thread names so a UI-thread stall can
//! be told apart from a busy async runtime. Override the filter with
//! `RUST_LOG` (for example `RUST_LOG=krusty=trace`).

use tracing_subscriber::EnvFilter;

pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,krusty=debug"));

    let result = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .try_init();

    if let Err(error) = result {
        eprintln!("krusty: logging already initialized: {error}");
    }
}
