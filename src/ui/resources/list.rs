//! Generic resource list view: header, virtualized table, and detail pane.

use crate::models::{NamespaceScope, ResourceKind, SnapshotPhase};
use crate::ui::resources::namespace_filter::{
    NamespaceFilter, NamespaceFilterEvent, render as render_namespace_filter,
};
use crate::view_models::ResourceViewModel;
use crate::view_models::resources::deployments::DeploymentsDetailViewModel;
use crate::view_models::resources::detail::ResourceDetailViewModel;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_resizable, resizable_panel,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, FontWeight, IntoElement, ParentElement as _, Pixels,
    Render, SharedString, Styled as _, Subscription, Window, div, px,
};

const LIST_MIN_WIDTH: Pixels = px(340.);
const LIST_MAX_WIDTH: Pixels = px(900.);
const DETAIL_MIN_WIDTH: Pixels = px(360.);
const SPLIT_MIN_VIEWPORT_WIDTH: f32 = 1080.;

fn list_default_width(viewport_width: f32) -> Pixels {
    if viewport_width >= 1500. {
        px(720.)
    } else if viewport_width >= 1280. {
        px(620.)
    } else {
        px(560.)
    }
}

pub struct ResourceListView {
    kind: ResourceKind,
    title: SharedString,
    vm: ResourceViewModel,
    namespace_filter: gpui_kit::Entity<NamespaceFilter>,
    detail: gpui_kit::Entity<ResourceDetailViewModel>,
    deployment_actions: gpui_kit::Entity<DeploymentsDetailViewModel>,
    _vm_sub: Subscription,
    _namespace_sub: Subscription,
    _detail_sub: Subscription,
    _row_sub: Subscription,
}

impl ResourceListView {
    pub fn new(
        kind: ResourceKind,
        scope: NamespaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let vm = ResourceViewModel::new(kind, scope.clone(), window, cx);
        let vm_sub = vm.observe(cx, |_, cx| cx.notify());
        let namespace_filter = cx.new(|cx| NamespaceFilter::with_scope(scope, window, cx));
        let namespace_sub = cx.subscribe(
            &namespace_filter,
            |this, _, event: &NamespaceFilterEvent, cx| {
                let NamespaceFilterEvent::Changed(scope) = event;
                this.vm.set_scope(scope.clone(), cx);
                this.close_detail(cx);
            },
        );
        let detail = cx.new(ResourceDetailViewModel::new);
        let deployment_actions = cx.new(|_| DeploymentsDetailViewModel::new(detail.downgrade()));
        let detail_sub = cx.observe(&detail, |_, _, cx| cx.notify());
        let detail_handle = detail.clone();
        let row_sub = vm.observe_row_selected(cx, move |this, row, cx| {
            let Some(item) = this.vm.item_at(row, cx) else {
                return;
            };
            detail_handle.update(cx, |detail, cx| {
                detail.open(kind, item.namespace.clone(), item.name.clone(), cx);
            });
        });
        Self {
            kind,
            title: kind.label().into(),
            vm,
            namespace_filter,
            detail,
            deployment_actions,
            _vm_sub: vm_sub,
            _namespace_sub: namespace_sub,
            _detail_sub: detail_sub,
            _row_sub: row_sub,
        }
    }

    pub fn scope_label(&self, cx: &App) -> String {
        self.vm.scope(cx).label()
    }

    pub fn namespace_filter(&self) -> &gpui_kit::Entity<NamespaceFilter> {
        &self.namespace_filter
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.set_active(active, cx);
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.detail.update(cx, |detail, cx| detail.close(cx));
    }

    fn list(&self, cx: &Context<Self>) -> AnyElement {
        let phase = self.vm.phase(cx);
        let count = self.vm.item_count(cx);
        let mode = self.vm.mode(cx);

        let status = match &phase {
            SnapshotPhase::Loading => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Loading…")
                .into_any_element(),
            SnapshotPhase::Ready => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{count} objects"))
                .into_any_element(),
            SnapshotPhase::Error(message) => div()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(message.clone())
                .into_any_element(),
        };

        let mut header_left = h_flex().gap_3().items_center().child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::MEDIUM)
                .child(self.title.clone()),
        );
        if self.kind.is_namespaced() {
            header_left = header_left.child(render_namespace_filter(&self.namespace_filter, cx));
        }
        let header_left = header_left
            .child(status)
            .child(super::watch_mode::render(mode, self.kind, cx));

        let table_el = self.vm.render_table(cx);
        let handle = cx.entity().clone();

        v_flex()
            .size_full()
            .min_w_0()
            .child(
                h_flex()
                    .justify_between()
                    .py_2()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(header_left)
                    .child(
                        Button::new("refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.vm.refresh(cx);
                                });
                            }),
                    ),
            )
            .child(div().flex_1().min_h_0().px_4().py_3().child(table_el))
            .into_any_element()
    }
}

impl Render for ResourceListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let detail_state = self.detail.read(cx).state().clone();
        let list = self.list(cx);

        if !detail_state.is_open() {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(list)
                .into_any_element();
        }

        let viewport_width = window.viewport_size().width.as_f32();
        let split = viewport_width >= SPLIT_MIN_VIEWPORT_WIDTH;
        let detail = if self.kind == ResourceKind::Deployments {
            crate::ui::resources::deployments::detail::render(
                detail_state,
                self.deployment_actions.clone(),
                cx.entity().clone(),
                !split,
                ResourceListView::close_detail,
                cx,
            )
        } else {
            crate::ui::resources::render_detail(
                self.kind,
                detail_state,
                cx.entity().clone(),
                !split,
                ResourceListView::close_detail,
                cx,
            )
        };

        if !split {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(detail)
                .into_any_element();
        }

        h_resizable("resource-workspace")
            .child(
                resizable_panel()
                    .size(list_default_width(viewport_width))
                    .size_range(LIST_MIN_WIDTH..LIST_MAX_WIDTH)
                    .fixed()
                    .child(list),
            )
            .child(
                resizable_panel()
                    .size_range(DETAIL_MIN_WIDTH..Pixels::MAX)
                    .child(detail),
            )
            .into_any_element()
    }
}
