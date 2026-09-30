//! Engine operations and effects: the closed vocabulary the kernel uses to
//! drive each engine (`XOp`, the argument of the engine's `apply`) and the
//! consequences an engine reports back (`XEffect`), which the kernel conducts
//! into ops on other engines, intents or host commands (design §3.0, §3.8).
//!
//! These are kernel-facing, not app-facing: the app speaks
//! [`AppCommand`](crate::AppCommand); the kernel translates it into ops.
//! This module grows one op / effect pair per engine brief (F2: windows,
//! cadence, animation; F3: input, keymap).

use uzor::render::{InvalidateBits, TickRate};
use uzor::{Rect, WidgetId};

use crate::types::anim::{AnimKey, AnimPolicy};
use crate::types::bus::{
    ClipboardResult, ImeInput, KeyInput, PointerInput, RenderInfo, WheelInput, WindowInput,
};
use crate::types::command::{CadenceCmd, ClipboardCmd, FocusCmd, KeymapCmd, ThemeCmd, WindowCmd};
use crate::types::frame::TimerOwner;
use crate::types::ids::{OverlaySlot, RegionId, ScopeId, Seconds, Ticket, TimerToken, WindowId};
use crate::types::intent::TextIntent;
use crate::types::window::{RenderCmd, WindowCommand, WindowSpec};

// ---------------------------------------------------------------------------
// WindowEngine
// ---------------------------------------------------------------------------

/// One mutation of the WindowEngine.
#[derive(Clone, Debug)]
pub enum WindowOp {
    /// Allocate a window id and ask the host to spawn it
    /// ([`WindowEffect::Spawned`]). Runtime start-up windows, the app's
    /// `WindowCmd::Open` and drag-out micro-windows all come through here.
    Create(WindowSpec),
    /// A host lifecycle echo for one window (lifecycle phase).
    Lifecycle {
        /// The window the envelope named.
        win: WindowId,
        /// The echo.
        input: WindowInput,
    },
    /// An app window command.
    Cmd(WindowCmd),
    /// An app theme command.
    Theme(ThemeCmd),
    /// An app render-backend command, forwarded to the primary window's host.
    Render(RenderCmd),
    /// An app clipboard command, forwarded to the primary window's host.
    Clipboard(ClipboardCmd),
    /// Capture a window.
    Screenshot {
        /// Window to capture.
        win: WindowId,
        /// Correlation ticket.
        ticket: Ticket,
    },
    /// Leave the host loop.
    Shutdown,
    /// Another engine's host-bound command, conducted by the kernel
    /// (e.g. `SetCursor` from the InputEngine, `SetOuterRect` from layout).
    Enqueue {
        /// Target window.
        win: WindowId,
        /// The command.
        cmd: WindowCommand,
    },
    /// Fresh render facts from the host (`HostEvent::RenderInfo`).
    RenderInfo(RenderInfo),
}

/// A consequence of a WindowEngine op other engines or the app must see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowEffect {
    /// A window id was allocated and `WindowCommand::Spawn` enqueued; the
    /// host answers with `WindowInput::Created` for this id.
    Spawned(WindowId),
    /// The host created the window; it is open.
    Created(WindowId),
    /// The window is gone (host `Destroyed` echo); drop its entries in every
    /// engine.
    Closed(WindowId),
    /// The user asked to close the window under `ClosePolicy::AskApp`.
    CloseRequested(WindowId),
    /// The window's logical viewport or device pixel ratio changed.
    GeometryChanged(WindowId),
    /// OS keyboard focus moved into (`true`) or out of the window.
    FocusChanged {
        /// The window.
        win: WindowId,
        /// Focus gained.
        focused: bool,
    },
    /// The app-wide tokens or the dark flag changed.
    ThemeChanged,
}

// ---------------------------------------------------------------------------
// CadenceEngine
// ---------------------------------------------------------------------------

