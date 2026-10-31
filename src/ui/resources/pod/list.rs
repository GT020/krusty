//! Compact pod list widget used by the dashboard.
//!
//! It uses the list half of `PodsViewModel` in list-only mode, so selecting a
//! row emits an intent instead of starting a detail subscription.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div,
};

use crate::models::dashboard::{PodWidgetSource, WidgetSource};
use crate::models::{NamespaceScope, SnapshotPhase};
use crate::ui::dashboard::pinnable_sized;
use crate::view_models::resources::pod::PodsViewModel;

pub struct PodListView {
    vm: Entity<PodsViewModel>,
    id_prefix: SharedString,
    title: SharedString,
    show_header: bool,
    _vm_sub: Subscription,
}

impl PodListView {
    fn new(
        vm: Entity<PodsViewModel>,
        id_prefix: SharedString,
        title: SharedString,
        show_header: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        Self {
            vm,
            id_prefix,
            title,
            show_header,
            _vm_sub: subscription,
        }
    }

    fn header(&self, cx: &Context<Self>) -> AnyElement {
        let phase = self.vm.read(cx).phase(cx);
        let count = self.vm.read(cx).item_count(cx);
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

        let handle = cx.entity().clone();
        h_flex()
            .justify_between()
            .py_2()
            .px_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_size(gpui_kit::px(15.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child(self.title.clone()),
                    )
                    .child(status),
            )
            .child(
                Button::new(format!("{}-refresh", self.id_prefix))
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .tooltip("Refresh")
                    .on_click(move |_, _, cx| {
                        handle.update(cx, |this, cx| {
                            this.vm.update(cx, |vm, cx| vm.refresh(cx));
                        });
                    }),
            )
            .into_any_element()
    }

    fn table(&self, cx: &Context<Self>) -> AnyElement {
        let table = self.vm.read(cx).table().clone();
        div()
            .flex_1()
            .min_h_0()
            .px_4()
            .py_3()
            .child(DataTable::new(&table))
            .into_any_element()
    }
}

impl Render for PodListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = v_flex().size_full().min_w_0();
        if self.show_header {
            root = root.child(self.header(cx));
        }
        root.child(self.table(cx))
    }
}

pub struct PodListArgs {
    pub scope: NamespaceScope,
    pub id_prefix: SharedString,
    pub title: SharedString,
    pub show_header: bool,
}

/// Reusable pod list component. Owns its list watch and table state.
pub struct PodListWidget {
    vm: Entity<PodsViewModel>,
    view: Entity<PodListView>,
    id: SharedString,
    source: WidgetSource,
}

impl PodListWidget {
    pub fn mount(args: PodListArgs, window: &mut Window, cx: &mut App) -> Self {
        let source = WidgetSource::Pod(PodWidgetSource::List {
            namespace: args.scope.namespace().map(str::to_string),
        });
        let id = args.id_prefix;
        let vm = cx.new(|cx| PodsViewModel::new_list_only(args.scope, window, cx));
        let view = cx.new(|cx| {
            PodListView::new(
                vm.clone(),
                id.clone(),
                args.title.clone(),
                args.show_header,
                cx,
            )
        });
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
            self.source.clone(),
        )
    }

    pub fn set_active(&self, active: bool, cx: &mut App) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }
}
