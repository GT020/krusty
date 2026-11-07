//! Square, outlined tab controls shared by the workspace and session tabs.

use gpui_kit::component::button::{Button, ButtonRounded, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::{App, Background, ElementId, SharedString, Styled as _};

/// A tab button with square corners and a full outline.
///
/// The selected tab uses a lighter outline, while unselected tabs use the
/// theme border so adjacent tabs stay visually distinct.
pub fn tab(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Button {
    let border = if selected {
        cx.theme().foreground.opacity(0.55)
    } else {
        cx.theme().border
    };

    Button::new(id)
        .ghost()
        .small()
        .rounded(ButtonRounded::None)
        .selected(selected)
        .label(label)
        .border_1()
        .border_color(border)
        .bg(if selected {
            cx.theme().tokens.tab_active.into()
        } else {
            Background::from(cx.theme().transparent)
        })
        .text_color(if selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        })
        .cursor_pointer()
}
