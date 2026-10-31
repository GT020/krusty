//! Pod usage widget.
//!
//! This is the same concrete widget in classic pod detail and on the dashboard.
//! It owns a `PodUsageViewModel`, which subscribes only to this pod's metrics.

use gpui_kit::base::v_flex;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _,
};

use crate::models::dashboard::{PodWidgetSource, WidgetSource};
use crate::ui::components::detail::{section_with_app, usage_body};
use crate::ui::dashboard::pinnable;
use crate::view_models::resources::pod::usage::PodUsageViewModel;

pub struct PodUsageArgs {
    pub namespace: String,
    pub name: String,
    pub id_prefix: SharedString,
    pub compact: bool,
    pub show_header: bool,
}

pub struct PodUsageWidget {
    vm: Entity<PodUsageViewModel>,
    namespace: String,
    name: String,
    id_prefix: SharedString,
    compact: bool,
    show_header: bool,
}

impl PodUsageWidget {
    pub fn mount(args: PodUsageArgs, cx: &mut App) -> Self {
        let vm = cx.new(|cx| PodUsageViewModel::new(args.namespace.clone(), args.name.clone(), cx));
        Self {
            vm,
            namespace: args.namespace,
            name: args.name,
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
            "",
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
            format!("pod-usage-menu-{}", self.id_prefix),
            content,
            WidgetSource::Pod(PodWidgetSource::Usage {
                namespace: self.namespace.clone(),
                name: self.name.clone(),
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
