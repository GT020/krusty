use dioxus::prelude::*;
use tracing::info as log_info;

mod components;
mod k8s;
mod views;

pub use views::Route;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[component]
fn App() -> Element {
    use_effect(|| {
        let fut = async {
            match k8s::client::create_client().await {
                Ok(_) => {
                    log_info!("Cluster connected");
                    k8s::context::set_connected(true);
                }
                Err(e) => eprintln!("Failed to connect: {}", e),
            }
        };
        spawn(fut);
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

fn main() {
    dioxus::launch(App);
}