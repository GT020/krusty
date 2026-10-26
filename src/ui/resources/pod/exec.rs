//! Interactive terminal sheet for a pod exec session.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, Context, Entity, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, MouseButton, ParentElement as _, Render, ScrollDelta, SharedString,
    Styled as _, Subscription, Window, div, px, rgb,
};

use crate::view_models::resources::pod::exec::{PodExecViewModel, SHELLS};

const CELL_WIDTH: f32 = 7.2;
const CELL_HEIGHT: f32 = 16.0;
const TERMINAL_FONT_SIZE: f32 = 12.0;

pub struct PodExecView {
    vm: Entity<PodExecViewModel>,
    focus: FocusHandle,
    focused_once: bool,
    _vm_sub: Subscription,
    _bounds_sub: Subscription,
}

impl PodExecView {
    pub fn new(vm: Entity<PodExecViewModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&vm, |_, _, cx| cx.notify());
        let (cols, rows) = terminal_size(window);
        vm.update(cx, |vm, cx| vm.resize(cols, rows, cx));
        let resize_vm = vm.clone();
        let bounds_subscription = cx.observe_window_bounds(window, move |_, window, cx| {
            let (cols, rows) = terminal_size(window);
            resize_vm.update(cx, |vm, cx| vm.resize(cols, rows, cx));
        });
        Self {
            vm,
            focus: cx.focus_handle(),
            focused_once: false,
            _vm_sub: subscription,
            _bounds_sub: bounds_subscription,
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let keystroke = &event.keystroke;
        let paste = (keystroke.modifiers.platform
            || (keystroke.modifiers.control && keystroke.modifiers.shift))
            && keystroke.key.eq_ignore_ascii_case("v");
        let bytes = if paste {
            match cx.read_from_clipboard().and_then(|item| item.text()) {
                Some(text) => Some(text.into_bytes()),
                None => Some(Vec::new()),
            }
        } else {
            let app_cursor = self.vm.read(cx).application_cursor();
            key_bytes(keystroke, app_cursor)
        };
        let Some(bytes) = bytes else {
            return false;
        };
        self.vm.update(cx, |vm, cx| vm.send_input(bytes, cx));
        true
    }

    fn toolbar(
        vm: Entity<PodExecViewModel>,
        containers: Vec<String>,
        container: String,
        shell: String,
        status: String,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu_vm = vm.clone();
        let menu_container = container.clone();
        let container_button = Button::new("pod-exec-container")
            .ghost()
            .small()
            .label(container)
            .icon(IconName::ChevronsUpDown)
            .dropdown_menu(move |mut menu, _, _| {
                for name in &containers {
                    let selected = name == &menu_container;
                    let value = name.clone();
                    let vm = menu_vm.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).checked(selected).on_click(
                        move |_, _, cx| {
                            let value = value.clone();
                            vm.update(cx, |vm, cx| vm.set_container(value, cx));
                        },
                    ));
                }
                menu
            });

        let shell_vm = vm;
        let menu_shell = shell.clone();
        let shell_button = Button::new("pod-exec-shell")
            .ghost()
            .small()
            .label(shell)
            .icon(IconName::ChevronsUpDown)
            .dropdown_menu(move |mut menu, _, _| {
                for name in SHELLS {
                    let selected = name == menu_shell;
                    let value = name.to_string();
                    let vm = shell_vm.clone();
                    menu = menu.item(PopupMenuItem::new(name).checked(selected).on_click(
                        move |_, _, cx| {
                            let value = value.clone();
                            vm.update(cx, |vm, cx| vm.set_shell(value, cx));
                        },
                    ));
                }
                menu
            });

        let mut status_row = h_flex().gap_2().items_center().child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(status)),
        );
        if let Some(error) = error {
            status_row = status_row.child(
                div()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(error)),
            );
        }

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Container"),
                            )
                            .child(container_button),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Shell"),
                            )
                            .child(shell_button),
                    ),
            )
            .child(status_row)
            .into_any_element()
    }
}

