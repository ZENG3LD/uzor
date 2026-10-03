//! `uzor-agent-api` — HTTP shim over [`uzor::layout::agent::AgentControl`].
//!
//! All types are defined in `uzor::layout::agent`; this crate is just
//! the axum + tokio transport layer.  Each platform window manager
//! (e.g. `uzor-desktop::Manager`) implements `AgentControl` and hands
//! it to [`spawn_server`].

pub mod server;

pub use server::{spawn_server, AgentApiHandle};

// Re-exports so callers can pull everything from one crate.
pub use uzor::layout::agent::{
    AgentControl, AgentControlObj, AgentSnapshot, BranchSnapshot, ClickSnap, Command,
    CommandReply, LmAgent, MouseButton, NodeSyncSnapshot, RectSnap, RootSnapshot,
    WidgetSnapshot,
};
