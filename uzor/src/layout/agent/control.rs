//! [`AgentControl`] — sync trait every platform window manager
//! implements so external tooling (HTTP shim, IDE plugin, test
//! harness) can drive LM uniformly across desktop / web / mobile.
//!
//! Sync rather than async because the WM applies commands on its own
//! event-loop thread.  The HTTP shim wraps the call in a oneshot if
//! the underlying transport (axum + tokio) is async.

use std::sync::Arc;

use super::command::{Command, CommandReply};
use super::snapshot::{AgentSnapshot, WidgetSnapshot};

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
}

/// Convenience alias for sharing the trait object across threads.
pub type AgentControlObj = Arc<dyn AgentControl>;