/// One mutation of the CadenceEngine.
#[derive(Clone, Debug, PartialEq)]
pub enum CadenceOp {
    /// Start scheduling a window (conducted from `WindowEffect::Created`);
    /// its first frame is due at once.
    Open {
        /// The window.
        win: WindowId,
        /// Its base tick rate (window spec, else the runtime default).
        tick: TickRate,
    },
    /// Stop scheduling a window and disarm its window-scoped deadlines
    /// (conducted from `WindowEffect::Closed`).
    Close(WindowId),
    /// An app cadence command.
    Cmd(CadenceCmd),
    /// Conducted invalidation (resize, theme, hover, overlay, ...).
    Invalidate {
        /// Target window.
        win: WindowId,
        /// Region, or `None` for the whole window.
        region: Option<RegionId>,
        /// What changed.
        bits: InvalidateBits,
    },
    /// Arm (or re-arm, replacing) a framework deadline.
    Arm {
        /// Who is woken.
        owner: TimerOwner,
        /// Host-clock instant.
        at: Seconds,
    },
    /// Disarm a framework deadline.
    Disarm(TimerOwner),
}

/// A consequence of a CadenceEngine op or tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CadenceEffect {
    /// A deadline was armed under this token (an earlier one of the same
    /// owner, if any, was replaced).
    Armed {
        /// The owner.
        owner: TimerOwner,
        /// The new token.
        token: TimerToken,
    },
    /// A deadline elapsed; the kernel converts the owner
    /// (`App(n)` -> `Intent::Timer(n)`, `OverlayAutoClose` -> close, ...).
    Fired {
        /// The token it was armed under.
        token: TimerToken,
        /// The owner.
        owner: TimerOwner,
    },
}

// ---------------------------------------------------------------------------
// AnimationEngine
// ---------------------------------------------------------------------------

/// One mutation of the AnimationEngine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimOp {
    /// Point an animator at a new target, creating it at `0.0` if absent.
    SetTarget {
        /// The animator.
        key: AnimKey,
        /// Target value, normally `0.0` or `1.0`.
        target: f64,
        /// Host clock now; an animator at rest starts timing from here.
        now: Seconds,
    },
    /// Drop one animator.
    Remove(AnimKey),
    /// Drop every animator of a window (conducted from `WindowEffect::Closed`).
    DropWindow(WindowId),
    /// Replace the durations; applies to running animators too.
    SetPolicy(AnimPolicy),
}

/// A consequence of an AnimationEngine tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimEffect {
    /// The animator's value moved this tick.
    Value {
        /// The animator.
        key: AnimKey,
        /// New value.
        t: f64,
    },
    /// The animator reached its target this tick (follows its last `Value`).
    Finished {
        /// The animator.
        key: AnimKey,
        /// Final value (= target).
        t: f64,
    },
}

// ---------------------------------------------------------------------------
// InputEngine
// ---------------------------------------------------------------------------

/// Who opened a focus scope; a scope is popped by the same owner.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScopeOwner {
    /// An open overlay instance (conducted from `OverlayEffect::Opened`).
    Overlay(OverlaySlot),
    /// A widget that wants scoped tab order without being an overlay
    /// (e.g. a non-modal floating panel).
    Widget(WidgetId),
}

/// An engine that can hold the pointer (splitter / tab / resize drags,
/// overlay resize).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EngineTarget {
    /// The LayoutEngine (splitters, tab drags, drag-out, chrome).
    Layout,
    /// The OverlayEngine, for one overlay instance.
    Overlay(OverlaySlot),
}

/// The pointer's owner while captured: every pointer event goes to it,
/// regardless of what is under the cursor, until the button is released.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Capture {
    /// A content widget (also held as the coordinator's pointer grab).
    Widget(WidgetId),
    /// An engine.
    Engine(EngineTarget),
}

/// One keyboard-focus mutation of a window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusOp {
    /// Focus a widget; a no-op when a scope is active and `id` is not one
    /// of its members.
    Set(WidgetId),
    /// Clear focus.
    Clear,
    /// Move focus to the next widget (scoped to the active scope).
    Next,
    /// Move focus to the previous widget (scoped to the active scope).
    Prev,
    /// Open a scope: the current focus is saved and cleared, and from now
    /// on focus may only move among `members` (in tab order).
    PushScope {
        /// Who opens it.
        owner: ScopeOwner,
        /// The focusable widgets inside, in tab order.
        members: Vec<WidgetId>,
    },
    /// Replace the member list of an open scope (e.g. once the overlay body
    /// has been composed and its widgets are known).
    SetMembers {
        /// The scope's owner.
        owner: ScopeOwner,
        /// The new members, in tab order.
        members: Vec<WidgetId>,
    },
    /// Close the owner's scope. With `restore`, focus returns to the widget
    /// saved when the scope opened (when the scope was the active one).
    PopScope {
        /// The scope's owner.
        owner: ScopeOwner,
        /// Restore the saved focus (the overlay's `restore_focus` policy).
        restore: bool,
    },
}

