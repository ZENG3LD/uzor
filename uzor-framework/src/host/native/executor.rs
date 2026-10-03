//! Flat `WindowCommand` executor (design §5.4).
//!
//! Idempotent, infallible toward the kernel (OS errors are logged), and
//! never invents commands. The [`OsWindow`] trait lets unit tests drive the
//! match with a recording fake; the live host wraps a winit `Window`.

use uzor::{CornerStyle, CursorIcon, Rect, ResizeDirection, RgbaIcon};

use crate::types::ids::Ticket;
use crate::types::window::{
    Attention, CursorMode, FullscreenMode, ImePurpose, Point, RenderCmd, SizePx, ThemeHint,
    WindowCommand, WindowSpec,
};

/// Side-effects an executor may need the host to finish outside the match
/// (clipboard, spawn, screenshot, exit, render-hub control).
#[derive(Clone, Debug, PartialEq)]
pub enum ExecutorFollowUp {
    /// Write text to the system clipboard.
    ClipboardWrite(String),
    /// Read the clipboard; answer with this ticket.
    ClipboardRead(Ticket),
    /// Capture the window; answer with this ticket.
    Screenshot(Ticket),
    /// Create a new OS window from `spec`.
    Spawn(WindowSpec),
    /// Drop / destroy this window.
    Close,
    /// Leave the event loop.
    ExitApp,
    /// Forward a render-hub control command.
    Render(RenderCmd),
    /// Echo after an atomic outer-rect change.
    OuterRectEcho {
        /// Outer position, physical screen pixels.
        position: (i32, i32),
        /// Outer size, physical pixels.
        size: SizePx,
    },
}

/// OS window surface the executor mutates. Recording fakes implement this
/// for unit tests; the live host wraps `winit::window::Window`.
pub trait OsWindow {
    /// Set the OS title.
    fn set_title(&mut self, title: &str);
    /// Set or clear the window icon.
    fn set_icon(&mut self, icon: Option<&RgbaIcon>);
    /// Show or hide OS decorations.
    fn set_decorations(&mut self, on: bool);
    /// Corner rounding preference.
    fn set_corner_style(&mut self, style: CornerStyle);
    /// Border colour `0xAARRGGBB`, or the OS default.
    fn set_border_color(&mut self, color: Option<u32>);
    /// Theme hint for OS decorations.
    fn set_theme(&mut self, theme: Option<ThemeHint>);
    /// Show or hide the window.
    fn set_visible(&mut self, on: bool);
    /// Minimize or restore.
    fn set_minimized(&mut self, on: bool);
    /// Maximize or restore.
    fn set_maximized(&mut self, on: bool);
    /// Enter or leave fullscreen.
    fn set_fullscreen(&mut self, mode: FullscreenMode);
    /// Allow or forbid user resizing.
    fn set_resizable(&mut self, on: bool);
    /// Minimum inner size.
    fn set_min_inner_size(&mut self, size: Option<SizePx>);
    /// Maximum inner size.
    fn set_max_inner_size(&mut self, size: Option<SizePx>);
    /// Atomic outer move + resize. Returns the applied outer rect for echo.
    fn set_outer_rect(&mut self, position: (i32, i32), size: SizePx) -> (i32, i32, SizePx);
    /// Ask the OS for a new inner size.
    fn request_inner_size(&mut self, size: SizePx);
    /// Start an OS window move.
    fn drag_window(&mut self);
    /// Start an OS window resize from an edge / corner.
    fn drag_resize_window(&mut self, dir: ResizeDirection);
    /// Show the OS window menu.
    fn show_window_menu(&mut self, at: (i32, i32));
    /// Set the cursor icon.
    fn set_cursor(&mut self, icon: CursorIcon);
    /// Show or hide the cursor.
    fn set_cursor_visible(&mut self, on: bool);
    /// Cursor grab / lock mode.
    fn set_cursor_mode(&mut self, mode: CursorMode);
    /// Warp the cursor to a window-local logical position.
    fn set_cursor_position(&mut self, pos: Point);
    /// Enable or disable the OS input method.
    fn set_ime_allowed(&mut self, on: bool);
    /// Tell the IME where the caret is.
    fn set_ime_cursor_area(&mut self, area: Rect);
    /// IME purpose hint.
    fn set_ime_purpose(&mut self, purpose: ImePurpose);
    /// Bring the window to the front and focus it.
    fn focus_window(&mut self);
    /// Request user attention.
    fn request_attention(&mut self, kind: Option<Attention>);
    /// Request a redraw.
    fn request_redraw(&mut self);
}

