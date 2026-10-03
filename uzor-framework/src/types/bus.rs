//! The input bus: the one value every host pushes into the runtime.
//!
//! Coordinates are window-local logical pixels (hosts divide by the device
//! pixel ratio before pushing), and every pointer / key / wheel event carries
//! its own modifiers, so no host keeps shadow input state.

use std::path::PathBuf;
use std::sync::Arc;

use uzor::input::{KeyCode, ModifierKeys, MouseButton};
use uzor::RenderBackend;

use crate::types::ids::{Seconds, Ticket, TrayItemId, WindowId};
use crate::types::window::{Point, SizePx};

/// One input item crossing from a host into the kernel.
///
/// Produced by hosts (native mapper, web listeners, headless host); consumed
/// by the kernel's drain / lifecycle / route phases, in `(t, arrival)` order.
#[derive(Clone, Debug, PartialEq)]
pub struct InputEnvelope {
    /// The window the event belongs to.
    pub window: WindowId,
    /// Host monotonic clock at capture.
    pub t: Seconds,
    /// The event itself.
    pub event: InputEvent,
}

impl InputEnvelope {
    /// Construct an envelope.
    pub fn new(window: WindowId, t: Seconds, event: InputEvent) -> Self {
        Self { window, t, event }
    }
}

/// Every kind of input the kernel accepts; one variant per input kind, one
/// path per kind into the kernel.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// Mouse / pen pointer.
    Pointer(PointerInput),
    /// Scroll wheel / trackpad scroll.
    Wheel(WheelInput),
    /// Touch contact.
    Touch(TouchInput),
    /// Physical key press / repeat / release.
    Key(KeyInput),
    /// Input-method composition.
    Ime(ImeInput),
    /// File drag and drop.
    Drop(DropInput),
    /// Answer to a `WindowCommand::ClipboardRead`.
    Clipboard(ClipboardResult),
    /// The host woke the loop at the instant the kernel asked for.
    Timer(TimerWake),
    /// Window lifecycle.
    Window(WindowInput),
    /// Results of commands the host executed, tray selections, render facts.
    Host(HostEvent),
}

/// Pointer events; positions are window-local logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerInput {
    /// Pointer moved.
    Moved {
        /// New position.
        pos: Point,
        /// Modifiers held.
        mods: ModifierKeys,
    },
    /// Button pressed.
    Down {
        /// Press position.
        pos: Point,
        /// Which button.
        button: MouseButton,
        /// Modifiers held.
        mods: ModifierKeys,
    },
    /// Button released.
    Up {
        /// Release position.
        pos: Point,
        /// Which button.
        button: MouseButton,
        /// Modifiers held.
        mods: ModifierKeys,
    },
    /// Pointer entered the window.
    Entered,
    /// Pointer left the window.
    Left,
    /// The OS took the gesture away (e.g. a system drag started).
    Cancelled,
    /// Raw motion, only while the cursor is `CursorMode::LockedHidden`.
    RawDelta {
        /// Horizontal motion.
        dx: f64,
        /// Vertical motion.
        dy: f64,
    },
}

/// A wheel / trackpad scroll step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelInput {
    /// Pointer position at the time of the scroll.
    pub pos: Point,
    /// Scroll amount with an explicit unit.
    pub delta: WheelDelta,
    /// Modifiers held.
    pub mods: ModifierKeys,
}

/// Scroll amount; the unit is always explicit, never guessed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelDelta {
    /// Logical pixels (trackpads, normalised browser wheel).
    Pixels {
        /// Horizontal amount.
        x: f64,
        /// Vertical amount.
        y: f64,
    },
    /// Lines (classic notched wheels).
    Lines {
        /// Horizontal amount.
        x: f64,
        /// Vertical amount.
        y: f64,
    },
}

/// Touch contacts, mirroring the lib's touch platform events (no modifiers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchInput {
    /// A contact started.
    Start {
        /// Contact id, stable for the contact's lifetime.
        id: u64,
        /// Position.
        pos: Point,
    },
    /// A contact moved.
    Move {
        /// Contact id.
        id: u64,
        /// Position.
        pos: Point,
    },
    /// A contact lifted.
    End {
        /// Contact id.
        id: u64,
        /// Position.
        pos: Point,
    },
    /// The OS cancelled a contact.
    Cancel {
        /// Contact id.
        id: u64,
    },
}

/// Phase of a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyState {
    /// First press.
    Down,
    /// Auto-repeat while held.
    Repeat,
    /// Release.
    Up,
}

