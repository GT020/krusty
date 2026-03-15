use iced::widget::{column, container, scrollable, text, row};
use iced::{Element, Length};
use kube::api::DynamicObject;
use crate::app::Message;

pub fn view(routes: &[DynamicObject]) -> Element<Message> {
    if routes.is_empty() {
        return container(text("No IngressRoutes found").size(20))
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
            text("Age").width(Length::FillPortion(1)).size(16),
        ]
        .spacing(20)
        .padding(10)
    ]
    .spacing(2);

    for route in routes {
        let name = route.metadata.name.as_deref().unwrap_or("<unknown>");
        let ns = route.metadata.namespace.as_deref().unwrap_or("default");
        let creation = route.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()).unwrap_or_else(|| "".to_string());
        
        content = content.push(
            row![
                text(name).width(Length::FillPortion(2)),
                text(ns).width(Length::FillPortion(1)),
                text(creation).width(Length::FillPortion(1)),
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
