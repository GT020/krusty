use iced::widget::{column, container, scrollable, text, row};
use iced::{Element, Length};
use k8s_openapi::api::core::v1::Service;
use crate::app::Message;

pub fn view(services: &[Service]) -> Element<Message> {
    if services.is_empty() {
        return container(text("No Services found").size(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let mut content = column![
        row![
            text("Name").width(Length::FillPortion(2)).size(16),
            text("Namespace").width(Length::FillPortion(1)).size(16),
            text("Type").width(Length::FillPortion(1)).size(16),
            text("Cluster IP").width(Length::FillPortion(2)).size(16),
        ]
        .spacing(20)
        .padding(10)
    ]
    .spacing(2);

    for svc in services {
        let name = svc.metadata.name.as_deref().unwrap_or("<unknown>");
        let ns = svc.metadata.namespace.as_deref().unwrap_or("default");
        
        let (svc_type, cluster_ip) = if let Some(spec) = &svc.spec {
            (
                spec.type_.as_deref().unwrap_or("ClusterIP"),
                spec.cluster_ip.as_deref().unwrap_or("<none>")
            )
        } else {
            ("<unknown>", "<unknown>")
        };
        
        content = content.push(
            row![
                text(name).width(Length::FillPortion(2)),
                text(ns).width(Length::FillPortion(1)),
                text(svc_type).width(Length::FillPortion(1)),
                text(cluster_ip).width(Length::FillPortion(2)),
            ]
            .spacing(20)
            .padding(10)
        );
    }

    scrollable(
        container(content)
            .width(Length::Fill)
            .padding(10)
    ).into()
}
