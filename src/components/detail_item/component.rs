use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct DetailItemProps {
    pub label: String,
    pub value: String,
}

#[component]
pub fn DetailItem(props: DetailItemProps) -> Element {
    rsx! {
        div {
            div { class: "text-sm text-gray-500", "{props.label}" }
            div { class: "font-medium text-gray-900 break-all", "{props.value}" }
        }
    }
}
