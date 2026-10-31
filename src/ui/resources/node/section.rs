//! Rich node detail section widget.
//!
//! Reuses the same section renderers as the classic node detail pane.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};

use crate::models::dashboard::{NodeSection, NodeWidgetSource, WidgetSource};
use crate::models::resources::node::detail::NodeDetailState;
use crate::ui::components::detail::{error_body, key_values, loading_body, raw_manifest};
use crate::ui::dashboard::pinnable_sized;
use crate::ui::resources::node::detail::{
    addresses, capacity, conditions, images, overview, system, taints,
};
use crate::view_models::resources::node::section::NodeSectionViewModel;

pub struct NodeSectionView {
    vm: Entity<NodeSectionViewModel>,
    id: SharedString,
    section: NodeSection,
    _vm_sub: Subscription,
}

impl NodeSectionView {
    fn new(
        vm: Entity<NodeSectionViewModel>,
        id: SharedString,
        section: NodeSection,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        Self {
            vm,
            id,
            section,
            _vm_sub: subscription,
        }
    }

    fn section_element(
        &self,
        detail: &crate::models::resources::node::detail::NodeDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.section {
            NodeSection::Overview => overview(detail, cx),
            NodeSection::Capacity => capacity(detail, cx),
            NodeSection::Conditions => conditions(detail, cx),
            NodeSection::System => system(detail, cx),
            NodeSection::Addresses => addresses(detail, cx),
            NodeSection::Taints => taints(detail, cx),
            NodeSection::Labels => key_values("Labels", &detail.labels, cx),
            NodeSection::Annotations => key_values("Annotations", &detail.annotations, cx),
            NodeSection::Images => images(detail, cx),
            NodeSection::Manifest => raw_manifest(&detail.raw_json, cx),
        }
    }
}

impl Render for NodeSectionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.vm.read(cx).state().clone();
        match state {
            NodeDetailState::Closed => div().into_any_element(),
            NodeDetailState::Loading { .. } => loading_body(
                "Loading node section…",
                "Reading the node from the API server.",
                cx,
            ),
            NodeDetailState::Error { message, .. } => {
                error_body("Could not load this node section", &message, cx)
            }
            NodeDetailState::Ready(detail) => {
                let content = self.section_element(&detail, cx);
                div()
                    .id(self.id.clone())
                    .size_full()
                    .min_w_0()
                    .overflow_y_scrollbar()
                    .p_3()
                    .child(content)
                    .into_any_element()
            }
        }
    }
}

pub struct NodeSectionWidget {
    vm: Entity<NodeSectionViewModel>,
    view: Entity<NodeSectionView>,
    id: SharedString,
    node: String,
    section: NodeSection,
}

impl NodeSectionWidget {
    pub fn mount(
        id: impl Into<SharedString>,
        node: String,
        section: NodeSection,
        cx: &mut App,
    ) -> Self {
        let id = id.into();
        let vm = cx.new(|cx| NodeSectionViewModel::new(node.clone(), cx));
        let view = cx.new(|cx| NodeSectionView::new(vm.clone(), id.clone(), section, cx));
        Self {
            vm,
            view,
            id,
            node,
            section,
        }
    }

    pub fn element(&self, _cx: &App) -> AnyElement {
        pinnable_sized(
            format!("{}-menu", self.id),
            self.view.clone().into_any_element(),
            WidgetSource::Node(NodeWidgetSource::Section {
                name: self.node.clone(),
                section: self.section,
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
