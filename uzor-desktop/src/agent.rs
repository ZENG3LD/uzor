//! Agent-API plumbing for [`crate::manager::Manager`].
//!
//! Sits between `uzor-agent-api` (HTTP server, platform-agnostic) and
//! the Manager (winit-bound).  Responsibilities:
//!
//! - Hold an [`Arc<RwLock<AgentSnapshot>>`] the manager refreshes at
//!   the end of every tick.
//! - Hold an unbounded mpsc channel of `(Command, oneshot<Reply>)` —
//!   the HTTP server enqueues, the manager drains in
//!   `about_to_wait` BEFORE the next solve.
//! - Provide an [`AgentControlImpl`] type the server calls for both
//!   read and write paths.

use std::sync::{Arc, RwLock};

use tokio::sync::{mpsc, oneshot};

use uzor::docking::panels::DockPanel;
use uzor::layout::LayoutManager;

use uzor_agent_api::{
    AgentControl, AgentSnapshot, BranchSnapshot, ClickSnap, Command, CommandReply,
    NodeSyncSnapshot, RectSnap, RootSnapshot, WidgetSnapshot,
};

/// Plumbing the manager owns; shared (clones) with the HTTP server.
pub(crate) struct AgentBus {
    pub snapshot: Arc<RwLock<AgentSnapshot>>,
    pub cmd_tx: mpsc::UnboundedSender<(Command, oneshot::Sender<CommandReply>)>,
    pub cmd_rx: mpsc::UnboundedReceiver<(Command, oneshot::Sender<CommandReply>)>,
}

impl AgentBus {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            snapshot: Arc::new(RwLock::new(empty_snapshot())),
            cmd_tx: tx,
            cmd_rx: rx,
        }
    }

    /// Build the trait object the HTTP server drives.
    pub fn control(&self) -> Arc<AgentControlImpl> {
        Arc::new(AgentControlImpl {
            snapshot: Arc::clone(&self.snapshot),
            cmd_tx: self.cmd_tx.clone(),
            widgets: Arc::new(RwLock::new(Vec::new())),
        })
    }
}

/// Implements [`AgentControl`] by reading the shared snapshot and
/// pushing into the manager's mpsc.
pub struct AgentControlImpl {
    snapshot: Arc<RwLock<AgentSnapshot>>,
    cmd_tx: mpsc::UnboundedSender<(Command, oneshot::Sender<CommandReply>)>,
    /// Widget list refreshed by the manager tick, kept separate from
    /// the small snapshot because it can grow much larger.
    pub(crate) widgets: Arc<RwLock<Vec<WidgetSnapshot>>>,
}

impl AgentControl for AgentControlImpl {
    fn latest_snapshot(&self) -> AgentSnapshot {
        self.snapshot.read().expect("agent snapshot lock").clone()
    }

    fn widget_list(&self) -> Vec<WidgetSnapshot> {
        self.widgets.read().expect("agent widgets lock").clone()
    }

    fn dispatch(&self, cmd: Command) -> oneshot::Receiver<CommandReply> {
        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send((cmd, tx)).is_err() {
            // Receiver dropped — return a closed rx so the caller sees an error.
            let (closed_tx, closed_rx) = oneshot::channel();
            drop(closed_tx);
            return closed_rx;
        }
        rx
    }
}

// ── snapshot building ────────────────────────────────────────────────

fn empty_snapshot() -> AgentSnapshot {
    AgentSnapshot {
        root: RootSnapshot {
            current_window: None,
            window_count: 0,
            style_preset: None,
        },
        windows: Vec::new(),
        sync_nodes: Vec::new(),
        frame_time_ms: 0.0,
        frame_count: 0,
        fps_ema: 0.0,
    }
}

/// Build an [`AgentSnapshot`] from the live `LayoutManager`.  Called by
/// the manager at the end of each tick.
pub(crate) fn build_snapshot<P: DockPanel>(
    layout: &LayoutManager<P>,
    fps_ema: f32,
    frame_count: u64,
    frame_time_ms: f64,
) -> AgentSnapshot {
    use uzor::layout::sync::SyncMode;

    let current_window = layout.current_window().map(|k| k.as_str().to_owned());
    let window_count = layout.window_keys().count();

    let windows: Vec<BranchSnapshot> = layout
        .window_keys()
        .filter_map(|key| {
            let b = layout.window(key)?;
            Some(BranchSnapshot {
                key: key.as_str().to_owned(),
                rect: RectSnap {
                    x: b.rect.x,
                    y: b.rect.y,
                    w: b.rect.width,
                    h: b.rect.height,
                },
                initialised: b.initialised,
                chrome_visible: b.chrome.visible,
                edge_count: b.edges.iter().count(),
                dock_leaves: b.dock.tree().leaves().len(),
                overlay_count: b.overlays.entries().len(),
                modal_count: b.modals.len(),
                popup_count: b.popups.len(),
                dropdown_count: b.dropdowns.len(),
                toolbar_count: b.toolbars.len(),
                sidebar_count: b.sidebars.len(),
                context_menu_count: b.context_menus.len(),
                hovered_widget: b.last_hovered.as_ref().map(|w| w.as_str().to_owned()),
                pressed_widget: b.last_pressed.as_ref().map(|w| w.as_str().to_owned()),
                last_click: b.last_click.as_ref().map(|(id, (x, y))| ClickSnap {
                    widget: id.as_str().to_owned(),
                    pos: [*x, *y],
                }),
                pointer_pos: b.last_pointer_pos.map(|(x, y)| [x, y]),
            })
        })
        .collect();

    let sync_nodes: Vec<NodeSyncSnapshot> = layout
        .sync_registry()
        .iter()
        .map(|(node_id, mode)| {
            let (label, group_id) = match mode {
                SyncMode::Synced => ("synced", None),
                SyncMode::Sometimes(None) => ("sometimes_alone", None),
                SyncMode::Sometimes(Some(g)) => ("sometimes_group", Some(g.0)),
                SyncMode::Standalone => ("standalone", None),
            };
            NodeSyncSnapshot {
                node_id: node_id.to_string(),
                mode: label.to_string(),
                group_id,
            }
        })
        .collect();

    AgentSnapshot {
        root: RootSnapshot {
            current_window,
            window_count,
            style_preset: None,
        },
        windows,
        sync_nodes,
        frame_time_ms,
        frame_count,
        fps_ema,
    }
}

// ── widget list builder (called separately, less often) ──────────────

/// Build a flat widget list across every attached window for
/// `GET /state/widgets`.  Walks each branch's `LayoutTree` (the
/// retained-mode tree LM owns) and pulls every widget node out.
pub(crate) fn build_widget_list<P: DockPanel>(
    layout: &LayoutManager<P>,
) -> Vec<WidgetSnapshot> {
    use uzor::layout::LayoutNode as TreeNode;
    let mut out = Vec::new();
    let keys: Vec<_> = layout.window_keys().cloned().collect();
    for key in keys {
        let Some(b) = layout.window(&key) else { continue };
        for entry in b.tree.entries() {
            if let TreeNode::Widget(w) = &entry.node {
                out.push(WidgetSnapshot {
                    window: key.as_str().to_owned(),
                    id: w.id.as_str().to_owned(),
                    kind: format!("{:?}", w.kind),
                    rect: RectSnap {
                        x: entry.rect.x,
                        y: entry.rect.y,
                        w: entry.rect.width,
                        h: entry.rect.height,
                    },
                    layer: String::new(),
                });
            }
        }
    }
    out
}
