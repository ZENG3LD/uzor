//! [`AgentControl`] — trait the platform window manager implements so
//! the HTTP server can read snapshots and queue write commands.
//!
//! The window manager owns the `LayoutManager`.  We borrow it through
//! this trait in two directions:
//!
//! - **Read**: snapshot is rebuilt on the winit/event-loop thread at
//!   the end of each tick and stashed into an `Arc<RwLock<...>>`.
//!   `latest_snapshot` returns a clone.
//! - **Write**: HTTP handlers queue a [`super::Command`] through
//!   `dispatch`, then await the [`super::CommandReply`] on the returned
//!   oneshot.  The manager drains the queue on the next tick before
//!   running the LM solve.

use std::sync::Arc;

use tokio::sync::oneshot;

use super::command::{Command, CommandReply};
use super::model::{AgentSnapshot, WidgetSnapshot};

/// Implemented by each platform window manager (`uzor-desktop::Manager`,
/// future web/mobile equivalents).
///
/// `Send + Sync` because the axum handlers call it from a different
/// thread than the manager's own event loop.  Implementations typically
/// hold `Arc<RwLock<AgentSnapshot>>` for the read path and
/// `mpsc::UnboundedSender<(Command, oneshot::Sender<CommandReply>)>`
/// for the write path.
pub trait AgentControl: Send + Sync + 'static {
    /// Latest read-only snapshot.  Cheap clone — backed by `Arc<RwLock>`.
    fn latest_snapshot(&self) -> AgentSnapshot;

    /// Detailed widget list across all windows (rect + sense + layer).
    /// Built on demand because it's larger than the small snapshot.
    fn widget_list(&self) -> Vec<WidgetSnapshot>;

    /// Queue a write command.  Returns a oneshot that resolves once the
    /// manager processes it on the next tick.  The Sender side of the
    /// oneshot is owned by the manager.
    fn dispatch(&self, cmd: Command) -> oneshot::Receiver<CommandReply>;
}

/// Convenience alias used by [`super::server::spawn_server`].
pub type AgentControlArc = Arc<dyn AgentControl>;
