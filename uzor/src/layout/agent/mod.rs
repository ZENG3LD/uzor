//! Agent control surface for `LayoutManager`.
//!
//! Exposes a platform-agnostic API that lets external agents (LLMs,
//! QA tooling, scripts, IDE plugins) read live LM state and drive
//! every UI operation LM understands.
//!
//! Two operating modes for write-side calls:
//!
//! - **Semantic / direct LM ops** (default).  Address things by their
//!   logical id: `ClickWidget { widget_id }`, `OpenModal { id }`,
//!   `ToggleSidebar`, `ApplyStylePreset`, `SetSyncMode`.  No pixel
//!   coordinates required, deterministic, robust against re-layout.
//!
//! - **Pixel + screenshot** (escape hatch).  `InjectClick { x, y }`,
//!   `InjectHover`, plus `Screenshot` from the WM trait.  Used for
//!   custom blackbox bodies (charts, sketch areas) where the widget
//!   tree cannot describe a clickable region.
//!
//! ## Layering
//!
//! - The [`AgentControl`] trait is defined here in uzor core so any
//!   platform window manager can implement it.
//! - [`LmAgent<P>`] is a default implementation that handles every
//!   call routable purely through `LayoutManager` — read snapshot,
//!   click_widget, modal/sidebar/dropdown ops, sync-registry edits,
//!   style presets.
//! - Window managers extend it with platform-specific operations
//!   (screenshot, spawn/close OS window, real synthetic input).
//!   They typically wrap an `LmAgent` and forward the calls it can
//!   answer, then add their own.

mod blackbox;
mod command;
mod control;
mod lm_agent;
mod log;
mod snapshot;

pub use blackbox::{AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface};
pub use command::{Command, CommandReply, MouseButton};
pub use control::{AgentControl, AgentControlObj};
pub use lm_agent::LmAgent;
pub use log::{AgentLog, AgentLogEntry, DEFAULT_LOG_CAPACITY};
pub use snapshot::{
    AgentSnapshot, BranchSnapshot, ClickSnap, NodeSyncSnapshot, RectSnap, RootSnapshot,
    WidgetSnapshot,
};
