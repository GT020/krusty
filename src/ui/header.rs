use crate::app::Message;
use iced::widget::{button, pick_list, row, text};
use iced::{Element, Length};

pub fn view<'a>(
    title: &'a str,
    namespace: &'a Option<String>,
    namespaces: &'a [String],
    is_loading: bool,
) -> Element<'a, Message> {
    let mut all_ns = vec!["All Namespaces".to_string()];
    all_ns.extend(namespaces.iter().cloned());

    let selected = match namespace {
        None => Some("All Namespaces".to_string()),
        Some(n) if n.is_empty() => Some("All Namespaces".to_string()),
        Some(n) => Some(n.clone()),
    };

    let namespace_picker = pick_list(all_ns, selected, |ns| {
        if ns == "All Namespaces" {
            Message::NamespaceChanged(None)
        } else {
            Message::NamespaceChanged(Some(ns))
        }
    })
    .width(Length::Fixed(180.0));

    let refresh_btn = if is_loading {
        button(text("⟳").size(20)).on_press(Message::RefreshRequested)
    } else {
        button(text("↻").size(20)).on_press(Message::RefreshRequested)
    };

    row![text(title).size(24), namespace_picker, refresh_btn,]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .into()
}
