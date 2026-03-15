use iced::widget::{column, container, scrollable, text, row};
use iced::{Element, Length};
use k8s_openapi::api::core::v1::Event;
use crate::app::Message;

pub fn view(events: &[Event]) -> Element<Message> {
    if events.is_empty() {
        return container(text("No Events found").size(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let mut content = column![
        row![
            text("Type").width(Length::FillPortion(1)).size(16),
            text("Reason").width(Length::FillPortion(2)).size(16),
            text("Message").width(Length::FillPortion(4)).size(16),
            text("Source").width(Length::FillPortion(1)).size(16),
        ]
        .spacing(20)
        .padding(10)
    ]
    .spacing(2);

    for event in events {
        let ev_type = event.type_.as_deref().unwrap_or("<unknown>");
        let reason = event.reason.as_deref().unwrap_or("<unknown>");
        let message = event.message.as_deref().unwrap_or("");
        let source = event.source.as_ref().and_then(|s| s.component.as_deref()).unwrap_or("<unknown>");
        
        content = content.push(
            row![
                text(ev_type).width(Length::FillPortion(1)),
                text(reason).width(Length::FillPortion(2)),
                text(message).width(Length::FillPortion(4)),
                text(source).width(Length::FillPortion(1)),
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
