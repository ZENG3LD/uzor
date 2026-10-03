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
//! Briefs F1-F6: the `types` role (the input bus, the command vocabulary,
//! intents, the visual snapshot, window commands, frame requests, the layout
//! blob, the [`Spec`] vocabulary trait, engine ops / effects) and seven
//! engines in the crate-internal `engine` module (windows, cadence +
//! deadline wheel, animation, input, keymap, overlays, layout with expand
//! and drag-out). F7: the kernel
//! (phase pipeline, conduction per design §3.8, routing per §4.3), the
//! [`App`] contract with the hook contexts, [`Handle`], [`Runtime`] and the
//! headless host in [`host`]. F8: [`widgets`] convenience functions over the
//! hook contexts, [`flex`] and [`profiler`]. F9: native host behind feature
//! `native` (`host::native`). F10: web host behind feature `web`
//! (`host::web`: listeners, RAF, executor, canvas2d paint). F11: the demo
//! (`examples/demo`, one `DemoApp`, native and web entries) and the §9.4
//! goldens. M1: [`view!`] retargets onto [`widgets`] / [`flex`].
//!
//! ## Data flow
//!
//! Hosts push [`InputEnvelope`]s in; the app sends [`AppCommand`]s in; the
//! kernel publishes a [`VisualSnapshot`], typed [`Intent`]s, host-bound
//! [`WindowCommand`]s and [`FrameRequest`]s out. Everything in this crate is
//! generic over one app-supplied [`Spec`] (panel / overlay / action types).

#![deny(missing_docs)]

pub(crate) mod engine;
pub mod flex;
pub mod handle;
pub mod host;
mod kernel;
pub mod profiler;
mod runtime;
pub mod types;
pub mod widgets;

/// `view!` JSX-like DSL — re-exported from `uzor-framework-macros` (brief M1).
pub use uzor_framework_macros::view;

pub use handle::{
    App, FrameTime, Handle, HandleError, HookOp, HookOps, InitCx, IntentCx, OverlayCx, PanelCx,
    VisualView, Waker, Widgets,
};
pub use host::HeadlessHost;
pub use runtime::{Runtime, RuntimeConfig, TickOutput, MAX_SETTLE};

pub use types::anim::{AnimKey, AnimPolicy, ExpandKind};

pub use types::bus::{
    ClipboardResult, DropInput, DroppedFile, HostCaps, HostEvent, ImeInput, InputEnvelope,
    InputEvent, KeyInput, KeyState, PointerInput, RenderInfo, TimerWake, TouchInput, WheelDelta,
    WheelInput, WindowInput,
};
pub use types::command::{
    AppCommand, Binding, CadenceCmd, ChromeKind, ChromeModel, ChromeTab, ClipboardCmd, DockTarget,
    Domain, DragOutChrome, DragOutPolicy, FocusCmd, KeymapCmd, KeymapScope, LayoutCmd, LayoutHit,
    LayoutPolicy, OverlayCmd, OverlayPolicy, OverlaySize, SplitDir, SplitterPolicy, ThemeCmd,
    WindowCmd,
};
pub use types::error::FrameworkError;
pub use types::frame::{FrameRequest, RegionPlan, RegionSpec, TimerOwner, Wake, WindowSurface};
pub use types::ids::{
    OrderedSeconds, OverlaySlot, RegionId, Revision, ScopeId, Seconds, Ticket, TimerToken,
    TrayItemId, WindowId,
};
pub use types::intent::{
    CloseCause, DockIntent, DropIntent, Intent, OverlayIntent, TextIntent, UnhandledInput,
    WindowIntent,
};
pub use types::layout_blob::{LayoutBlob, LayoutCodecError, WindowGeometrySnapshot};
pub use types::overlay_model::{
    ContextMenuEntry, ContextMenuModel, DropdownModel, ModalModel, OverlayModel, TooltipModel,
};
pub use types::snapshot::{
    CadenceView, ChromeView, DockView, EdgeSlotView, FloatingView, InputView, LeafView,
    OverlayView, PanelRef, SeparatorView, VisualSnapshot, WindowView,
};
pub use types::spec::{PanelHome, Spec};
pub use types::window::{
    Attention, ClosePolicy, CursorMode, FullscreenMode, ImePurpose, Point, RenderCmd, SizePx,
    ThemeHint, WindowCommand, WindowGeometry, WindowSpec,
};
