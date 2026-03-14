use iced::{Element, Length};
use iced::widget::{column, scrollable, text};
use k8s_openapi::api::core::v1::Node;
use crate::app::Message;

pub fn view(nodes: &[Node]) -> Element<Message> {
    let mut col = column![text("Nodes").size(24)].spacing(10);
    
    if nodes.is_empty() {
        col = col.push(text("No nodes found or loading..."));
    } else {
        for node in nodes {
            let name = node.metadata.name.as_deref().unwrap_or("Unknown");
            col = col.push(text(format!("Node: {}", name)));
        }
    }
    
    scrollable(col).width(Length::Fill).height(Length::Fill).into()
}
