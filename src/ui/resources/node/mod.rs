//! Node workspace: list plus a detail pane that opens on row selection.

mod detail;

use gpui_kit::base::v_flex;
use gpui_kit::component::{h_resizable, resizable_panel, table::DataTable};
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Pixels, Render, Styled as _,
    Subscription, Window, div, px,
};

use crate::models::NamespaceScope;
use crate::ui::resources::search_filter::{SearchFilter, SearchFilterEvent};
use crate::view_models::resources::node::NodesViewModel;

const LIST_MIN_WIDTH: Pixels = px(320.);
const LIST_MAX_WIDTH: Pixels = px(720.);
const DETAIL_MIN_WIDTH: Pixels = px(360.);
const SPLIT_MIN_VIEWPORT_WIDTH: f32 = 1080.;

fn list_default_width(viewport_width: f32) -> Pixels {
    if viewport_width >= 1500. {
        px(640.)
    } else if viewport_width >= 1280. {
        px(540.)
    } else {
        px(460.)
    }
}

pub struct NodesView {
    vm: gpui_kit::Entity<NodesViewModel>,
    search_filter: gpui_kit::Entity<SearchFilter>,
    width_share: f32,
    _vm_sub: Subscription,
    _search_sub: Subscription,
}

impl NodesView {
    pub fn new(scope: NamespaceScope, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let vm = cx.new(|cx| NodesViewModel::new(scope, window, cx));
        let sub = cx.observe(&vm, |_, _, cx| cx.notify());
        let search_filter = cx.new(|cx| SearchFilter::new(window, cx));
        let search_sub = cx.subscribe(&search_filter, |this, _, event: &SearchFilterEvent, cx| {
            let SearchFilterEvent::Changed(filter) = event;
            this.vm.update(cx, |vm, cx| {
                vm.set_filter(filter.clone(), cx);
                vm.close_detail(cx);
            });
        });
        Self {
            vm,
            search_filter,
            width_share: 1.0,
            _vm_sub: sub,
            _search_sub: search_sub,
        }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }

    /// Fraction of the window width this editor group occupies.
    pub fn set_width_share(&mut self, share: f32, cx: &mut Context<Self>) {
        let share = share.clamp(0.1, 1.0);
        if (self.width_share - share).abs() < f32::EPSILON {
            return;
        }
        self.width_share = share;
        cx.notify();
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.close_detail(cx));
    }

    /// Open a node's detail pane by name, used by `kubectl describe`.
    pub fn open_detail(&mut self, name: String, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.open_detail(name, cx));
    }

    fn header(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let phase = self.vm.read(cx).phase(cx);
        let count = self.vm.read(cx).item_count(cx);
        let mode = self.vm.read(cx).mode(cx);
        super::list_header::render(
            crate::models::ResourceKind::Nodes,
            &phase,
            count,
            mode,
            None,
            &self.search_filter,
            |this, _window, cx| {
                this.vm.update(cx, |vm, cx| vm.refresh(cx));
            },
            cx,
        )
    }

    fn list(&self, cx: &Context<Self>) -> gpui_kit::AnyElement {
        let table = self.vm.read(cx).table().clone();
        v_flex()
            .size_full()
            .min_w_0()
            .child(self.header(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px_4()
                    .py_3()
                    .child(DataTable::new(&table)),
            )
            .into_any_element()
    }
}

impl Render for NodesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let detail_state = self.vm.read(cx).detail(cx);
        let list = self.list(cx);

        if !detail_state.is_open() {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(list)
                .into_any_element();
        }

        let viewport_width = window.viewport_size().width.as_f32() * self.width_share;
        let split = viewport_width >= SPLIT_MIN_VIEWPORT_WIDTH;
        let usage = self.vm.read(cx).usage(cx);
        let usage_error = self.vm.read(cx).usage_error(cx);
        let detail = detail::render_detail(
            detail_state,
            usage,
            usage_error,
            cx.entity().clone(),
            !split,
            cx,
        );

        if !split {
            return v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(detail)
                .into_any_element();
        }

        h_resizable("nodes-workspace")
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
