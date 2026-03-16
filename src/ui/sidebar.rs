use crate::app::{Message, Route};
use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};

pub fn view(current_route: &Route) -> Element<Message> {
    let make_btn = |label: String, route: Route, is_active: bool| -> Element<Message> {
        let mut b = button(text(label.clone()).size(16)).width(Length::Fill);
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
    ]
    .spacing(10);

    container(scrollable(col))
        .width(Length::Fixed(200.0))
        .height(Length::Fill)
        .padding(20)
        .into()
}