/// One key event.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInput {
    /// Physical, layout-independent key.
    pub code: KeyCode,
    /// Logical text produced by this press, if any.
    pub text: Option<String>,
    /// Press / repeat / release.
    pub state: KeyState,
    /// Modifiers held.
    pub mods: ModifierKeys,
}

/// Input-method composition events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeInput {
    /// The IME became active for the window.
    Enabled,
    /// Composition in progress; never enters a text buffer before commit.
    Preedit {
        /// Current composition text.
        text: String,
        /// Caret / selection inside `text` as byte offsets, if any.
        cursor: Option<(usize, usize)>,
    },
    /// Composition finished with this text.
    Commit(String),
    /// The IME became inactive.
    Disabled,
}

/// File drag-and-drop events.
#[derive(Clone, Debug, PartialEq)]
pub enum DropInput {
    /// A file is dragged over the window.
    Hovered(DroppedFile),
    /// A file was dropped.
    Dropped(DroppedFile),
    /// The drag left or was aborted.
    Cancelled,
}

/// One dragged / dropped file. Native hosts fill `path`, web hosts `bytes`;
/// the framework never opens the path itself.
#[derive(Clone, Debug, PartialEq)]
pub struct DroppedFile {
    /// File name as reported by the OS / browser.
    pub name: String,
    /// Filesystem path (native only).
    pub path: Option<PathBuf>,
    /// File contents (web only), shared immutably.
    pub bytes: Option<Arc<[u8]>>,
}

/// Answer to `WindowCommand::ClipboardRead`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardResult {
    /// The ticket from the read command.
    pub ticket: Ticket,
    /// Clipboard text, or `None` when empty / not text / denied.
    pub text: Option<String>,
}

/// The host woke the loop at the instant the kernel requested via `Wake::At`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerWake {
    /// Host clock at wake-up.
    pub now: Seconds,
}

/// What the host can do; decides degradation paths without code forks in the
/// kernel (e.g. no expand gutters or drag-out windows on the web).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HostCaps {
    /// The host can open more than one OS window.
    pub multi_window: bool,
    /// The OS provides resize borders itself (no uzor bezel hit zone).
    pub os_resize_bezel: bool,
    /// Clipboard reads complete asynchronously.
    pub clipboard_async: bool,
}

/// Window lifecycle events; all land in the lifecycle phase, before any
/// pointer event of the same tick is routed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowInput {
    /// The host created the window (initial or after `WindowCommand::Spawn`).
    Created {
        /// Inner size, physical pixels.
        size: SizePx,
        /// Device pixel ratio.
        dpr: f64,
        /// Outer position, if known.
        position: Option<(i32, i32)>,
        /// Host capabilities for this window.
        caps: HostCaps,
    },
    /// Inner size or DPI changed.
    Resized {
        /// New inner size, physical pixels.
        size: SizePx,
        /// Device pixel ratio.
        dpr: f64,
    },
    /// The window moved.
    Moved {
        /// New outer position, physical screen pixels.
        position: (i32, i32),
    },
    /// Keyboard focus gained (`true`) or lost.
    Focused(bool),
    /// Fully occluded (`true`) or visible again.
    Occluded(bool),
    /// Minimized (`true`) or restored.
    Minimized(bool),
    /// Maximized (`true`) or restored.
    Maximized(bool),
    /// The OS theme changed.
    ThemeChanged {
        /// Dark theme active.
        dark: bool,
    },
    /// The user asked to close the window.
    CloseRequested,
    /// The window is gone.
    Destroyed,
    /// Echo after `WindowCommand::SetOuterRect`.
    OuterRect {
        /// Outer position, physical screen pixels.
        position: (i32, i32),
        /// Outer size, physical pixels.
        size: SizePx,
    },
}

/// Read-only render facts the host reports after submitting frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderInfo {
    /// Active 2D backend, once one is running.
    pub backend: Option<RenderBackend>,
    /// Backends this host can switch to.
    pub available: Vec<RenderBackend>,
    /// Exponential moving average of frames per second.
    pub fps_ema: f64,
    /// Last frame time in milliseconds.
    pub frame_ms: f64,
    /// Frames submitted so far.
    pub frame_count: u64,
}

/// Results of commands the host executed, and host-only sources.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    /// Answer to `WindowCommand::Screenshot`: PNG bytes, never a file.
    Screenshot {
        /// The ticket from the command.
        ticket: Ticket,
        /// Encoded PNG.
        png: Arc<[u8]>,
    },
    /// A tray menu entry was selected (native only).
    Tray {
        /// The selected entry.
        id: TrayItemId,
    },
    /// Fresh render facts.
    RenderInfo(RenderInfo),
}