/// Apply one [`WindowCommand`] to `window`. Returns a follow-up the host
/// must finish (clipboard / spawn / paint / exit), if any.
pub fn execute(window: &mut dyn OsWindow, cmd: WindowCommand) -> Option<ExecutorFollowUp> {
    match cmd {
        WindowCommand::RequestRedraw => {
            window.request_redraw();
            None
        }
        WindowCommand::SetTitle(title) => {
            window.set_title(&title);
            None
        }
        WindowCommand::SetIcon(icon) => {
            window.set_icon(icon.as_ref());
            None
        }
        WindowCommand::SetDecorations(on) => {
            window.set_decorations(on);
            None
        }
        WindowCommand::SetCornerStyle(style) => {
            window.set_corner_style(style);
            None
        }
        WindowCommand::SetBorderColor(color) => {
            window.set_border_color(color);
            None
        }
        WindowCommand::SetTheme(theme) => {
            window.set_theme(theme);
            None
        }
        WindowCommand::SetVisible(on) => {
            window.set_visible(on);
            None
        }
        WindowCommand::SetMinimized(on) => {
            window.set_minimized(on);
            None
        }
        WindowCommand::SetMaximized(on) => {
            window.set_maximized(on);
            None
        }
        WindowCommand::SetFullscreen(mode) => {
            window.set_fullscreen(mode);
            None
        }
        WindowCommand::SetResizable(on) => {
            window.set_resizable(on);
            None
        }
        WindowCommand::SetMinInnerSize(size) => {
            window.set_min_inner_size(size);
            None
        }
        WindowCommand::SetMaxInnerSize(size) => {
            window.set_max_inner_size(size);
            None
        }
        WindowCommand::SetOuterRect { position, size } => {
            let (x, y, applied) = window.set_outer_rect(position, size);
            Some(ExecutorFollowUp::OuterRectEcho {
                position: (x, y),
                size: applied,
            })
        }
        WindowCommand::RequestInnerSize(size) => {
            window.request_inner_size(size);
            None
        }
        WindowCommand::DragWindow => {
            window.drag_window();
            None
        }
        WindowCommand::DragResizeWindow(dir) => {
            window.drag_resize_window(dir);
            None
        }
        WindowCommand::ShowWindowMenu { at } => {
            window.show_window_menu(at);
            None
        }
        WindowCommand::SetCursor(icon) => {
            window.set_cursor(icon);
            None
        }
        WindowCommand::SetCursorVisible(on) => {
            window.set_cursor_visible(on);
            None
        }
        WindowCommand::SetCursorMode(mode) => {
            window.set_cursor_mode(mode);
            None
        }
        WindowCommand::SetCursorPosition(pos) => {
            window.set_cursor_position(pos);
            None
        }
        WindowCommand::SetImeAllowed(on) => {
            window.set_ime_allowed(on);
            None
        }
        WindowCommand::SetImeCursorArea(area) => {
            window.set_ime_cursor_area(area);
            None
        }
        WindowCommand::SetImePurpose(purpose) => {
            window.set_ime_purpose(purpose);
            None
        }
        WindowCommand::ClipboardWrite(text) => Some(ExecutorFollowUp::ClipboardWrite(text)),
        WindowCommand::ClipboardRead(ticket) => Some(ExecutorFollowUp::ClipboardRead(ticket)),
        WindowCommand::FocusWindow => {
            window.focus_window();
            None
        }
        WindowCommand::RequestAttention(kind) => {
            window.request_attention(kind);
            None
        }
        WindowCommand::Spawn(spec) => Some(ExecutorFollowUp::Spawn(spec)),
        WindowCommand::Close => Some(ExecutorFollowUp::Close),
        WindowCommand::ExitApp => Some(ExecutorFollowUp::ExitApp),
        WindowCommand::Render(cmd) => Some(ExecutorFollowUp::Render(cmd)),
        WindowCommand::Screenshot(ticket) => Some(ExecutorFollowUp::Screenshot(ticket)),
    }
}

/// Recording fake for executor unit tests.
#[derive(Default, Debug)]
pub struct RecordingWindow {
    /// Log of method names and a short payload.
    pub log: Vec<String>,
    /// Last outer rect applied.
    pub outer: Option<((i32, i32), SizePx)>,
}

