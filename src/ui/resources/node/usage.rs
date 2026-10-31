//! Node usage widget.
//!
//! The same concrete widget renders in node detail and on the dashboard. Its
//! view model subscribes only to this node's metrics.

use gpui_kit::base::v_flex;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _,
};

use crate::models::dashboard::{NodeWidgetSource, WidgetSource};
use crate::ui::components::detail::{section_with_app, usage_body};
use crate::ui::dashboard::pinnable;
use crate::view_models::resources::node::usage::NodeUsageViewModel;

pub struct NodeUsageArgs {
    pub node: String,
    pub id_prefix: SharedString,
    pub compact: bool,
    pub show_header: bool,
}

pub struct NodeUsageWidget {
    vm: Entity<NodeUsageViewModel>,
    node: String,
    id_prefix: SharedString,
    compact: bool,
    show_header: bool,
}

impl NodeUsageWidget {
    pub fn mount(args: NodeUsageArgs, cx: &mut App) -> Self {
        let vm = cx.new(|cx| NodeUsageViewModel::new(args.node.clone(), cx));
        Self {
            vm,
            node: args.node,
            id_prefix: args.id_prefix,
            compact: args.compact,
            show_header: args.show_header,
        }
    }

    pub fn element(&self, cx: &App) -> AnyElement {
        let vm = self.vm.read(cx);
        let body = usage_body(
            &self.id_prefix,
            vm.history(),
            vm.error(),
            "allocatable",
            self.compact,
            cx,
        );
        let content = if self.show_header {
            section_with_app("Usage", body, cx)
        } else {
            v_flex()
                .size_full()
                .overflow_y_scrollbar()
                .p_3()
                .child(body)
                .into_any_element()
        };
        pinnable(
            format!("node-usage-menu-{}", self.id_prefix),
            content,
            WidgetSource::Node(NodeWidgetSource::Usage {
                name: self.node.clone(),
            }),
        )
    }

    pub fn set_active(&self, active: bool, cx: &mut App) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }

    pub fn shutdown(&self, cx: &mut App) {
        self.vm.update(cx, |vm, _| vm.shutdown());
    }
}
