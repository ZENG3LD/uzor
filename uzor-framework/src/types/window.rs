//! Window vocabulary: the one host-bound command enum, window spec and
//! geometry, and the small OS-facing enums (cursor mode, IME purpose, ...).
//!
//! [`WindowCommand`] is the only thing a host executor receives from the
//! kernel. It replaces the old `WindowHost` trait and per-window host
//! adapters; every command passes through the WindowEngine queue, so the
//! output vocabulary has one writer and one drain point.

use serde::{Deserialize, Serialize};
use uzor::layout::window::WindowKey;
use uzor::render::TickRate;
use uzor::{CornerStyle, CursorIcon, Rect, RenderBackend, ResizeDirection, RgbaIcon};

use crate::types::ids::Ticket;

/// A position in window-local logical pixels.
///
/// Produced by hosts (already divided by the device pixel ratio) and by
/// engines; consumed by routing, hit-testing and cursor commands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal offset from the window's left edge.
    pub x: f64,
    /// Vertical offset from the window's top edge.
    pub y: f64,
}

impl Point {
    /// Construct a point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A size in physical (device) pixels, as the OS reports and accepts it.
///
/// Produced by hosts (window created / resized echoes) and by the LayoutEngine
/// (expand / drag-out outer rects); consumed by host executors and frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SizePx {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

impl SizePx {
    /// Construct a size.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// How the OS cursor behaves inside a window.
///
/// Requested by the app (`WindowCmd::SetCursorMode`) or an engine; executed by
/// the host (native grab mode, web Pointer Lock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorMode {
    /// Free cursor (default).
    #[default]
    Normal,
    /// Cursor confined to the window rect.
    Confined,
    /// Cursor hidden and locked in place; motion arrives as
    /// [`PointerInput::RawDelta`](crate::PointerInput::RawDelta).
    LockedHidden,
}

/// Hint to the OS input method about the focused field.
///
/// Set by the InputEngine when a text field gains focus; executed by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImePurpose {
    /// Plain text entry.
    #[default]
    Normal,
    /// Password entry (IME suggestions off where the OS allows).
    Password,
    /// Terminal-like entry.
    Terminal,
}

/// Fullscreen state requested for a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FullscreenMode {
    /// Regular window.
    #[default]
    Off,
    /// Borderless fullscreen on the window's current monitor.
    Borderless,
}

/// Theme hint passed to the OS for its own window decorations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeHint {
    /// Light decorations.
    Light,
    /// Dark decorations.
    Dark,
}

/// Kind of user-attention request (taskbar flash / dock bounce).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attention {
    /// Persistent request until the window is focused.
    Critical,
    /// One-shot, low-priority request.
    Informational,
}

/// Render-backend control, executed by the host against its render hub.
///
/// Typed replacement for the old `SetRenderBackend(String)` command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderCmd {
    /// Switch the active 2D backend.
    SetBackend(RenderBackend),
    /// Cap presentation at this many frames per second (0 = no cap).
    SetFpsLimit(u32),
    /// Multisample count (1 = off).
    SetMsaa(u8),
    /// Enable or disable vsync.
    SetVsync(bool),
}

/// How to create one window.
///
/// Produced by the app (`WindowCmd::Open`, runtime config) or the LayoutEngine
/// (drag-out micro-window); consumed by the WindowEngine and, through
/// [`WindowCommand::Spawn`], the host. Spawn-time-only OS properties
/// (transparency, blur, content protection) live here and nowhere else.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSpec {
    /// Stable app label for the window; survives sessions.
    pub key: WindowKey,
    /// Initial title.
    pub title: String,
    /// Initial inner size.
    pub inner_size: SizePx,
    /// Minimum inner size, if any.
    pub min_inner_size: Option<SizePx>,
    /// Initial outer position; `None` lets the OS place the window.
    pub position: Option<(i32, i32)>,
    /// OS decorations (title bar, borders). `false` = uzor chrome.
    pub decorations: bool,
    /// Whether the user may resize the window.
    pub resizable: bool,
    /// Whether the window starts visible.
    pub visible: bool,
    /// Transparent surface (spawn-time only).
    pub transparent: bool,
    /// Background blur behind a transparent surface (spawn-time only).
    pub blur: bool,
    /// Exclude the window from screen capture (spawn-time only).
    pub content_protected: bool,
    /// Corner rounding preference.
    pub corner_style: CornerStyle,
    /// Border colour as `0xRRGGBB`, where the OS supports it.
    pub border_color: Option<u32>,
    /// Repaint cadence; `None` inherits the runtime default.
    pub tick: Option<TickRate>,
}

impl WindowSpec {
    /// A visible, resizable, OS-decorated window with the given label, title
    /// and inner size; every other field at its neutral default.
    pub fn new(key: impl Into<String>, title: impl Into<String>, inner_size: SizePx) -> Self {
        Self {
            key: WindowKey(key.into()),
            title: title.into(),
            inner_size,
            min_inner_size: None,
            position: None,
            decorations: true,
            resizable: true,
            visible: true,
            transparent: false,
            blur: false,
            content_protected: false,
            corner_style: CornerStyle::Default,
            border_color: None,
            tick: None,
        }
    }
}

