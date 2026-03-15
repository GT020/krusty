use iced::{Element, Length};
use iced::widget::{column, scrollable, text, row, button, container};
use crate::view_models::service::{ServiceViewModel, Message};

pub fn view(vm: &ServiceViewModel) -> Element<Message> {
    if vm.loading && vm.items.is_empty() {
        return container(text("Loading Services...").size(20)).center_x(Length::Fill).center_y(Length::Fill).into();
    }
    
    let mut list = column![
        row![
            text("Namespace").width(Length::FillPortion(1)).size(16),
            text("Name").width(Length::FillPortion(2)).size(16),
            text("Type").width(Length::FillPortion(1)).size(16),
        ].spacing(10).padding(10)
    ].spacing(5);
    
    for (i, item) in vm.items.iter().enumerate() {
        let content = row![
            text(&item.namespace).width(Length::FillPortion(1)),
            text(&item.name).width(Length::FillPortion(2)),
            text(&item.status).width(Length::FillPortion(1)),
        ].spacing(10);
        
        let btn = button(content).width(Length::Fill).on_press(Message::Select(i));
        list = list.push(btn);
    }
    
    let left_pane = scrollable(container(list).padding(10)).width(Length::FillPortion(2));
    
    let right_pane: Element<Message> = if let Some(idx) = vm.selected_index {
        if let Some(item) = vm.items.get(idx) {
            let details = scrollable(text(&item.raw).size(14)).height(Length::Fill);
            let delete_btn = button("Delete Resource").on_press(Message::Delete);
            column![
                text(format!("Details for {}", item.name)).size(20),
                delete_btn,
                details
            ].spacing(20).width(Length::FillPortion(3)).padding(10).into()
        } else {
            container(text("Select an item")).width(Length::FillPortion(3)).into()
        }
    } else {
        container(text("Select an item to view properties"))
            .width(Length::FillPortion(3)).center_x(Length::Fill).center_y(Length::Fill).into()
    };
    
    row![left_pane, right_pane].spacing(20).into()
}