impl Render for PodExecView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            window.focus(&self.focus, cx);
        }
        let vm = self.vm.read(cx);
        let containers = vm.containers().to_vec();
        let container = vm.container().to_string();
        let shell = vm.shell().to_string();
        let status = vm.status().to_string();
        let error = vm.error().map(str::to_string);
        let screen = vm.screen();
        let (rows, cols) = screen.size();
        let cursor = screen.cursor_position();
        let show_cursor = !screen.hide_cursor();

        let foreground = cx.theme().foreground;
        let background = rgb(0x101014).into();
        let rows: Vec<AnyElement> = (0..rows.get())
            .map(|row| {
                render_row(
                    screen,
                    row,
                    cols.get(),
                    (cursor.0, cursor.1),
                    show_cursor,
                    foreground,
                    background,
                )
            })
            .collect();

        let terminal = cx.entity().clone();
        let key_terminal = terminal.clone();
        let wheel_terminal = terminal.clone();
        let focus_handle = self.focus.clone();

        v_flex()
            .size_full()
            .min_w_0()
            .gap_3()
            .p_3()
            .child(Self::toolbar(
                self.vm.clone(),
                containers,
                container,
                shell,
                status,
                error,
                cx,
            ))
            .child(
                div()
                    .id("pod-exec-terminal")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(background)
                    .p_2()
                    .track_focus(&self.focus)
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&focus_handle, cx);
                        cx.stop_propagation();
                    })
                    .capture_key_down(move |event, window, cx| {
                        let handled =
                            key_terminal.update(cx, |this, cx| this.handle_key(event, cx));
                        if handled {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    })
                    .on_scroll_wheel(move |event, _, cx| {
                        let lines = match event.delta {
                            ScrollDelta::Lines(point) => point.y.round() as i32,
                            ScrollDelta::Pixels(point) => {
                                (point.y.as_f32() / CELL_HEIGHT).round() as i32
                            }
                        };
                        wheel_terminal.update(cx, |this, cx| {
                            this.vm.update(cx, |vm, cx| vm.scroll(lines, cx));
                        });
                        cx.stop_propagation();
                    })
                    .child(v_flex().gap_0().children(rows)),
            )
    }
}

fn terminal_size(window: &Window) -> (u16, u16) {
    let width = (window.viewport_size().width.as_f32().min(820.) - 48.).max(160.);
    let height = (window.viewport_size().height.as_f32() - 220.).max(96.);
    (
        ((width / CELL_WIDTH) as u16).clamp(20, 240),
        ((height / CELL_HEIGHT) as u16).clamp(6, 120),
    )
}

fn render_row(
    screen: &vt100::Screen,
    row: u16,
    cols: u16,
    cursor: (u16, u16),
    show_cursor: bool,
    default_fg: Hsla,
    default_bg: Hsla,
) -> AnyElement {
    struct Run {
        text: String,
        fg: Hsla,
        bg: Hsla,
        bold: bool,
        italic: bool,
        underline: bool,
    }

    let mut runs: Vec<Run> = Vec::new();
    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        let is_cursor = show_cursor && cursor == (row, col);
        let mut fg = ansi_color(cell.fgcolor(), default_fg, cell.bold());
        let mut bg = ansi_color(cell.bgcolor(), default_bg, false);
        if cell.inverse() ^ is_cursor {
            std::mem::swap(&mut fg, &mut bg);
        }
        let text = if cell.contents().is_empty() {
            " ".to_string()
        } else {
            cell.contents().to_string()
        };
        let bold = cell.bold();
        let italic = cell.italic();
        let underline = cell.underline();
        match runs.last_mut() {
            Some(run)
                if run.fg == fg
                    && run.bg == bg
                    && run.bold == bold
                    && run.italic == italic
                    && run.underline == underline =>
            {
                run.text.push_str(&text);
            }
            _ => runs.push(Run {
                text,
                fg,
                bg,
                bold,
                italic,
                underline,
            }),
        }
    }

    h_flex()
        .h(px(CELL_HEIGHT))
        .w_full()
        .flex_none()
        .items_center()
        .whitespace_nowrap()
        .children(runs.into_iter().map(|run| {
            let mut element = div()
                .h_full()
                .flex()
                .items_center()
                .font_family("monospace")
                .text_size(px(TERMINAL_FONT_SIZE))
                .line_height(px(CELL_HEIGHT))
                .text_color(run.fg)
                .bg(run.bg)
                .child(SharedString::from(run.text));
            if run.bold {
                element = element.font_weight(gpui_kit::FontWeight::BOLD);
            }
            if run.italic {
                element = element.italic();
            }
            if run.underline {
                element = element.underline();
            }
            element
        }))
        .into_any_element()
}

