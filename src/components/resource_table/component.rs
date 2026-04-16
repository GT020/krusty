use crate::components::resource_row::ResourceRow;
use crate::k8s::client::K8sResource;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ResourceTableProps {
    pub resources: Vec<K8sResource>,
    pub on_select: EventHandler<K8sResource>,
}

#[component]
pub fn ResourceTable(props: ResourceTableProps) -> Element {
    let resources = props.resources.clone();
    let on_select = props.on_select;

    if resources.is_empty() {
        return rsx! { div { class: "text-gray-500 text-center py-8", "No resources found" } };
    }

    rsx! {
        div { class: "bg-white rounded-lg shadow overflow-hidden",
            table { class: "w-full",
                thead {
                    tr { class: "bg-gray-50",
                        th { class: "text-left p-3 text-sm font-semibold text-gray-600", "Name" }
                        th { class: "text-left p-3 text-sm font-semibold text-gray-600", "Namespace" }
                        th { class: "text-left p-3 text-sm font-semibold text-gray-600 hidden md:table-cell", "Labels" }
                        th { class: "text-left p-3 text-sm font-semibold text-gray-600", "Age" }
                    }
                }
                tbody {
                    for (i, resource) in resources.iter().enumerate() {
                        ResourceRow {
                            resource: resource.clone(),
                            index: i,
                            on_select: on_select.clone()
                        }
                    }
                }
            }
        }
    }
}
