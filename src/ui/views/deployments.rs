use iced::{Element, Length};
use iced::widget::{column, scrollable, text};
use k8s_openapi::api::apps::v1::Deployment;
use crate::app::Message;

pub fn view(deployments: &[Deployment]) -> Element<Message> {
    let mut col = column![text("Deployments").size(24)].spacing(10);
    
    if deployments.is_empty() {
        col = col.push(text("No deployments found or loading..."));
    } else {
        for def in deployments {
            let name = def.metadata.name.as_deref().unwrap_or("Unknown");
            let ns = def.metadata.namespace.as_deref().unwrap_or("default");
            let replicas = def.status.as_ref().and_then(|s| s.ready_replicas).unwrap_or(0);
            
            col = col.push(text(format!("{} | {} | {} replicas", ns, name, replicas)));
        }
    }
    
    scrollable(col).width(Length::Fill).height(Length::Fill).into()
}
