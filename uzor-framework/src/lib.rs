//! # uzor-framework — the app engine
//!
//! ```text
//! Role: kernel (+ engines, handle, host shells by module — see module table)
//! Owns: on-screen state only — windows, layout / dock trees, overlay stacks, focus scopes, pointer
//!       capture, hover / press, keymap bindings, cadence, animation.  Nothing else.
//! Exports: Handle<S>, App / Spec, AppCommand<S>, Intent<S>, VisualSnapshot<S>, InputEnvelope /
//!       InputEvent, WindowCommand, Runtime, hosts behind features `native` / `web`, headless host.
//! Imports: uzor (widgets, input, docking, tokens), uzor-render-hub (hosts only),
//!       uzor-window-desktop (feature native), uzor-window-web (feature web).
//! Forbidden: winit / web-sys / render backends inside kernel, engine, types, handle, runtime;
//!       HTTP, MCP, scripting, plugins, persistence, file I/O, business data anywhere;
//!       locks / threads / RefCell outside handle and hosts; pub fields on kernel / engine structs;
//!       a second door into on-screen state (the old LayoutManager / framework::* paths).
//!
//! Module roles: types | engine/* | kernel | handle | runtime (shell) | host/* (shell).
//! Gate: CICD/uzor/scripts/gate-next.sh --crates uzor-framework  (release profile; never
//!       --workspace --all-targets).  Bans F1-F10 live in CICD/uzor/scripts/forbidden-deps.toml.
//! ```
//!
//! ## Status
//!
//! Only the `types` role exists so far (brief F1): the input bus, the command
//! vocabulary, intents, the visual snapshot, window commands, frame requests,
//! the layout blob and the [`Spec`] vocabulary trait. Engines, kernel, handle,
//! runtime and hosts follow in later briefs; `Handle`, `App`, `Runtime` and the
//! hosts named in the header above do not exist yet.
//!
//! ## Data flow
//!
//! Hosts push [`InputEnvelope`]s in; the app sends [`AppCommand`]s in; the
//! kernel publishes a [`VisualSnapshot`], typed [`Intent`]s, host-bound
//! [`WindowCommand`]s and [`FrameRequest`]s out. Everything in this crate is
//! generic over one app-supplied [`Spec`] (panel / overlay / action types).

#![deny(missing_docs)]

pub mod types;

pub use types::bus::{
    ClipboardResult, DropInput, DroppedFile, HostCaps, HostEvent, ImeInput, InputEnvelope,
    InputEvent, KeyInput, KeyState, PointerInput, RenderInfo, TimerWake, TouchInput, WheelDelta,
    WheelInput, WindowInput,
};
pub use types::command::{
    AppCommand, Binding, CadenceCmd, ClipboardCmd, DockTarget, Domain, DragOutChrome,
    DragOutPolicy, FocusCmd, KeymapCmd, KeymapScope, LayoutCmd, LayoutHit, LayoutPolicy,
    OverlayCmd, OverlayPolicy, OverlaySize, SplitDir, SplitterPolicy, ThemeCmd, WindowCmd,
};
pub use types::error::FrameworkError;
pub use types::frame::{FrameRequest, RegionPlan, RegionSpec, TimerOwner, Wake};
pub use types::ids::{
    OverlaySlot, RegionId, Revision, ScopeId, Seconds, Ticket, TimerToken, TrayItemId, WindowId,
};
pub use types::intent::{
    CloseCause, DockIntent, DropIntent, Intent, OverlayIntent, TextIntent, UnhandledInput,
    WindowIntent,
};
pub use types::layout_blob::{LayoutBlob, LayoutCodecError, WindowGeometrySnapshot};
pub use types::snapshot::{
    CadenceView, ChromeView, DockView, EdgeSlotView, FloatingView, InputView, LeafView,
    OverlayView, PanelRef, SeparatorView, VisualSnapshot, WindowView,
};
pub use types::spec::Spec;
pub use types::window::{
    Attention, CursorMode, FullscreenMode, ImePurpose, Point, RenderCmd, SizePx, ThemeHint,
    WindowCommand, WindowGeometry, WindowSpec,
};
