use crate::ui::widgets::graph_widget::{cpu_usage_graph_widget, memory_usage_graph_widget};
use crate::view_models::pod::{Message, PodViewModel};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

pub fn view(vm: &PodViewModel) -> Element<Message> {
    if vm.loading && vm.items.is_empty() {
        return container(text("Loading Pods...").size(20))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let mut list = column![row![
        text("Namespace").width(Length::FillPortion(1)).size(16),
        text("Name").width(Length::FillPortion(2)).size(16),
        text("Status").width(Length::FillPortion(1)).size(16),
    ]
    .spacing(10)
    .padding(10)]
    .spacing(5);

    for (i, item) in vm.items.iter().enumerate() {
        let content = row![
            text(&item.namespace).width(Length::FillPortion(1)),
            text(&item.name).width(Length::FillPortion(2)),
            text(&item.status).width(Length::FillPortion(1)),
        ]
        .spacing(10);

        let btn = button(content)
            .width(Length::Fill)
            .on_press(Message::Select(i));
        if Some(i) == vm.selected_index {
            // we could style differently if iced supported easy active states, rely on click state
        }
        list = list.push(btn);
    }

    let left_pane = scrollable(container(list).padding(10)).width(Length::FillPortion(2));

    let right_pane: Element<Message> = if let Some(idx) = vm.selected_index {
        if let Some(item) = vm.items.get(idx) {
            let delete_btn = button("Delete Resource").on_press(Message::Delete);
            let logs_btn = button("View Logs").on_press(Message::ViewLogs);
            let clear_logs_btn = button("Clear Logs").on_press(Message::ClearLogs);

            let details = if vm.logs_loading {
                scrollable(text("Loading logs...").size(14)).height(Length::Fill)
            } else if let Some(ref logs) = vm.logs {
                scrollable(text(logs).size(12)).height(Length::FillPortion(3))
            } else {
                scrollable(text(&item.raw).size(14)).height(Length::Fill)
            };

            // CPU and Memory usage graphs
            let cpu_graph = if !vm.cpu_metrics.is_empty() {
                let widget = cpu_usage_graph_widget(vm.cpu_metrics.clone());
                let element = widget.view().map(|_| Message::Tick);
                container(element)
                    .width(Length::Fill)
                    .height(Length::FillPortion(2))
            } else {
                container(text("No CPU data available").size(12))
                    .width(Length::Fill)
                    .height(Length::FillPortion(2))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
            };

            let memory_graph = if !vm.memory_metrics.is_empty() {
                let widget = memory_usage_graph_widget(vm.memory_metrics.clone());
                let element = widget.view().map(|_| Message::Tick);
                container(element)
                    .width(Length::Fill)
                    .height(Length::FillPortion(2))
            } else {
                container(text("No memory data available").size(12))
                    .width(Length::Fill)
                    .height(Length::FillPortion(2))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
            };

            let graphs = column![text("Resource Usage").size(16), cpu_graph, memory_graph]
                .spacing(10)
                .width(Length::Fill)
                .height(Length::FillPortion(2));

            let action_btns = if vm.logs.is_some() {
                row![delete_btn, logs_btn, clear_logs_btn].spacing(10)
            } else {
                row![delete_btn, logs_btn].spacing(10)
            };

            column![
                text(format!("Details for {}", item.name)).size(20),
                action_btns,
                details,
                graphs
            ]
            .spacing(20)
            .width(Length::FillPortion(3))
            .padding(10)
            .into()
        } else {
            container(text("Select an item"))
                .width(Length::FillPortion(3))
                .into()
        }
    } else {
        container(text("Select an item to view properties"))
            .width(Length::FillPortion(3))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    row![left_pane, right_pane].spacing(20).into()
}
