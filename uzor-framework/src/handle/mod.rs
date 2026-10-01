//! ROLE handle: the app's only door into on-screen state (design §6.1).
//!
//! [`Handle`] is `Clone + Send + Sync`: a back-office task thread can
//! dispatch a visual command ("open the error modal") without touching the
//! UI thread, and read the last published snapshot lock-free. This module
//! and the hosts are the only places channels and `Arc` exist (SWC Law
//! 2-3, ban F5). There is no `subscribe`: observers poll
//! [`Handle::revision`]; the in-thread event channel is [`App::intent`].
//!
//! [`App::intent`]: crate::handle::App::intent

mod app;

pub use app::{
    App, FrameTime, HookOp, HookOps, InitCx, IntentCx, OverlayCx, PanelCx, VisualView, Widgets,
};

use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::types::command::AppCommand;
use crate::types::ids::Revision;
use crate::types::snapshot::VisualSnapshot;
use crate::types::spec::Spec;

/// Bounded command inbox capacity (design §6.1).
pub(crate) const INBOX_CAP: usize = 1024;

/// Wakes the host loop after a dispatch so a command never waits for the
/// next input event. Native: `EventLoopProxy::send_event`; web: a RAF
/// request; headless: a no-op.
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    /// A waker from any thread-safe closure.
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// A waker that does nothing (headless host, tests).
    pub fn noop() -> Self {
        Self(Arc::new(|| {}))
    }

    /// Ask the host loop to run a tick.
    pub fn wake(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waker").finish_non_exhaustive()
    }
}

/// Why a dispatch failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HandleError {
    /// The inbox is at capacity; the command was NOT queued (never a
    /// silent drop). The app should coalesce and retry.
    #[error("inbox full")]
    Full,
    /// The runtime is gone; the command went nowhere.
    #[error("runtime stopped")]
    Closed,
}

/// The app's door into on-screen state (design §6.1).
///
/// Cheap to clone, `Send + Sync` when the spec's vocabularies are (given
/// by [`Spec`]). Produced by [`Runtime::new`](crate::Runtime::new); the
/// app hands clones to whatever threads need them.
pub struct Handle<S: Spec> {
    pub(crate) tx: SyncSender<AppCommand<S>>,
    pub(crate) snap: Arc<ArcSwap<VisualSnapshot<S>>>,
    pub(crate) waker: Waker,
}

impl<S: Spec> Clone for Handle<S> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            snap: Arc::clone(&self.snap),
            waker: self.waker.clone(),
        }
    }
}

impl<S: Spec> std::fmt::Debug for Handle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle")
            .field("revision", &self.revision())
            .finish_non_exhaustive()
    }
}

impl<S: Spec> Handle<S> {
    /// Enqueue a visual command and wake the loop. Never blocks; a full
    /// inbox surfaces as [`HandleError::Full`], a dead runtime as
    /// [`HandleError::Closed`].
    pub fn dispatch(&self, cmd: AppCommand<S>) -> Result<(), HandleError> {
        self.tx.try_send(cmd).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => HandleError::Full,
            std::sync::mpsc::TrySendError::Disconnected(_) => HandleError::Closed,
        })?;
        self.waker.wake();
        Ok(())
    }

    /// The last published snapshot: lock-free, at most one tick old.
    pub fn snapshot(&self) -> Arc<VisualSnapshot<S>> {
        self.snap.load_full()
    }

    /// The published revision; observers compare it to know whether to
    /// re-read the snapshot.
    pub fn revision(&self) -> Revision {
        self.snap.load().revision
    }
}
