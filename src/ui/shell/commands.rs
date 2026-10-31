//! Workspace-level actions shared by keybindings and visible controls.

use gpui_kit::actions;

actions!(
    workspace_shell,
    [SplitRight, Unsplit, FocusNextGroup, MoveTabToOtherGroup,]
);

/// Bind the workspace shortcuts. Visible controls remain the primary path.
pub fn init(cx: &mut gpui_kit::App) {
    use gpui_kit::KeyBinding;

    cx.bind_keys([
        KeyBinding::new("secondary-shift-e", SplitRight, Some("Workspace")),
        KeyBinding::new("secondary-shift-u", Unsplit, Some("Workspace")),
        KeyBinding::new("secondary-alt-right", FocusNextGroup, Some("Workspace")),
        KeyBinding::new(
            "secondary-alt-shift-right",
            MoveTabToOtherGroup,
            Some("Workspace"),
        ),
    ]);
}
