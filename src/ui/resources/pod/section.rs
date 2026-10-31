//! Rich pod detail section widget.
//!
//! Reuses the same section renderers as the classic pod detail pane.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};

use crate::models::dashboard::{PodSection, PodWidgetSource, WidgetSource};
use crate::models::resources::pod::detail::PodDetailState;
use crate::ui::components::detail::{error_body, key_values, loading_body, raw_manifest};
use crate::ui::dashboard::pinnable_sized;
use crate::ui::resources::pod::detail::{
    conditions, containers, overview, owners, tolerations, volumes,
};
use crate::view_models::resources::pod::section::PodSectionViewModel;

pub struct PodSectionView {
    vm: Entity<PodSectionViewModel>,
    id: SharedString,
    section: PodSection,
    _vm_sub: Subscription,
}

impl PodSectionView {
    fn new(
        vm: Entity<PodSectionViewModel>,
        id: SharedString,
        section: PodSection,
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
        detail: &crate::models::resources::pod::detail::PodDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.section {
            PodSection::Overview => overview(detail, cx),
            PodSection::Containers => containers("Containers", &detail.containers, cx),
            PodSection::InitContainers => {
                containers("Init containers", &detail.init_containers, cx)
            }
            PodSection::Conditions => conditions(detail, cx),
            PodSection::Volumes => volumes(detail, cx),
            PodSection::Tolerations => tolerations(detail, cx),
            PodSection::Owners => owners(detail, cx),
            PodSection::Labels => key_values("Labels", &detail.labels, cx),
            PodSection::Annotations => key_values("Annotations", &detail.annotations, cx),
            PodSection::Manifest => raw_manifest(&detail.raw_json, cx),
        }
    }
}

impl Render for PodSectionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.vm.read(cx).state().clone();
        match state {
            PodDetailState::Closed => div().into_any_element(),
            PodDetailState::Loading { .. } => loading_body(
                "Loading pod section…",
                "Reading the pod from the API server.",
                cx,
            ),
            PodDetailState::Error { message, .. } => {
                error_body("Could not load this pod section", &message, cx)
            }
            PodDetailState::Ready(detail) => {
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

pub struct PodSectionWidget {
    vm: Entity<PodSectionViewModel>,
    view: Entity<PodSectionView>,
    id: SharedString,
    namespace: String,
    name: String,
    section: PodSection,
}

impl PodSectionWidget {
    pub fn mount(
        id: impl Into<SharedString>,
        namespace: String,
        name: String,
        section: PodSection,
        cx: &mut App,
    ) -> Self {
        let id = id.into();
        let vm = cx.new(|cx| PodSectionViewModel::new(namespace.clone(), name.clone(), cx));
        let view = cx.new(|cx| PodSectionView::new(vm.clone(), id.clone(), section, cx));
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
            WidgetSource::Pod(PodWidgetSource::Section {
                namespace: self.namespace.clone(),
                name: self.name.clone(),
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