impl FocusOp {
    /// The window and focus op an app [`FocusCmd`] names.
    pub fn from_cmd(cmd: FocusCmd) -> (WindowId, Self) {
        match cmd {
            FocusCmd::Focus { win, id } => (win, Self::Set(id)),
            FocusCmd::Clear { win } => (win, Self::Clear),
            FocusCmd::Next { win } => (win, Self::Next),
            FocusCmd::Prev { win } => (win, Self::Prev),
        }
    }
}

/// One mutation of the InputEngine. Time-dependent ops carry the host
/// clock (`now`, the envelope's `t`); the engine never reads a clock.
#[derive(Clone, Debug, PartialEq)]
pub enum InputOp {
    /// Start tracking a window (conducted from `WindowEffect::Created`).
    Open(WindowId),
    /// Drop a window's input state (conducted from `WindowEffect::Closed`).
    Close(WindowId),
    /// OS keyboard focus entered or left the window
    /// (conducted from `WindowEffect::FocusChanged`).
    WindowFocus {
        /// The window.
        win: WindowId,
        /// Focus gained.
        focused: bool,
        /// Host clock.
        now: Seconds,
    },
    /// Routing step 1: cook a pointer event (click count, drag arm) and say
    /// whether a capture owns it. Answered by [`InputEffect::Pointer`].
    Pointer {
        /// The window.
        win: WindowId,
        /// Host clock at capture.
        now: Seconds,
        /// The event.
        input: PointerInput,
    },
    /// Routing step 5: hand a pointer event nothing above claimed to the
    /// content widgets (it enters the coordinator's next frame input).
    /// Answered by [`InputEffect::Content`].
    Deliver {
        /// The window.
        win: WindowId,
        /// The event.
        input: PointerInput,
    },
    /// A wheel step for the content widgets. Answered by
    /// [`InputEffect::Content`].
    Wheel {
        /// The window.
        win: WindowId,
        /// The event.
        input: WheelInput,
    },
    /// Routing key step 1: the focused text field's editing chords and
    /// text, and Tab / Shift+Tab. Anything else is answered by
    /// [`InputEffect::KeyPassed`].
    Key {
        /// The window.
        win: WindowId,
        /// Host clock.
        now: Seconds,
        /// The event.
        input: KeyInput,
    },
    /// An input-method event for the focused text field.
    Ime {
        /// The window.
        win: WindowId,
        /// Host clock.
        now: Seconds,
        /// The event.
        input: ImeInput,
    },
    /// The host's answer to a clipboard read the engine asked for.
    Clipboard {
        /// The window.
        win: WindowId,
        /// Host clock.
        now: Seconds,
        /// The answer.
        result: ClipboardResult,
    },
    /// A focus mutation (app command, conducted overlay open / close).
    Focus {
        /// The window.
        win: WindowId,
        /// Host clock.
        now: Seconds,
        /// The mutation.
        op: FocusOp,
    },
    /// Set or clear the pointer capture (conducted from
    /// `LayoutEffect::Capture`, overlay resize, or a widget grab).
    SetCapture {
        /// The window.
        win: WindowId,
        /// The new owner, or `None` to release.
        capture: Option<Capture>,
    },
    /// Compose start: feed the frame's pointer input into the coordinator
    /// and clear last frame's clicks and registrations.
    BeginFrame {
        /// The window.
        win: WindowId,
        /// Host clock (stamped into the coordinator's `InputState::time`).
        now: Seconds,
    },
    /// Compose end: evaluate this frame's registrations (hover, clicks).
    EndFrame {
        /// The window.
        win: WindowId,
        /// Host clock.
        now: Seconds,
    },
}

/// Where a cooked pointer event goes next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerTarget {
    /// A capture owns it: deliver to the owner and stop routing.
    Captured(Capture),
    /// No capture: continue with overlays, layout zones, content.
    HitTest,
}