fn ansi_color(color: vt100::Color, default: Hsla, bold: bool) -> Hsla {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(index) => {
            let index = if bold && index < 8 { index + 8 } else { index };
            ansi_palette(index)
        }
        vt100::Color::Rgb(red, green, blue) => {
            rgb(((red as u32) << 16) | ((green as u32) << 8) | blue as u32).into()
        }
    }
}

fn ansi_palette(index: u8) -> Hsla {
    let hex = match index {
        0 => 0x1d1f21,
        1 => 0xcc6666,
        2 => 0xb5bd68,
        3 => 0xf0c674,
        4 => 0x81a2be,
        5 => 0xb294bb,
        6 => 0x8abeb7,
        7 => 0xc5c8c6,
        8 => 0x666666,
        9 => 0xd54e53,
        10 => 0xb9ca4a,
        11 => 0xe7c547,
        12 => 0x7aa6da,
        13 => 0xc397d8,
        14 => 0x70c0b1,
        15 => 0xeaeaea,
        _ => return ansi_palette(index % 16),
    };
    rgb(hex).into()
}

fn key_bytes(keystroke: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
    let key = keystroke.key.as_str();
    let ctrl = keystroke.modifiers.control;
    let alt = keystroke.modifiers.alt;
    let shift = keystroke.modifiers.shift;

    if keystroke.modifiers.platform {
        return None;
    }

    if ctrl && key == "space" {
        return Some(vec![0x00]);
    }

    if ctrl && !alt && key.chars().count() == 1 {
        let ch = key.chars().next()?.to_ascii_lowercase();
        if shift && ch == 'c' {
            return Some(Vec::new());
        }
        let byte = match ch {
            'a'..='z' => Some(ch as u8 - b'a' + 1),
            '[' => Some(0x1b),
            '\\' => Some(0x1c),
            ']' => Some(0x1d),
            '^' => Some(0x1e),
            '_' => Some(0x1f),
            ' ' => Some(0x00),
            _ => None,
        };
        if let Some(byte) = byte {
            return Some(vec![byte]);
        }
    }

    let sequence: &[u8] = match key {
        "enter" | "return" => b"\r",
        "backspace" => b"\x7f",
        "tab" if shift => b"\x1b[Z",
        "tab" => b"\t",
        "escape" => b"\x1b",
        "up" if app_cursor => b"\x1bOA",
        "down" if app_cursor => b"\x1bOB",
        "right" if app_cursor => b"\x1bOC",
        "left" if app_cursor => b"\x1bOD",
        "home" if app_cursor => b"\x1bOH",
        "end" if app_cursor => b"\x1bOF",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "insert" => b"\x1b[2~",
        "delete" => b"\x1b[3~",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => &[],
    };
    if !sequence.is_empty() {
        let mut bytes = Vec::with_capacity(sequence.len() + 1);
        if alt {
            bytes.push(0x1b);
        }
        bytes.extend_from_slice(sequence);
        return Some(bytes);
    }

    if ctrl {
        return None;
    }
    let text = keystroke.key_char.as_deref()?;
    if text.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() + 1);
    if alt {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(text.as_bytes());
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(source: &str) -> Keystroke {
        Keystroke::parse(source).expect("valid keystroke")
    }

    #[test]
    fn maps_control_and_special_keys_for_the_terminal() {
        assert_eq!(key_bytes(&stroke("ctrl-c"), false), Some(vec![0x03]));
        assert_eq!(key_bytes(&stroke("ctrl-d"), false), Some(vec![0x04]));
        assert_eq!(key_bytes(&stroke("enter"), false), Some(b"\r".to_vec()));
        assert_eq!(
            key_bytes(&stroke("backspace"), false),
            Some(b"\x7f".to_vec())
        );
        assert_eq!(key_bytes(&stroke("tab"), false), Some(b"\t".to_vec()));
        assert_eq!(
            key_bytes(&stroke("shift-tab"), false),
            Some(b"\x1b[Z".to_vec())
        );
    }

    #[test]
    fn uses_application_cursor_sequences_when_requested() {
        assert_eq!(key_bytes(&stroke("up"), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes(&stroke("up"), true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key_bytes(&stroke("left"), false), Some(b"\x1b[D".to_vec()));
    }
}
