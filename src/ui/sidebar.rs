use crate::app::{Message, Route};
use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length, Color, Background};

pub fn view(current_route: &Route) -> Element<Message> {
    let make_btn = |label: String, route: Route, is_active: bool| -> Element<Message> {
        let mut b = button(text(label.clone()).size(16)).width(Length::Fill);
        
        b = b.style(move |_, status| {
            use iced::widget::button::Status;
            let (bg_color, text_color) = if is_active {
                // #9B8EC7
                (Color::from_rgb8(155, 142, 199), Color::WHITE)
            } else {
                match status {
                    Status::Hovered | Status::Pressed => {
                        (Color::from_rgb8(155, 142, 199), Color::WHITE)
                    }
                    _ => {
                        // #BDA6CE
                        (Color::from_rgb8(189, 166, 206), Color::BLACK)
                    }
                }
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg_color)),
                text_color,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
            }
        });

        if !is_active {
            b = b.on_press(Message::RouteChanged(route));
        }
        b.into()
    };

    let col = column![
        text("Krusty").size(24),
        make_btn(
            "Pods".to_string(),
            Route::Pods,
            *current_route == Route::Pods
        ),
        make_btn(
            "Nodes".to_string(),
            Route::Nodes,
            *current_route == Route::Nodes
        ),
        make_btn(
            "Deployments".to_string(),
            Route::Deployments,
            *current_route == Route::Deployments
        ),
        make_btn(
            "Secrets".to_string(),
            Route::Secrets,
            *current_route == Route::Secrets
        ),
        make_btn(
            "Events".to_string(),
            Route::Events,
            *current_route == Route::Events
        ),
        make_btn(
            "Services".to_string(),
            Route::Services,
            *current_route == Route::Services
        ),
        make_btn(
            "Ingress".to_string(),
            Route::Ingress,
            *current_route == Route::Ingress
        ),
        make_btn(
            "Settings".to_string(),
            Route::Settings,
            *current_route == Route::Settings
        ),
    ]
    .spacing(10);

    container(scrollable(col))
        .width(Length::Fixed(200.0))
        .height(Length::Fill)
        .padding(20)
        .style(|_| iced::widget::container::Style {
            // #B4D3D9
            background: Some(Background::Color(Color::from_rgb8(180, 211, 217))),
            text_color: Some(Color::BLACK),
            ..Default::default()
        })
        .into()
}
