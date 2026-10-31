//! Context menu for the current window text selection.

use gpui_kit::base::TextSelection;
use gpui_kit::component::IconName;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, ClipboardItem, Context, Window};

/// Returns an empty menu when nothing is selected, so right-clicking plain
/// detail text does not open a menu without a target.
pub(crate) fn text_selection_context_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    if TextSelection::selected_text(window, cx).trim().is_empty() {
        return menu;
    }
    menu.item(PopupMenuItem::new("Copy").icon(IconName::Copy).on_click(
        |_, window, cx: &mut App| {
            let text = TextSelection::selected_text(window, cx).trim().to_string();
            if !text.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
        },
    ))
}
