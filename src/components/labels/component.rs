use dioxus::prelude::*;

#[component]
pub fn LabelsDisplay(labels: std::collections::BTreeMap<String, String>) -> Element {
    if labels.is_empty() {
        rsx! { span { class: "text-gray-400 text-sm", "-" } }
    } else {
        rsx! {
            div { class: "flex flex-wrap gap-1",
                for (key, value) in labels.iter().take(3) {
                    span {
                        class: "bg-gray-100 text-gray-700 px-2 py-0.5 rounded text-xs",
                        "{key}: {value}"
                    }
                }
                if labels.len() > 3 {
                    span { class: "text-gray-500 text-xs", "+{labels.len() - 3}" }
                }
            }
        }
    }
}

#[component]
pub fn LabelTags(labels: std::collections::BTreeMap<String, String>) -> Element {
    if labels.is_empty() {
        rsx! { div { class: "text-gray-400 text-sm", "No labels" } }
    } else {
        rsx! {
            div { class: "flex flex-wrap gap-2",
                for (key, value) in labels.iter() {
                    span { class: "bg-blue-100 text-blue-800 px-2 py-1 rounded text-sm", "{key}: {value}" }
                }
            }
        }
    }
}
