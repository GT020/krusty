use iced::widget::{column, text, container, slider, row, radio, Space};
use iced::{Element, Length};
use crate::view_models::settings::{Message, SettingsViewModel};
use crate::models::settings::FetchMode;

pub fn view<'a>(vm: &'a SettingsViewModel, namespaces: &'a [String]) -> Element<'a, Message> {
    let main_col = column![
        text("Settings").size(32),
        Space::with_height(20),
        
        text(format!("Global Refresh Interval: {} seconds", vm.settings.refresh_interval)),
        slider(1.0..=60.0, vm.settings.refresh_interval as f64, |v| Message::IntervalChanged(v as u64))
            .width(Length::Fixed(300.0)),
            
        Space::with_height(30),
        text("Fetch Mode Overrides (per Resource)").size(20),
        text("Configure whether a specific resource uses a live Watcher or standard Polling.").size(14),
        Space::with_height(10),
    ].spacing(10);
    
    // Build a matrix of checkboxes for Resources and Namespaces
    let resources = ["Pods", "Nodes", "Deployments", "Secrets", "Events", "Services", "Ingress"];
    
    let mut overrides_col = column![].spacing(15);
    for res in resources {
        let current_mode = vm.settings.get_fetch_mode(res);
        let poll_radio = radio(
            "Poll",
            FetchMode::Polling,
            Some(current_mode.clone()),
            move |mode| Message::SetFetchMode(res.to_string(), mode)
        );
        let watch_radio = radio(
            "Watch",
            FetchMode::Watcher,
            Some(current_mode.clone()),
            move |mode| Message::SetFetchMode(res.to_string(), mode)
        );
        
        let row_items = row![
            text(res).width(Length::Fixed(150.0)),
            poll_radio,
            watch_radio
        ].spacing(20);
        overrides_col = overrides_col.push(row_items);
    }

    container(column![main_col, overrides_col])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .into()
}
