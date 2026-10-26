//! Settings page: application preferences.

pub mod custom_resources;
pub mod theme;

use self::theme::{AppearanceState, ThemePreference};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme,
    radio::{Radio, RadioGroup},
};
use gpui_kit::{Context, FontWeight, IntoElement, ParentElement as _, Styled as _, div, px};

pub fn render_settings(cx: &Context<crate::ui::Workspace>) -> impl IntoElement {
    let preference = cx.global::<AppearanceState>().preference();
    let selected = ThemePreference::ALL
        .iter()
        .position(|option| *option == preference);

    v_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .child(
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
                                .child("Settings"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Application preferences"),
                        ),
                ),
        )
        .child(
            div().flex_1().min_h_0().overflow_y_scrollbar().child(
                v_flex().max_w(px(640.)).gap_6().p_4().child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Appearance"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    "Follow the system appearance or choose a fixed \
                                         light or dark theme.",
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Theme"),
                                )
                                .child(
                                    RadioGroup::new("theme-preference")
                                        .selected_index(selected)
                                        .children(ThemePreference::ALL.into_iter().map(|option| {
                                            Radio::new(option.label()).label(option.label())
                                        }))
                                        .on_change(move |index, _, cx| {
                                            if let Some(option) = ThemePreference::ALL.get(*index) {
                                                theme::set_preference(cx, *option);
                                            }
                                        }),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Custom resources"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            "Add a resource kind served by a CRD so it appears \
                                             in the sidebar.",
                                        ),
                                )
                                .child(custom_resources::render_add_button(cx)),
                        ),
                ),
            ),
        )
}
