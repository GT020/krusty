use crate::components::labels::LabelsDisplay;
use crate::k8s::client::K8sResource;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ResourceRowProps {
    pub resource: K8sResource,
    pub index: usize,
    pub on_select: EventHandler<K8sResource>,
}

#[component]
pub fn ResourceRow(props: ResourceRowProps) -> Element {
    let row_class = if props.index % 2 == 0 {
        "bg-white"
    } else {
        "bg-gray-50"
    };

    rsx! {
        tr {
            class: "border-t border-gray-100 hover:bg-blue-50 cursor-pointer transition-colors {row_class}",
            onclick: move |_| props.on_select.call(props.resource.clone()),
            td { class: "p-3",
                div { class: "font-medium text-blue-600", "{props.resource.name}" }
            }
            td { class: "p-3 text-gray-600", "{props.resource.namespace.clone().unwrap_or_default()}" }
            td { class: "p-3 hidden md:table-cell",
                LabelsDisplay { labels: props.resource.labels.clone() }
            }
            td { class: "p-3 text-gray-500 text-sm", "{props.resource.creation_timestamp.clone().unwrap_or_default()}" }
        }
    }
}
