//! Engine operations and effects: the closed vocabulary the kernel uses to
//! drive each engine (`XOp`, the argument of the engine's `apply`) and the
//! consequences an engine reports back (`XEffect`), which the kernel conducts
//! into ops on other engines, intents or host commands (design §3.0, §3.8).
//!
//! These are kernel-facing, not app-facing: the app speaks
//! [`AppCommand`](crate::AppCommand); the kernel translates it into ops.
//! This module grows one op / effect pair per engine brief (F2: windows,
//! cadence, animation).

use uzor::render::{InvalidateBits, TickRate};

use crate::types::anim::{AnimKey, AnimPolicy};
use crate::types::bus::{RenderInfo, WindowInput};
use crate::types::command::{CadenceCmd, ClipboardCmd, ThemeCmd, WindowCmd};
use crate::types::frame::TimerOwner;
use crate::types::ids::{RegionId, Seconds, Ticket, TimerToken, WindowId};
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
