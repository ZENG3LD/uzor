//! [`AgentControl`] — sync trait every platform window manager
//! implements so external tooling (HTTP shim, IDE plugin, test
//! harness) can drive LM uniformly across desktop / web / mobile.
//!
//! Sync rather than async because the WM applies commands on its own
//! event-loop thread.  The HTTP shim wraps the call in a oneshot if
//! the underlying transport (axum + tokio) is async.

use std::sync::Arc;

use super::blackbox::{AgentAction, AgentActionReply, AgentWidget};
use super::command::{Command, CommandReply};
use super::log::AgentLogEntry;
use super::snapshot::{AgentSnapshot, WidgetSnapshot};
use serde_json::Value;

pub trait AgentControl: Send + Sync + 'static {
    /// Latest snapshot of LM state.  Cheap clone — backed by an
    /// internal `Arc<RwLock>`.
    fn snapshot(&self) -> AgentSnapshot;

    /// Flat widget list across every attached window.  Built on
    /// demand because it can be much larger than the snapshot.
    fn widgets(&self) -> Vec<WidgetSnapshot>;

    /// Apply one command on the WM's event-loop thread.  May be called
    /// from another thread — implementations queue the command into
    /// their own channel and block (or async-await) until the WM
    /// drains it.
    fn dispatch(&self, cmd: Command) -> CommandReply;

    /// PNG-encoded screenshot of the named window's most recent frame.
    /// Default impl returns `None`; window managers backed by a GPU
    /// override it.
    fn screenshot_png(&self, _window: &str) -> Option<Vec<u8>> {
        None
    }

    /// Drain agent-log entries with `seq > since`, capped at `limit`.
    /// Default returns empty so transports without a log surface
    /// (TUI, mobile) keep working.
    fn log_since(&self, _since: u64, _limit: usize) -> Vec<AgentLogEntry> {
        Vec::new()
    }

    /// Last `n` log entries.  Default returns empty.
    fn log_tail(&self, _n: usize) -> Vec<AgentLogEntry> {
        Vec::new()
    }

    // ── Blackbox routing ─────────────────────────────────────────────
    //
    // LM keeps a routing table from `slot_id` to the corresponding
    // `BlackboxAgentSurface` trait object.  These methods expose that
    // table to the HTTP shim.  Default impls return empty so
    // platforms without blackbox panels (TUI) keep working.

    /// All blackbox `slot_id`s currently registered.
    fn blackbox_slots(&self) -> Vec<String> { Vec::new() }

    /// Mini-widget list published by the named blackbox.  `None` if
    /// the slot isn't registered.
    fn blackbox_widgets(&self, _slot_id: &str) -> Option<Vec<AgentWidget>> { None }

    /// Internal-state snapshot from the named blackbox.
    fn blackbox_state(&self, _slot_id: &str) -> Option<Value> { None }

    /// Forward a typed action to the named blackbox.
    fn blackbox_action(&self, _slot_id: &str, _action: AgentAction) -> Option<AgentActionReply> {
        None
    }

    /// Synthetic click on a published mini-widget.  Returns the
    /// rect's centre that was clicked, or `None` if the widget /
    /// slot isn't found.
    fn blackbox_click_widget(&self, _slot_id: &str, _sub_id: &str) -> Option<CommandReply> {
        None
    }
}

/// Convenience alias for sharing the trait object across threads.
pub type AgentControlObj = Arc<dyn AgentControl>;
