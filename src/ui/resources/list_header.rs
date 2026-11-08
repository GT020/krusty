//! Shared header for every resource list: filters, status, watch mode, refresh.

use gpui_kit::base::h_flex;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AnyElement, Context, Entity, IntoElement, ParentElement as _, Styled as _, Window, div,
};

use crate::models::{ResourceKind, SnapshotPhase, WatchMode};
use crate::ui::resources::namespace_filter::{NamespaceFilter, render as render_namespace_filter};
use crate::ui::resources::search_filter::{SearchFilter, render as render_search_filter};

#[allow(clippy::too_many_arguments)]
pub fn render<T: 'static>(
    kind: ResourceKind,
    phase: &SnapshotPhase,
    count: usize,
    mode: WatchMode,
    namespace_filter: Option<&Entity<NamespaceFilter>>,
    search_filter: &Entity<SearchFilter>,
    on_refresh: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    cx: &Context<T>,
) -> AnyElement {
    let status = match phase {
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

    let mut filters = h_flex().gap_3().items_center();
    if kind.is_namespaced()
        && let Some(namespace_filter) = namespace_filter
    {
        filters = filters.child(render_namespace_filter(namespace_filter, cx));
    }
    let filters = filters
        .child(render_search_filter(search_filter, cx))
        .child(status)
        .child(super::watch_mode::render(mode, cx));

    let handle = cx.entity().clone();
    h_flex()
        .justify_between()
        .py_2()
        .px_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(filters)
        .child(
            Button::new(gpui_kit::SharedString::from(format!(
                "refresh-{}",
                kind.label()
            )))
            .ghost()
            .small()
            .icon(IconName::RefreshCw)
            .on_click(move |_, window, cx| {
                handle.update(cx, |this, cx| on_refresh(this, window, cx));
            }),
        )
        .into_any_element()
}
