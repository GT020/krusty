//! Rich deployment detail section widget.
//!
//! Reuses the same section builders as the classic deployment detail pane.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};

use crate::models::ResourceKind;
use crate::models::dashboard::{DeploymentSection, DeploymentWidgetSource, WidgetSource};
use crate::models::resources::detail::ResourceDetail;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::components::detail::{error_body, key_values, loading_body, raw_manifest};
use crate::ui::dashboard::pinnable_sized;
use crate::ui::resources::deployments::detail::{
    conditions, overview, pod_template, selector, strategy,
};
use crate::ui::resources::detail_common::string_map;
use crate::view_models::resources::detail::ResourceDetailViewModel;

pub struct DeploymentSectionView {
    vm: Entity<ResourceDetailViewModel>,
    id: SharedString,
    section: DeploymentSection,
    _vm_sub: Subscription,
}

impl DeploymentSectionView {
    fn new(
        vm: Entity<ResourceDetailViewModel>,
        id: SharedString,
        section: DeploymentSection,
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

    fn section_element(&self, detail: &ResourceDetail, cx: &mut Context<Self>) -> AnyElement {
        match self.section {
            DeploymentSection::Overview => overview(detail, cx),
            DeploymentSection::Selector => selector(detail, cx),
            DeploymentSection::Strategy => strategy(detail, cx),
            DeploymentSection::Conditions => conditions(detail, cx),
            DeploymentSection::PodTemplate => pod_template(detail, cx),
            DeploymentSection::Labels => {
                let labels = string_map(
                    detail
                        .value
                        .get("metadata")
                        .and_then(|metadata| metadata.get("labels")),
                );
                key_values("Labels", &labels, cx)
            }
            DeploymentSection::Annotations => {
                let annotations = string_map(
                    detail
                        .value
                        .get("metadata")
                        .and_then(|metadata| metadata.get("annotations")),
                );
                key_values("Annotations", &annotations, cx)
            }
            DeploymentSection::Manifest => raw_manifest(&detail.raw_json, cx),
        }
    }
}

impl Render for DeploymentSectionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.vm.read(cx).state().clone();
        match state {
            ResourceDetailState::Closed => div().into_any_element(),
            ResourceDetailState::Loading { .. } => loading_body(
                "Loading deployment section…",
                "Reading the deployment from the API server.",
                cx,
            ),
            ResourceDetailState::Error { message, .. } => {
                error_body("Could not load this deployment section", &message, cx)
            }
            ResourceDetailState::Ready(detail) => {
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

pub struct DeploymentSectionWidget {
    vm: Entity<ResourceDetailViewModel>,
    view: Entity<DeploymentSectionView>,
    id: SharedString,
    namespace: String,
    name: String,
    section: DeploymentSection,
}

impl DeploymentSectionWidget {
    pub fn mount(
        id: impl Into<SharedString>,
        namespace: String,
        name: String,
        section: DeploymentSection,
        cx: &mut App,
    ) -> Self {
        let id = id.into();
        let vm = cx.new(ResourceDetailViewModel::new);
        vm.update(cx, |vm, cx| {
            vm.open(
                ResourceKind::Deployments,
                namespace.clone(),
                name.clone(),
                cx,
            );
        });
        let view = cx.new(|cx| DeploymentSectionView::new(vm.clone(), id.clone(), section, cx));
        Self {
            vm,
            view,
            id,
            namespace,
            name,
            section,
        }
    }

    pub fn element(&self, _cx: &App) -> AnyElement {
        pinnable_sized(
            format!("{}-menu", self.id),
            self.view.clone().into_any_element(),
            WidgetSource::Deployment(DeploymentWidgetSource::Section {
                namespace: self.namespace.clone(),
                name: self.name.clone(),
                section: self.section,
            }),
        )
    }

    pub fn set_active(&self, _active: bool, _cx: &mut App) {}

    pub fn shutdown(&self, cx: &mut App) {
        self.vm.update(cx, |vm, cx| vm.close(cx));
    }
}
