//! Read-only snapshot of LM state — wire format for `GET` endpoints.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AgentSnapshot {
    pub root: RootSnapshot,
    pub windows: Vec<BranchSnapshot>,
    pub sync_nodes: Vec<NodeSyncSnapshot>,
    pub frame_time_ms: f64,
    pub frame_count: u64,
    pub fps_ema: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RootSnapshot {
    pub current_window: Option<String>,
    pub window_count: usize,
    pub style_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchSnapshot {
    pub key: String,
    pub rect: RectSnap,
    pub initialised: bool,
    pub tick_count: u64,
    /// Resolved baseline `TickRate` as a short label
    /// (`"dirty"`, `"60"`, `"uncapped"`).
    pub tick_rate: String,

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

#[derive(Debug, Clone, Serialize)]
pub struct NodeSyncSnapshot {
    pub node_id: String,
    pub mode: String,
    pub group_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetSnapshot {
    pub window: String,
    pub id:     String,
    pub kind:   String,
    pub rect:   RectSnap,
    /// Layer name resolved through `LM::compute_layer_for`.  Empty for
    /// widgets whose layer cannot be determined from the tree alone.
    pub layer:  String,
    /// Human-readable text the L3 builder attached (button text, label
    /// content).  Lets agents address widgets by visible label
    /// instead of having to know their stable id.
    pub label:  Option<String>,
}
