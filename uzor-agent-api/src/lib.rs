//! `uzor-agent-api` — local HTTP control plane for any uzor app.
//!
//! Sits next to the platform-specific window manager (`uzor-desktop`,
//! `uzor-window-web`, `uzor-window-mobile`) as a *peer* L4 helper.  The
//! window manager owns the `LayoutManager`; this crate borrows snapshot
//! access and write-command channels from it through the
//! [`AgentControl`] trait.
//!
//! ## Goals
//!
//! - Read everything: window list, branches, sync registry, widget
//!   rects, hovered/pressed/clicked, modal state, fps.
//! - Write everything LM can do: spawn windows, open/close modals,
//!   add/remove edges, dock split, restyle, promote/demote sync nodes.
//! - Inject synthetic pointer / keyboard / scroll events keyed by
//!   `WindowKey` so external agents can drive the UI exactly like a
//!   human would.
//!
//! ## Status
//!
//! First-pass: read snapshot + 5 endpoints
//! (`/health`, `/state/tree`, `/state/widgets`, `/input/click`,
//! `/input/hover`).  Direct LM write commands are added incrementally.
//!
//! ## Usage
//!
//! Each platform window manager provides an `AgentControl` impl.  The
//! consumer (e.g. `uzor-desktop::AppBuilder::agent_api(port)`) hands it
//! to [`spawn_server`] which launches an axum server on the given port.

pub mod command;
pub mod control;
pub mod model;
pub mod server;

pub use command::{Command, CommandReply, MouseButton};
pub use control::AgentControl;
pub use model::{
    AgentSnapshot, BranchSnapshot, ClickSnap, NodeSyncSnapshot, RectSnap, RootSnapshot,
    WidgetSnapshot,
};
pub use server::{spawn_server, AgentApiHandle};
