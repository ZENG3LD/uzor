//! Agent-API plumbing for [`crate::manager::Manager`].
//!
//! Bridges `uzor::layout::agent::AgentControl` (defined in core uzor)
//! to the platform window manager.  Read snapshot lives in an
//! `Arc<RwLock<...>>` rebuilt at the end of each tick; write commands
//! ride a sync mpsc and are drained on the winit thread before the
//! next solve.
//!
//! The manager handles two command classes itself:
//! - **Window lifecycle** (`SpawnWindow`, `CloseWindow`) — needs winit.
//! - **Synthetic input** (`InjectClick`, `InjectHover`, `InjectScroll`) —
//!   acts on its own `pw.last_mouse_pos` + the LM via `set_current_window`.
//!
//! Every other command is forwarded to
//! [`uzor::layout::agent::LmAgent::try_apply`].

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, Receiver, Sender};

use uzor::docking::panels::DockPanel;
use uzor::layout::LayoutManager;

use uzor::layout::agent::{
    AgentControl, AgentSnapshot, Command, CommandReply, LmAgent, WidgetSnapshot,
};

/// Internal-only "command" the manager handles outside of the public
/// `Command` enum.  Carries a screenshot request keyed by window name
/// and a one-shot reply channel that returns PNG bytes.
pub(crate) struct ScreenshotRequest {
    pub window: String,
    pub reply:  Sender<Option<Vec<u8>>>,
}

/// One pending command + the channel its caller is blocking on.
pub(crate) type PendingCmd = (Command, Sender<CommandReply>);

/// Plumbing the manager owns; cloned (Arc-wrapped) into the HTTP server.
pub(crate) struct AgentBus {
    pub snapshot: Arc<RwLock<AgentSnapshot>>,
    pub widgets:  Arc<RwLock<Vec<WidgetSnapshot>>>,
    pub cmd_tx:   Sender<PendingCmd>,
    pub cmd_rx:   Receiver<PendingCmd>,
    pub shot_tx:  Sender<ScreenshotRequest>,
    pub shot_rx:  Receiver<ScreenshotRequest>,
}

impl AgentBus {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (stx, srx) = mpsc::channel();
        Self {
            snapshot: Arc::new(RwLock::new(empty_snapshot())),
            widgets:  Arc::new(RwLock::new(Vec::new())),
            cmd_tx: tx,
            cmd_rx: rx,
            shot_tx: stx,
            shot_rx: srx,
        }
    }

    pub fn control(&self) -> Arc<DesktopAgentControl> {
        Arc::new(DesktopAgentControl {
            snapshot: Arc::clone(&self.snapshot),
            widgets:  Arc::clone(&self.widgets),
            cmd_tx:   self.cmd_tx.clone(),
            shot_tx:  self.shot_tx.clone(),
        })
    }
}

/// Desktop trait-object wired into [`uzor_agent_api::spawn_server`].
pub struct DesktopAgentControl {
    snapshot: Arc<RwLock<AgentSnapshot>>,
    widgets:  Arc<RwLock<Vec<WidgetSnapshot>>>,
    cmd_tx:   Sender<PendingCmd>,
    shot_tx:  Sender<ScreenshotRequest>,
}

impl AgentControl for DesktopAgentControl {
    fn snapshot(&self) -> AgentSnapshot {
        self.snapshot.read().expect("agent snapshot lock").clone()
    }

    fn widgets(&self) -> Vec<WidgetSnapshot> {
        self.widgets.read().expect("agent widgets lock").clone()
    }

    fn dispatch(&self, cmd: Command) -> CommandReply {
        let (tx, rx) = mpsc::channel();
        if self.cmd_tx.send((cmd, tx)).is_err() {
            return CommandReply::err("manager command channel closed");
        }
        rx.recv().unwrap_or_else(|_| CommandReply::err("manager dropped reply"))
    }

    fn screenshot_png(&self, window: &str) -> Option<Vec<u8>> {
        let (tx, rx) = mpsc::channel();
        let req = ScreenshotRequest {
            window: window.to_owned(),
            reply: tx,
        };
        if self.shot_tx.send(req).is_err() { return None; }
        // Block on the manager's reply (drained in `about_to_wait`).
        rx.recv().ok().flatten()
    }
}

// ── snapshot / widget rebuild — called from the manager tick ─────────

fn empty_snapshot() -> AgentSnapshot {
    LmAgent::<uzor::framework::app::NoPanel>::build_snapshot(
        &LayoutManager::<uzor::framework::app::NoPanel>::new(),
        0.0, 0, 0.0,
    )
}

pub(crate) fn build_snapshot<P: DockPanel>(
    layout: &LayoutManager<P>,
    fps_ema: f32,
    frame_count: u64,
    frame_time_ms: f64,
) -> AgentSnapshot {
    LmAgent::<P>::build_snapshot(layout, fps_ema, frame_count, frame_time_ms)
}

pub(crate) fn build_widget_list<P: DockPanel>(
    layout: &LayoutManager<P>,
) -> Vec<WidgetSnapshot> {
    LmAgent::<P>::build_widget_list(layout)
}
