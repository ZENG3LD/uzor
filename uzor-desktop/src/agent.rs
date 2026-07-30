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
type WakeCallback = Arc<dyn Fn() -> bool + Send + Sync>;

/// Cross-thread wake bridge for synchronous Agent API commands.
///
/// The HTTP server runs outside winit's event-loop thread. Merely enqueueing a
/// command is not enough when every window uses `TickRate::Dirty`: winit may be
/// sleeping indefinitely while the HTTP handler blocks waiting for the reply.
/// The manager installs an `EventLoopProxy` callback before exposing the
/// server; its user event wakes winit so `about_to_wait` can drain the queue.
#[derive(Clone, Default)]
struct AgentWake {
    callback: Arc<RwLock<Option<WakeCallback>>>,
}

impl AgentWake {
    fn set(&self, callback: WakeCallback) {
        *self.callback.write().expect("agent wake lock") = Some(callback);
    }

    fn notify(&self) -> bool {
        let callback = self.callback.read().expect("agent wake lock").clone();
        callback.is_some_and(|callback| callback())
    }
}

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
    wake: AgentWake,
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
            wake: AgentWake::default(),
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
            wake:       self.wake.clone(),
        })
    }

    pub fn set_waker(
        &self,
        callback: impl Fn() -> bool + Send + Sync + 'static,
    ) {
        self.wake.set(Arc::new(callback));
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
    wake:       AgentWake,
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
        if !self.wake.notify() {
            return CommandReply::err("manager event loop wake failed");
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
        if !self.wake.notify() { return None; }
        // Block on the manager's reply (drained after the wake reaches winit).
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn synchronous_dispatch_wakes_before_waiting_for_manager_reply() {
        let bus = AgentBus::new();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&wake_count);
        bus.wake.set(Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
            true
        }));
        let control = bus.control();

        let caller = std::thread::spawn(move || {
            control.dispatch(Command::LogPush {
                category: "wake-test".to_owned(),
                payload: serde_json::Value::Null,
                window: None,
            })
        });

        let (_cmd, reply) = bus.cmd_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("agent command reached manager queue");
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        reply.send(CommandReply::ok()).expect("caller still waiting");
        assert!(caller.join().expect("dispatch thread").ok);
    }

    #[test]
    fn failed_wake_returns_without_waiting_for_manager_reply() {
        let bus = AgentBus::new();
        bus.wake.set(Arc::new(|| false));
        let control = bus.control();

        let reply = control.dispatch(Command::LogPush {
            category: "wake-failure-test".to_owned(),
            payload: serde_json::Value::Null,
            window: None,
        });

        assert!(!reply.ok);
        assert_eq!(
            reply.message.as_deref(),
            Some("manager event loop wake failed"),
        );
        assert!(bus.cmd_rx.try_recv().is_ok());
    }
}
