//! Generic resource section widget.
//!
//! This is the widget-first fallback for any detail section: it subscribes to
//! the resource's generic detail stream and renders one overview, JSON path,
//! labels/annotations, or manifest section.

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};

use crate::models::dashboard::{ResourceSection, ResourceSectionSource, WidgetSource};
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::components::detail::{error_body, key_values, loading_body, raw_manifest};
use crate::ui::dashboard::pinnable_sized;
use crate::ui::resources::detail::overview_with_prefix;
use crate::ui::resources::detail_common::{json_section, path, string_map};
use crate::view_models::resources::detail::ResourceDetailViewModel;

pub struct ResourceSectionView {
    vm: Entity<ResourceDetailViewModel>,
    id: SharedString,
    section: ResourceSection,
    _vm_sub: Subscription,
}

impl ResourceSectionView {
    fn new(
        vm: Entity<ResourceDetailViewModel>,
        id: SharedString,
        section: ResourceSection,
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
        detail: &crate::models::resources::detail::ResourceDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &self.section {
            ResourceSection::Overview => overview_with_prefix(detail, &self.id, cx),
            ResourceSection::Path { title, path: parts } => {
                let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
                let value = path(&detail.value, &parts);
                let prefix = parts.join(".");
                json_section(title, &prefix, value, cx)
            }
            ResourceSection::Labels => {
                let labels = string_map(
                    detail
                        .value
                        .get("metadata")
                        .and_then(|metadata| metadata.get("labels")),
                );
                key_values("Labels", &labels, cx)
            }
            ResourceSection::Annotations => {
                let annotations = string_map(
                    detail
                        .value
                        .get("metadata")
                        .and_then(|metadata| metadata.get("annotations")),
                );
                key_values("Annotations", &annotations, cx)
            }
            ResourceSection::Manifest => raw_manifest(&detail.raw_json, cx),
        }
    }
}

impl Render for ResourceSectionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.vm.read(cx).state().clone();
        match state {
            ResourceDetailState::Closed => div().into_any_element(),
            ResourceDetailState::Loading { .. } => loading_body(
                "Loading section…",
                "Reading the object from the API server.",
                cx,
            ),
            ResourceDetailState::Error { message, .. } => {
                error_body("Could not load this section", &message, cx)
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

pub struct ResourceSectionWidget {
    vm: Entity<ResourceDetailViewModel>,
    view: Entity<ResourceSectionView>,
    id: SharedString,
    source: ResourceSectionSource,
}

impl ResourceSectionWidget {
    pub fn mount(id: impl Into<SharedString>, source: ResourceSectionSource, cx: &mut App) -> Self {
        let id = id.into();
        let vm = cx.new(ResourceDetailViewModel::new);
        vm.update(cx, |vm, cx| {
            vm.open(
                source.kind,
                source.namespace.clone(),
                source.name.clone(),
                cx,
            );
        });
        let view = cx
            .new(|cx| ResourceSectionView::new(vm.clone(), id.clone(), source.section.clone(), cx));
        Self {
            vm,
            view,
            id,
            source,
        }
    }

    pub fn element(&self, _cx: &App) -> AnyElement {
        pinnable_sized(
            format!("{}-menu", self.id),
            self.view.clone().into_any_element(),
            WidgetSource::Section(self.source.clone()),
        )
    }

    pub fn set_active(&self, _active: bool, _cx: &mut App) {}

    pub fn shutdown(&self, cx: &mut App) {
        self.vm.update(cx, |vm, cx| vm.close(cx));
    }
}

/// Build a section source descriptor for classic-mode right-click pinning.
pub fn section_source(
    kind: crate::models::ResourceKind,
    namespace: &str,
    name: &str,
    section: ResourceSection,
) -> crate::models::dashboard::WidgetSource {
    crate::models::dashboard::WidgetSource::Section(ResourceSectionSource {
        kind,
        namespace: namespace.to_string(),
        name: name.to_string(),
        section,
    })
}
