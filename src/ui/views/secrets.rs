use iced::widget::{column, container, scrollable, text, row};
use iced::{Element, Length};
use k8s_openapi::api::core::v1::Secret;
use crate::app::Message;

pub fn view(secrets: &[Secret]) -> Element<Message> {
    if secrets.is_empty() {
        return container(text("No Secrets found").size(20))
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
        ]
        .spacing(20)
        .padding(10)
    ]
    .spacing(2);

    for secret in secrets {
        let name = secret.metadata.name.as_deref().unwrap_or("<unknown>");
        let ns = secret.metadata.namespace.as_deref().unwrap_or("default");
        let secret_type = secret.type_.as_deref().unwrap_or("<unknown>");
        
        content = content.push(
            row![
                text(name).width(Length::FillPortion(2)),
                text(ns).width(Length::FillPortion(1)),
                text(secret_type).width(Length::FillPortion(1)),
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
