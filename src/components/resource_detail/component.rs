use crate::components::detail_item::DetailItem;
use crate::components::labels::LabelTags;
use crate::k8s::client::K8sResource;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ResourceDetailProps {
    pub resource: K8sResource,
    pub on_close: EventHandler<()>,
}

#[component]
pub fn ResourceDetail(props: ResourceDetailProps) -> Element {
    let resource = &props.resource;

    rsx! {
        div {
            class: "fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50",
            onclick: move |_| props.on_close.call(()),
            div {
                class: "bg-white rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] overflow-auto",
                onclick: move |e| e.stop_propagation(),
                div {
                    class: "sticky top-0 bg-white border-b px-6 py-4 flex justify-between items-center",
                    h2 { class: "text-xl font-bold", "{resource.name}" }
                    button {
                        class: "text-gray-500 hover:text-gray-700 text-2xl leading-none",
                        onclick: move |_| props.on_close.call(()),
                        "×"
                    }
                }
                div { class: "p-6",
                    div { class: "grid grid-cols-2 gap-4",
                        div { class: "space-y-4",
                            DetailItem { label: "Kind", value: resource.kind.clone() }
                            DetailItem { label: "Namespace", value: resource.namespace.clone().unwrap_or_else(|| "None".to_string()) }
                            DetailItem { label: "APIVersion", value: resource.api_version.clone() }
                            DetailItem { label: "Created", value: resource.creation_timestamp.clone().unwrap_or_else(|| "Unknown".to_string()) }
                        }
                        div { class: "space-y-4",
                            h3 { class: "font-semibold text-gray-700 mb-2", "Labels" }
                            LabelTags { labels: resource.labels.clone() }
                        }
                    }
                    if let Some(uid) = &resource.uid {
                        div { class: "mt-4",
                            DetailItem { label: "UID", value: uid.clone() }
                        }
                    }
                }
            }
        }
    }
}