/// OS-facing geometry and state flags of one window.
///
/// Written only by the WindowEngine from host lifecycle echoes; published in
/// [`WindowView`](crate::WindowView).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowGeometry {
    /// Content viewport in window-local logical pixels (origin at 0, 0).
    pub viewport: Rect,
    /// Outer size in physical pixels (last host echo).
    pub outer_size: SizePx,
    /// Outer position in physical screen pixels, when the host knows it.
    pub position: Option<(i32, i32)>,
    /// Window is maximized.
    pub maximized: bool,
    /// Window is minimized.
    pub minimized: bool,
    /// Window is fully occluded.
    pub occluded: bool,
    /// Window has OS keyboard focus.
    pub focused: bool,
    /// Device pixel ratio (physical / logical).
    pub scale: f64,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            viewport: Rect::default(),
            outer_size: SizePx::default(),
            position: None,
            maximized: false,
            minimized: false,
            occluded: false,
            focused: false,
            scale: 1.0,
        }
    }
}

/// The complete host-bound command vocabulary.
///
/// Produced only through the WindowEngine's queue (engines' effects and app
/// commands are enqueued there); drained by the runtime into the tick output
/// and executed by the host executor as one flat `match`. Executors are
/// idempotent, never fail toward the kernel, and never invent commands.
#[derive(Clone, Debug)]
pub enum WindowCommand {
    /// Paint this window on the next frame.
    RequestRedraw,
    /// Set the OS title.
    SetTitle(String),
    /// Set or clear the window icon.
    SetIcon(Option<RgbaIcon>),
    /// Show or hide OS decorations.
    SetDecorations(bool),
    /// Corner rounding preference.
    SetCornerStyle(CornerStyle),
    /// Border colour `0xRRGGBB`, or the OS default.
    SetBorderColor(Option<u32>),
    /// Theme hint for OS decorations, or follow the system.
    SetTheme(Option<ThemeHint>),
    /// Show or hide the window.
    SetVisible(bool),
    /// Minimize or restore.
    SetMinimized(bool),
    /// Maximize or restore.
    SetMaximized(bool),
    /// Enter or leave fullscreen.
    SetFullscreen(FullscreenMode),
    /// Allow or forbid user resizing.
    SetResizable(bool),
    /// Minimum inner size, or none.
    SetMinInnerSize(Option<SizePx>),
    /// Maximum inner size, or none.
    SetMaxInnerSize(Option<SizePx>),
    /// Atomic move + resize; the host echoes `WindowInput::OuterRect`.
    SetOuterRect {
        /// New outer position, physical screen pixels.
        position: (i32, i32),
        /// New outer size, physical pixels.
        size: SizePx,
    },
    /// Ask the OS for a new inner size.
    RequestInnerSize(SizePx),
    /// Start an OS window move with the pressed button.
    DragWindow,
    /// Start an OS window resize from the given edge / corner.
    DragResizeWindow(ResizeDirection),
    /// Show the OS window menu at a window-local physical position.
    ShowWindowMenu {
        /// Menu anchor, physical pixels relative to the window.
        at: (i32, i32),
    },
    /// Set the cursor icon.
    SetCursor(CursorIcon),
    /// Show or hide the cursor.
    SetCursorVisible(bool),
    /// Cursor grab / lock mode.
    SetCursorMode(CursorMode),
    /// Warp the cursor to a window-local logical position.
    SetCursorPosition(Point),
    /// Enable or disable the OS input method for this window.
    SetImeAllowed(bool),
    /// Tell the IME where the caret is (window-local logical rect).
    SetImeCursorArea(Rect),
    /// IME purpose hint.
    SetImePurpose(ImePurpose),
    /// Write text to the system clipboard.
    ClipboardWrite(String),
    /// Read the clipboard; the host answers with `InputEvent::Clipboard`
    /// carrying the same ticket.
    ClipboardRead(Ticket),
    /// Bring the window to the front and focus it.
    FocusWindow,
    /// Request user attention, or cancel a pending request.
    RequestAttention(Option<Attention>),
    /// Create a new OS window; the host answers with `WindowInput::Created`.
    Spawn(WindowSpec),
    /// Close this window.
    Close,
    /// Leave the event loop / stop the page loop.
    ExitApp,
    /// Render-backend control.
    Render(RenderCmd),
    /// Capture the window; the host answers with `HostEvent::Screenshot`.
    Screenshot(Ticket),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_spec_defaults() {
        let s = WindowSpec::new("main", "Demo", SizePx::new(800, 600));
        assert_eq!(s.key.as_str(), "main");
        assert!(s.decorations && s.resizable && s.visible);
        assert!(!s.transparent && !s.blur && !s.content_protected);
        assert_eq!(s.tick, None);
    }

    #[test]
    fn geometry_default_scale_is_one() {
        assert_eq!(WindowGeometry::default().scale, 1.0);
    }
}
