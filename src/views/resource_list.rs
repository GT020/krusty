use crate::k8s::context::CURRENT_NAMESPACE;
use dioxus::prelude::*;

#[component]
pub fn ResourcePageHeader(display_name: String, on_clear: EventHandler<()>) -> Element {
    let current_ns = CURRENT_NAMESPACE.signal().read().clone();

    rsx! {
        div {
            class: "p-6 pb-2",
            div { class: "flex justify-between items-center",
                h1 { class: "text-2xl font-bold", "{display_name}" }
                div { class: "flex items-center gap-4",
                    span { class: "text-sm text-green-600 bg-green-50 px-2 py-1 rounded", "● Watch Mode" }
                    button {
                        class: "px-3 py-1.5 bg-gray-100 text-gray-700 rounded hover:bg-gray-200 text-sm",
                        onclick: move |_| on_clear.call(()),
                        "Clear"
                    }
                }
            }
            div { class: "text-sm text-gray-500 mt-1",
                if current_ns.is_empty() {
                    "All Namespaces"
                } else {
                    span { "Namespace: " {current_ns} }
                }
            }
        }
    }
}
