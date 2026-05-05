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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{self, Receiver, Sender};

use uzor::docking::panels::DockPanel;
use uzor::layout::LayoutManager;

use uzor::layout::agent::{
    AgentAction, AgentActionReply, AgentControl, AgentLogEntry, AgentSnapshot,
    AgentWidget, BlackboxAgentSurface, Command, CommandReply, LmAgent,
    WidgetSnapshot,
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

type BlackboxRegistry = HashMap<String, Arc<Mutex<dyn BlackboxAgentSurface>>>;

/// Plumbing the manager owns; cloned (Arc-wrapped) into the HTTP server.
pub(crate) struct AgentBus {
    pub snapshot: Arc<RwLock<AgentSnapshot>>,
    pub widgets:  Arc<RwLock<Vec<WidgetSnapshot>>>,
    pub log:      Arc<RwLock<Vec<AgentLogEntry>>>,
    /// Mirror of `LayoutManager::blackbox_agents` so HTTP handlers
    /// running on the agent-api thread can lock blackbox surfaces
    /// directly without bouncing every read through the cmd channel.
    pub blackboxes: Arc<RwLock<BlackboxRegistry>>,
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
            log:      Arc::new(RwLock::new(Vec::new())),
            blackboxes: Arc::new(RwLock::new(HashMap::new())),
            cmd_tx: tx,
            cmd_rx: rx,
            shot_tx: stx,
            shot_rx: srx,
        }
    }

    pub fn control(&self) -> Arc<DesktopAgentControl> {
        Arc::new(DesktopAgentControl {
            snapshot:   Arc::clone(&self.snapshot),
            widgets:    Arc::clone(&self.widgets),
            log:        Arc::clone(&self.log),
            blackboxes: Arc::clone(&self.blackboxes),
            cmd_tx:     self.cmd_tx.clone(),
            shot_tx:    self.shot_tx.clone(),
        })
    }
}

/// Desktop trait-object wired into [`uzor_agent_api::spawn_server`].
pub struct DesktopAgentControl {
    snapshot:   Arc<RwLock<AgentSnapshot>>,
    widgets:    Arc<RwLock<Vec<WidgetSnapshot>>>,
    log:        Arc<RwLock<Vec<AgentLogEntry>>>,
    blackboxes: Arc<RwLock<BlackboxRegistry>>,
    cmd_tx:     Sender<PendingCmd>,
    shot_tx:    Sender<ScreenshotRequest>,
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

    fn log_since(&self, since: u64, limit: usize) -> Vec<AgentLogEntry> {
        let guard = self.log.read().expect("agent log lock");
        guard.iter()
            .filter(|e| e.seq > since)
            .take(limit)
            .cloned()
            .collect()
    }

    fn log_tail(&self, n: usize) -> Vec<AgentLogEntry> {
        let guard = self.log.read().expect("agent log lock");
        let len = guard.len();
        let start = len.saturating_sub(n);
        guard.iter().skip(start).cloned().collect()
    }

    fn blackbox_slots(&self) -> Vec<String> {
        let guard = self.blackboxes.read().expect("blackbox registry lock");
        let mut v: Vec<String> = guard.keys().cloned().collect();
        v.sort();
        v
    }

    fn blackbox_widgets(&self, slot_id: &str) -> Option<Vec<AgentWidget>> {
        let registry = self.blackboxes.read().expect("blackbox registry lock");
        let surface = registry.get(slot_id)?.clone();
        drop(registry); // release before locking the surface
        let guard = surface.lock().ok()?;
        Some(guard.list_agent_widgets())
    }

    fn blackbox_state(&self, slot_id: &str) -> Option<serde_json::Value> {
        let registry = self.blackboxes.read().expect("blackbox registry lock");
        let surface = registry.get(slot_id)?.clone();
        drop(registry);
        let guard = surface.lock().ok()?;
        Some(guard.agent_state())
    }

    fn blackbox_action(&self, slot_id: &str, action: AgentAction) -> Option<AgentActionReply> {
        let registry = self.blackboxes.read().expect("blackbox registry lock");
        let surface = registry.get(slot_id)?.clone();
        drop(registry);
        let mut guard = surface.lock().ok()?;
        Some(guard.apply_agent_action(action))
    }

    fn blackbox_click_widget(
        &self,
        slot_id: &str,
        sub_id: &str,
    ) -> Option<CommandReply> {
        // Click goes through the command channel so the synthetic
        // pointer events land on the winit thread.  We need the
        // window key — fish it out of the latest snapshot's
        // current_window.  Caller can override via `window` param
        // in the HTTP layer.
        let window = self.snapshot.read().ok()?.root.current_window.clone()?;
        let cmd = Command::BlackboxClickWidget {
            window,
            slot_id: slot_id.to_owned(),
            sub_id: sub_id.to_owned(),
        };
        Some(self.dispatch(cmd))
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
