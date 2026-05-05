//! Read-only snapshot of LM state — the JSON shape returned by `GET` endpoints.
//!
//! The window manager rebuilds this at the end of every tick (cheap —
//! it's mostly counters and string ids) so HTTP `GET`s never block on
//! the winit thread.

use serde::Serialize;

/// Top-level snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct AgentSnapshot {
    pub root: RootSnapshot,
    pub windows: Vec<BranchSnapshot>,
    /// Synced root node classifications and their current data hint.
    pub sync_nodes: Vec<NodeSyncSnapshot>,
    /// Frame timestamp the snapshot was built at (ms).
    pub frame_time_ms: f64,
    /// Frame counter from the runtime.
    pub frame_count: u64,
    /// EMA of measured fps from the runtime.
    pub fps_ema: f32,
}

/// Snapshot of LM-root (synced) state.
#[derive(Debug, Clone, Serialize)]
pub struct RootSnapshot {
    pub current_window: Option<String>,
    /// Number of attached windows.
    pub window_count: usize,
    /// Active style preset name, if known.
    pub style_preset: Option<String>,
}

/// Snapshot of one `WindowBranch`.
#[derive(Debug, Clone, Serialize)]
pub struct BranchSnapshot {
    pub key: String,
    pub rect: RectSnap,
    pub initialised: bool,

    pub chrome_visible: bool,
    pub edge_count: usize,
    pub dock_leaves: usize,
    pub overlay_count: usize,

    pub modal_count: usize,
    pub popup_count: usize,
    pub dropdown_count: usize,
    pub toolbar_count: usize,
    pub sidebar_count: usize,
    pub context_menu_count: usize,

    pub hovered_widget: Option<String>,
    pub pressed_widget: Option<String>,
    pub last_click: Option<ClickSnap>,
    pub pointer_pos: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RectSnap {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClickSnap {
    pub widget: String,
    pub pos: [f64; 2],
}

/// Sync registry entry projected for the wire.
#[derive(Debug, Clone, Serialize)]
pub struct NodeSyncSnapshot {
    pub node_id: String,
    pub mode: String,        // "synced" / "sometimes·alone" / "sometimes·group" / "standalone"
    pub group_id: Option<u64>,
}

/// One registered widget that an agent might want to click on.
#[derive(Debug, Clone, Serialize)]
pub struct WidgetSnapshot {
    pub window:  String,
    pub id:      String,
    pub kind:    String,
    pub rect:    RectSnap,
    pub layer:   String,
}
