use iced::{Element, Length};
use iced::widget::{column, scrollable, text};
use k8s_openapi::api::core::v1::Pod;
use crate::app::Message;

pub fn view(pods: &[Pod]) -> Element<Message> {
    let mut col = column![text("Pods").size(24)].spacing(10);
    
    if pods.is_empty() {
        col = col.push(text("No pods found or loading..."));
    } else {
        for pod in pods {
            let name = pod.metadata.name.as_deref().unwrap_or("Unknown");
            let ns = pod.metadata.namespace.as_deref().unwrap_or("default");
            let status = pod.status.as_ref().and_then(|s| s.phase.as_deref()).unwrap_or("Unknown");
            
            col = col.push(text(format!("{} | {} | {}", ns, name, status)));
        }
    }
    
    scrollable(col).width(Length::Fill).height(Length::Fill).into()
}
