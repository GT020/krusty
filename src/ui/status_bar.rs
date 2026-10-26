use crate::cluster::ClusterState;
use gpui_kit::base::h_flex;
use gpui_kit::component::{ActiveTheme, badge::Badge};
use gpui_kit::{Context, IntoElement, ParentElement as _, Styled as _, div};

pub fn render_status_bar(cx: &Context<crate::ui::Workspace>) -> impl IntoElement {
    let state = cx.global::<ClusterState>();
    let context = state
        .context()
        .map(|context| format!("{context} · {}", state.scope().label()))
        .unwrap_or_else(|| "no cluster · add a kubeconfig".to_string());
    let (dot, label) = match state.reachable() {
        Some(true) => (cx.theme().success, "Connected"),
        Some(false) => (cx.theme().danger, "Unreachable"),
        None if state.context().is_some() => (cx.theme().warning, "Connecting"),
        None => (cx.theme().warning, "Not connected"),
    };

    gpui_kit::component::status_bar::StatusBar::new()
        .left(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(context),
        )
        .right(
            h_flex()
                .gap_2()
                .text_xs()
                .child(Badge::new().dot().color(dot))
                .child(label),
        )
}
