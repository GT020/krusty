//! Update-mode indicator for resource list headers.
//!
//! Shows a small "Live" badge while the watch stream is healthy and "Polling"
//! when the store is re-listed on a timer — either because the API server
//! denied `watch` (RBAC) or because the user prefers polling. The dropdown
//! switches the kind between live watch and a poll rate (or off), and the
//! choice is saved per resource kind.

use gpui_kit::base::h_flex;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, div, px};

use crate::models::{PollRate, ResourceKind, WatchMode};
use crate::repository::WatchPreference;

/// Poll intervals offered in the rate menu, in seconds.
const POLL_RATES: [u64; 5] = [2, 5, 10, 30, 60];

pub(crate) fn render(mode: WatchMode, kind: ResourceKind, cx: &App) -> AnyElement {
    let (color, label, button_label) = match mode {
        WatchMode::Watch => (cx.theme().success, "Live".to_string(), "Live".to_string()),
        WatchMode::Poll(PollRate::Every(secs)) => (
            cx.theme().warning,
            format!("Polling · {secs}s"),
            format!("{secs}s"),
        ),
        WatchMode::Poll(PollRate::Off) => (
            cx.theme().warning,
            "Polling off".to_string(),
            "Off".to_string(),
        ),
    };

    let badge = h_flex()
        .gap_1()
        .items_center()
        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(color))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        );

    let mode_button = Button::new("watch-mode")
        .ghost()
        .small()
        .label(button_label)
        .icon(IconName::ChevronsUpDown)
        .tooltip(
            "How this resource type stays up to date. \
             Watch needs the `watch` permission; otherwise krusty falls back \
             to polling. Saved per resource type.",
        )
        .dropdown_menu(move |mut menu, _, _| {
            menu = menu.item(
                PopupMenuItem::new("Live watch")
                    .checked(mode == WatchMode::Watch)
                    .on_click(move |_, _, cx| {
                        crate::repository::set_watch_preference(cx, kind, WatchPreference::Watch);
                    }),
            );
            menu = menu.separator();
            for secs in POLL_RATES {
                menu = menu.item(
                    PopupMenuItem::new(format!("Poll every {secs}s"))
                        .checked(mode == WatchMode::Poll(PollRate::Every(secs)))
                        .on_click(move |_, _, cx| {
                            crate::repository::set_poll_rate(cx, kind, PollRate::Every(secs));
                        }),
                );
            }
            menu.separator().item(
                PopupMenuItem::new("Polling off")
                    .checked(mode == WatchMode::Poll(PollRate::Off))
                    .on_click(move |_, _, cx| {
                        crate::repository::set_poll_rate(cx, kind, PollRate::Off);
                    }),
            )
        });

    badge.child(mode_button).into_any_element()
}
