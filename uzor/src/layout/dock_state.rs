//! Docking state owned by `LayoutManager`.
//!
//! Step A of the absorption plan — fields previously living on
//! `PanelDockingManager` are gathered here. `LayoutManager` carries one
//! `DockState<P>` instance.  In subsequent steps the manager's methods
//! migrate from `PanelDockingManager` impl to `LayoutManager` impl
//! reading/writing these fields directly.

use std::collections::HashMap;

use crate::docking::panels::{
    CornerHandle, DockPanel, DockingTree, FloatingDragState, FloatingWindow,
    LeafId, PanelDragState, PanelRect, Separator, SnapBackAnimation, TabBarInfo,
    TabReorderState,
};

/// All cross-frame docking state — tree + drags + floatings + per-leaf
/// overrides — plus the per-frame `DockFrame` cache that `solve()` rebuilds.
pub struct DockState<P: DockPanel> {
    // ── Tree (the source of truth) ──────────────────────────────────────────
    pub tree: DockingTree<P>,

    // ── Per-frame layout cache (rebuilt by solve()) ─────────────────────────
    pub frame: DockFrame,

    // ── Window-edge drop indicator rects (set by solve(), read by hit-test) ─
    pub window_edge_rects: Option<[PanelRect; 4]>,

    // ── Drag / interaction state ────────────────────────────────────────────
    pub panel_drag:        Option<PanelDragState>,
    pub tab_reorder:       Option<TabReorderState>,
    pub floating_drag:     Option<FloatingDragState>,
    pub snap_animations:   Vec<SnapBackAnimation>,

    // ── Floating windows (extracted from tree) ──────────────────────────────
    pub floating_windows:  Vec<FloatingWindow<P>>,
    pub next_floating_id:  u64,

    // ── Hover / focus ───────────────────────────────────────────────────────
    pub hovered_header: Option<LeafId>,
    pub active_leaf:    Option<LeafId>,

    // ── Style knobs ─────────────────────────────────────────────────────────
    pub header_height:  f32,

    // ── Per-leaf min-size overrides (fallback to panel.min_size()) ──────────
    pub leaf_min_sizes: HashMap<LeafId, (f32, f32)>,
}

impl<P: DockPanel> DockState<P> {
    pub fn new() -> Self {
        Self {
            tree:              DockingTree::new(),
            frame:             DockFrame::default(),
            window_edge_rects: None,
            panel_drag:        None,
            tab_reorder:       None,
            floating_drag:     None,
            snap_animations:   Vec::new(),
            floating_windows:  Vec::new(),
            next_floating_id:  1,
            hovered_header:    None,
            active_leaf:       None,
            header_height:     24.0,
            leaf_min_sizes:    HashMap::new(),
        }
    }

    pub fn from_tree(tree: DockingTree<P>) -> Self {
        let active_leaf = tree.active_leaf_id();
        Self {
            tree,
            frame:             DockFrame::default(),
            window_edge_rects: None,
            panel_drag:        None,
            tab_reorder:       None,
            floating_drag:     None,
            snap_animations:   Vec::new(),
            floating_windows:  Vec::new(),
            next_floating_id:  1,
            hovered_header:    None,
            active_leaf,
            header_height:     24.0,
            leaf_min_sizes:    HashMap::new(),
        }
    }
}

impl<P: DockPanel> Default for DockState<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-frame computed docking layout — fully rebuilt by every `solve()`.
#[derive(Default)]
pub struct DockFrame {
    pub layout_area:   PanelRect,
    pub panel_rects:   HashMap<LeafId, PanelRect>,
    pub panel_headers: HashMap<LeafId, PanelRect>,
    pub separators:    Vec<Separator>,
    pub tab_bars:      Vec<TabBarInfo>,
    pub corners:       Vec<CornerHandle>,
}