impl OsWindow for RecordingWindow {
    fn set_title(&mut self, title: &str) {
        self.log.push(format!("set_title:{title}"));
    }
    fn set_icon(&mut self, icon: Option<&RgbaIcon>) {
        self.log.push(format!("set_icon:{}", icon.is_some()));
    }
    fn set_decorations(&mut self, on: bool) {
        self.log.push(format!("set_decorations:{on}"));
    }
    fn set_corner_style(&mut self, style: CornerStyle) {
        self.log.push(format!("set_corner_style:{style:?}"));
    }
    fn set_border_color(&mut self, color: Option<u32>) {
        self.log.push(format!("set_border_color:{color:?}"));
    }
    fn set_theme(&mut self, theme: Option<ThemeHint>) {
        self.log.push(format!("set_theme:{theme:?}"));
    }
    fn set_visible(&mut self, on: bool) {
        self.log.push(format!("set_visible:{on}"));
    }
    fn set_minimized(&mut self, on: bool) {
        self.log.push(format!("set_minimized:{on}"));
    }
    fn set_maximized(&mut self, on: bool) {
        self.log.push(format!("set_maximized:{on}"));
    }
    fn set_fullscreen(&mut self, mode: FullscreenMode) {
        self.log.push(format!("set_fullscreen:{mode:?}"));
    }
    fn set_resizable(&mut self, on: bool) {
        self.log.push(format!("set_resizable:{on}"));
    }
    fn set_min_inner_size(&mut self, size: Option<SizePx>) {
        self.log.push(format!("set_min_inner_size:{size:?}"));
    }
    fn set_max_inner_size(&mut self, size: Option<SizePx>) {
        self.log.push(format!("set_max_inner_size:{size:?}"));
    }
    fn set_outer_rect(&mut self, position: (i32, i32), size: SizePx) -> (i32, i32, SizePx) {
        self.outer = Some((position, size));
        self.log
            .push(format!("set_outer_rect:{position:?}:{size:?}"));
        (position.0, position.1, size)
    }
    fn request_inner_size(&mut self, size: SizePx) {
        self.log.push(format!("request_inner_size:{size:?}"));
    }
    fn drag_window(&mut self) {
        self.log.push("drag_window".into());
    }
    fn drag_resize_window(&mut self, dir: ResizeDirection) {
        self.log.push(format!("drag_resize_window:{dir:?}"));
    }
    fn show_window_menu(&mut self, at: (i32, i32)) {
        self.log.push(format!("show_window_menu:{at:?}"));
    }
    fn set_cursor(&mut self, icon: CursorIcon) {
        self.log.push(format!("set_cursor:{icon:?}"));
    }
    fn set_cursor_visible(&mut self, on: bool) {
        self.log.push(format!("set_cursor_visible:{on}"));
    }
    fn set_cursor_mode(&mut self, mode: CursorMode) {
        self.log.push(format!("set_cursor_mode:{mode:?}"));
    }
    fn set_cursor_position(&mut self, pos: Point) {
        self.log.push(format!("set_cursor_position:{pos:?}"));
    }
    fn set_ime_allowed(&mut self, on: bool) {
        self.log.push(format!("set_ime_allowed:{on}"));
    }
    fn set_ime_cursor_area(&mut self, area: Rect) {
        self.log.push(format!("set_ime_cursor_area:{area:?}"));
    }
    fn set_ime_purpose(&mut self, purpose: ImePurpose) {
        self.log.push(format!("set_ime_purpose:{purpose:?}"));
    }
    fn focus_window(&mut self) {
        self.log.push("focus_window".into());
    }
    fn request_attention(&mut self, kind: Option<Attention>) {
        self.log.push(format!("request_attention:{kind:?}"));
    }
    fn request_redraw(&mut self) {
        self.log.push("request_redraw".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::Ticket;

    #[test]
    fn set_title_and_cursor_are_recorded() {
        let mut w = RecordingWindow::default();
        assert!(execute(&mut w, WindowCommand::SetTitle("hi".into())).is_none());
        assert!(execute(&mut w, WindowCommand::SetCursor(CursorIcon::Text)).is_none());
        assert_eq!(w.log, ["set_title:hi", "set_cursor:Text"]);
    }

    #[test]
    fn set_outer_rect_echoes() {
        let mut w = RecordingWindow::default();
        let follow = execute(
            &mut w,
            WindowCommand::SetOuterRect {
                position: (10, 20),
                size: SizePx::new(800, 600),
            },
        );
        assert_eq!(
            follow,
            Some(ExecutorFollowUp::OuterRectEcho {
                position: (10, 20),
                size: SizePx::new(800, 600),
            })
        );
    }

    #[test]
    fn clipboard_and_spawn_are_follow_ups() {
        let mut w = RecordingWindow::default();
        let ticket = Ticket(7);
        assert_eq!(
            execute(&mut w, WindowCommand::ClipboardRead(ticket)),
            Some(ExecutorFollowUp::ClipboardRead(ticket))
        );
        assert_eq!(
            execute(&mut w, WindowCommand::ClipboardWrite("x".into())),
            Some(ExecutorFollowUp::ClipboardWrite("x".into()))
        );
        let spec = WindowSpec::new("k", "t", SizePx::new(100, 100));
        assert!(matches!(
            execute(&mut w, WindowCommand::Spawn(spec)),
            Some(ExecutorFollowUp::Spawn(_))
        ));
        assert_eq!(
            execute(&mut w, WindowCommand::ExitApp),
            Some(ExecutorFollowUp::ExitApp)
        );
        assert!(w.log.is_empty());
    }

    #[test]
    fn idempotent_reapply_is_fine() {
        let mut w = RecordingWindow::default();
        let _ = execute(&mut w, WindowCommand::SetVisible(true));
        let _ = execute(&mut w, WindowCommand::SetVisible(true));
        assert_eq!(w.log, ["set_visible:true", "set_visible:true"]);
    }
}
