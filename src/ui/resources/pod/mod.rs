//! Pod workspace: list, detail, and a per-tab logs/exec session panel.

mod detail;
mod exec;
mod logs;
mod sessions;

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_resizable, resizable_panel,
    table::DataTable,
    v_resizable,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, FontWeight, IntoElement, ParentElement as _, Pixels,
    Render, SharedString, Styled as _, Subscription, Window, div, px,
};

use crate::models::{NamespaceScope, SnapshotPhase};
use crate::ui::resources::namespace_filter::{
    NamespaceFilter, NamespaceFilterEvent, render as render_namespace_filter,
};
use crate::ui::resources::pod::exec::PodExecView;
use crate::ui::resources::pod::logs::PodLogsView;
use crate::ui::resources::pod::sessions::{PodSession, PodSessions, SessionKey};
use crate::view_models::resources::pod::PodsViewModel;
use crate::view_models::resources::pod::exec::PodExecViewModel;
use crate::view_models::resources::pod::logs::PodLogsViewModel;

const LIST_MIN_WIDTH: Pixels = px(340.);
const LIST_MAX_WIDTH: Pixels = px(800.);
const DETAIL_MIN_WIDTH: Pixels = px(360.);
const SPLIT_MIN_VIEWPORT_WIDTH: f32 = 1080.;
const SESSIONS_DEFAULT_HEIGHT: Pixels = px(320.);
const SESSIONS_MIN_HEIGHT: Pixels = px(160.);
const SESSIONS_MAX_HEIGHT: Pixels = px(760.);
const MAIN_MIN_HEIGHT: Pixels = px(240.);

fn list_default_width(viewport_width: f32) -> Pixels {
    if viewport_width >= 1500. {
        px(720.)
    } else if viewport_width >= 1280. {
        px(620.)
    } else {
        px(540.)
    }
}

pub struct PodsView {
    vm: gpui_kit::Entity<PodsViewModel>,
    namespace_filter: gpui_kit::Entity<NamespaceFilter>,
    sessions: PodSessions,
    _vm_sub: Subscription,
    _namespace_sub: Subscription,
}

impl PodsView {
    pub fn new(scope: NamespaceScope, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let vm = cx.new(|cx| PodsViewModel::new(scope, window, cx));
        let vm_sub = cx.observe(&vm, |_, _, cx| cx.notify());
        let namespace_filter = cx.new(|cx| NamespaceFilter::new(window, cx));
        let namespace_sub = cx.subscribe(
            &namespace_filter,
            |this, _, event: &NamespaceFilterEvent, cx| {
                let NamespaceFilterEvent::Changed(scope) = event;
                this.vm.update(cx, |vm, cx| {
                    vm.set_scope(scope.clone(), cx);
                    vm.close_detail(cx);
                });
            },
        );
        Self {
            vm,
            namespace_filter,
            sessions: PodSessions::default(),
            _vm_sub: vm_sub,
            _namespace_sub: namespace_sub,
        }
    }
    pub fn scope_label(&self, cx: &App) -> String {
        self.vm.read(cx).scope().label()
    }

    pub fn namespace_filter(&self) -> &gpui_kit::Entity<NamespaceFilter> {
        &self.namespace_filter
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.set_active(active, cx));
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.vm.update(cx, |vm, cx| vm.close_detail(cx));
    }

    /// Open logs in this tab's bottom panel, or focus them when already open.
    pub fn open_logs(
        &mut self,
        namespace: String,
        name: String,
        containers: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let key = SessionKey::logs(namespace.clone(), name.clone());
        if self.sessions.focus(&key) {
            cx.notify();
            return;
        }
        let vm = cx.new(|cx| PodLogsViewModel::new(namespace, name, containers, cx));
        let view = cx.new(|cx| PodLogsView::new(vm.clone(), cx));
        self.sessions.push(PodSession::logs(key, view, vm));
        cx.notify();
    }

    /// Open an exec terminal in this tab's bottom panel.
    pub fn open_exec(
        &mut self,
        namespace: String,
        name: String,
        containers: Vec<String>,
        container: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = SessionKey::exec(namespace.clone(), name.clone());
        if self.sessions.focus(&key) {
            cx.notify();
            return;
        }
        let vm = cx.new(|cx| {
            PodExecViewModel::new(
                namespace,
                name,
                containers,
                container,
                "sh".to_string(),
                window,
                cx,
            )
        });
        let view = cx.new(|cx| PodExecView::new(vm.clone(), window, cx));
        self.sessions.push(PodSession::exec(key, view, vm));
        cx.notify();
    }

    fn close_session(&mut self, key: &SessionKey, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.close(key) {
            session.shutdown(cx);
        }
        cx.notify();
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
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
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(SharedString::from("Pods")),
                    )
                    .child(render_namespace_filter(&self.namespace_filter, cx))
                    .child(status),
            )
            .child(
                Button::new("refresh-pods")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .on_click(move |_, _, cx| {
                        handle.update(cx, |this, cx| {
                            this.vm.update(cx, |vm, cx| vm.refresh(cx));
                        });
                    }),
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

impl PodsView {
    fn main_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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

        let viewport_width = window.viewport_size().width.as_f32();
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

        h_resizable(("pods-workspace", cx.entity().entity_id()))
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

    fn sessions_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.sessions.active_index().unwrap_or(0);
        let view = cx.entity();
        let mut bar = TabBar::new(("pod-sessions", view.entity_id()))
            .small()
            .menu(true)
            .max_width(px(300.))
            .selected_index(active)
            .on_click(cx.listener(|this, index, _, cx| {
                this.sessions.set_active(*index);
                cx.notify();
            }));

        for session in self.sessions.iter() {
            let key = session.key().clone();
            let close_view = view.clone();
            bar = bar.child(
                Tab::new().label(session.title()).suffix(
                    Button::new(SharedString::from(format!("close-session-{}", key.id())))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip("Close session")
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            close_view.update(cx, |this, cx| this.close_session(&key, cx));
                        }),
                ),
            );
        }

        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.sessions.active_element()),
            )
            .into_any_element()
    }
}

impl Render for PodsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = self.main_content(window, cx);
        if self.sessions.is_empty() {
            return main;
        }
        let panel = self.sessions_panel(cx);
        v_resizable(("pods-vertical", cx.entity().entity_id()))
            .child(
                resizable_panel()
                    .size_range(MAIN_MIN_HEIGHT..Pixels::MAX)
                    .child(main),
            )
            .child(
                resizable_panel()
                    .size(SESSIONS_DEFAULT_HEIGHT)
                    .size_range(SESSIONS_MIN_HEIGHT..SESSIONS_MAX_HEIGHT)
                    .fixed()
                    .child(panel),
            )
            .into_any_element()
    }
}