/// A pointer event after routing step 1 (cook).
#[derive(Clone, Debug, PartialEq)]
pub struct CookedPointer {
    /// The window.
    pub win: WindowId,
    /// The event.
    pub input: PointerInput,
    /// Clicks in the current sequence (1 single, 2 double, 3 triple); set
    /// by the press and kept through its release.
    pub click_count: u8,
    /// The held press has travelled past the drag threshold.
    pub drag_armed: bool,
    /// This event is the one that crossed the threshold.
    pub drag_started: bool,
    /// This release ended an armed drag (so it is not a click).
    pub ended_drag: bool,
    /// Where it goes next.
    pub target: PointerTarget,
}

/// A consequence of an InputEngine op or tick.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEffect {
    /// Answer to [`InputOp::Pointer`].
    Pointer(CookedPointer),
    /// Answer to [`InputOp::Deliver`] / [`InputOp::Wheel`]: the content
    /// widget under the pointer (or the grabbing widget); `None` when
    /// nothing is there (routing step 6: unhandled).
    Content {
        /// The window.
        win: WindowId,
        /// The widget, if any.
        target: Option<WidgetId>,
    },
    /// Key step 1 did not consume the key: route on (overlay Escape,
    /// keymap, focused widget, unhandled). Absent = consumed.
    KeyPassed {
        /// The window.
        win: WindowId,
        /// The event.
        key: KeyInput,
    },
    /// A text field changed or was submitted; the kernel pushes it to the
    /// intents.
    Text(TextIntent),
    /// Copy / cut produced clipboard text; the kernel enqueues
    /// `WindowCommand::ClipboardWrite`.
    CopyText {
        /// The window.
        win: WindowId,
        /// The text.
        text: String,
    },
    /// Paste needs the clipboard; the kernel enqueues
    /// `WindowCommand::ClipboardRead(ticket)` and the host's answer comes
    /// back as [`InputOp::Clipboard`].
    NeedPaste {
        /// The window.
        win: WindowId,
        /// The engine-minted ticket.
        ticket: Ticket,
    },
    /// The caret moved; the kernel enqueues
    /// `WindowCommand::SetImeCursorArea`. Emitted only on change.
    ImeArea {
        /// The window.
        win: WindowId,
        /// Caret rect, window-local logical pixels.
        rect: Rect,
    },
    /// A text field gained or lost focus; the kernel enqueues
    /// `WindowCommand::SetImeAllowed`. Emitted only on change.
    ImeAllowed {
        /// The window.
        win: WindowId,
        /// IME on.
        allowed: bool,
    },
    /// The caret blink deadline of the window: `Some(at)` -> the kernel
    /// applies `CadenceOp::Arm` for `TimerOwner::CaretBlink { win }` at
    /// `at` (replacing any earlier one); `None` -> `CadenceOp::Disarm`.
    /// Emitted only on change.
    CaretDeadline {
        /// The window.
        win: WindowId,
        /// Next blink edge, or `None` to disarm.
        at: Option<Seconds>,
    },
    /// Repaint only the focused field (blink edge, edit, focus move). The
    /// kernel maps `rect` to the narrowest region with
    /// `CadenceEngineView::region_containing` (`None` or no region: the
    /// whole window) and applies `CadenceOp::Invalidate` with
    /// `InvalidateBits::MATERIAL`.
    InvalidateField {
        /// The window.
        win: WindowId,
        /// The field's last drawn rect, if it was ever drawn.
        rect: Option<Rect>,
    },
    /// A focus scope opened.
    ScopePushed {
        /// The window.
        win: WindowId,
        /// Its owner.
        owner: ScopeOwner,
        /// Its engine-minted id.
        scope: ScopeId,
    },
}

// ---------------------------------------------------------------------------
// KeymapEngine
// ---------------------------------------------------------------------------

/// One mutation of the KeymapEngine: bindings change only through app
/// commands (`AppCommand::Keymap`). `O` / `A` are the app's overlay and
/// action types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapOp<O, A> {
    /// An app keymap command.
    Cmd(KeymapCmd<O, A>),
}

/// A consequence of a KeymapEngine op. Uninhabited: a binding edit affects
/// no other engine, and resolution is a read (`KeymapEngine::resolve`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeymapEffect {}
