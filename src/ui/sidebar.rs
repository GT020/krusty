use iced::{Element, Length};
use iced::widget::{button, column, container, text};
use crate::app::{Message, Route};

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
        make_btn("Pods".to_string(), Route::Pods, *current_route == Route::Pods),
        make_btn("Nodes".to_string(), Route::Nodes, *current_route == Route::Nodes),
        make_btn("Deployments".to_string(), Route::Deployments, *current_route == Route::Deployments),
    ].spacing(10);

    container(col)
        .width(Length::Fixed(200.0))
        .height(Length::Fill)
        .padding(20)
        .into()
}
