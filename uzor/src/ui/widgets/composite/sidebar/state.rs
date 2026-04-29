//! Sidebar persistent state.
//!
//! `SidebarState` is flat — fields irrelevant to the active `SidebarRenderKind`
//! are simply never touched.

use std::collections::HashMap;

use crate::types::ScrollState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default sidebar width in pixels (matches mlc `RIGHT_SIDEBAR_WIDTH`).
pub const DEFAULT_SIDEBAR_WIDTH: f64 = 340.0;
/// Minimum allowed sidebar width in pixels (matches mlc `MIN_SIDEBAR_WIDTH`).
pub const MIN_SIDEBAR_WIDTH: f64 = 280.0;
/// Maximum allowed sidebar width in pixels (matches mlc `MAX_SIDEBAR_WIDTH`).
pub const MAX_SIDEBAR_WIDTH: f64 = 4000.0;

// ---------------------------------------------------------------------------
// SidebarState
// ---------------------------------------------------------------------------

/// All per-sidebar-instance persistent state.
#[derive(Debug, Clone)]
pub struct SidebarState {
    // --- Collapse ---

    /// Whether the sidebar is in its collapsed (hidden) state.
    pub is_collapsed: bool,

    // --- Width ---

    /// Current sidebar width in pixels.  Clamped to `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
    pub width: f64,

    // --- Active tab (WithTypeSelector) ---

    /// Id of the currently active tab.  `None` = no tab selected.
    pub active_tab: Option<String>,

    // --- Per-panel scroll ---

    /// Per-panel scroll state keyed by panel id string.
    ///
    /// Each panel keeps its own scroll offset; switching panels does not reset scroll.
    pub scroll_per_panel: HashMap<String, ScrollState>,

    // --- Resize drag ---

    /// Whether a resize drag gesture is currently in progress.
    pub resize_dragging: bool,

    /// Screen X at the start of the current resize drag.
    pub resize_drag_start_x: f64,

    /// Sidebar width recorded at drag start (used to compute new width).
    pub resize_drag_start_width: f64,

    // --- Header action hover ---

    /// Id of the header action button the pointer is currently hovering over.
    pub header_action_hovered: Option<String>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            is_collapsed: false,
            width: DEFAULT_SIDEBAR_WIDTH,
            active_tab: None,
            scroll_per_panel: HashMap::new(),
            resize_dragging: false,
            resize_drag_start_x: 0.0,
            resize_drag_start_width: DEFAULT_SIDEBAR_WIDTH,
            header_action_hovered: None,
        }
    }
}

impl SidebarState {
    // -------------------------------------------------------------------------
    // Scroll helpers
    // -------------------------------------------------------------------------

    /// Return a mutable reference to the `ScrollState` for `panel_id`,
    /// inserting a default entry if one does not exist yet.
    pub fn get_or_insert_scroll(&mut self, panel_id: &str) -> &mut ScrollState {
        self.scroll_per_panel
            .entry(panel_id.to_owned())
            .or_default()
    }

    // -------------------------------------------------------------------------
    // Resize helpers
    // -------------------------------------------------------------------------

    /// Begin a resize drag at screen position `x`.
    pub fn start_resize_drag(&mut self, x: f64) {
        self.resize_dragging = true;
        self.resize_drag_start_x = x;
        self.resize_drag_start_width = self.width;
    }

    /// Update width while dragging.
    ///
    /// For a `Right` sidebar the resize edge is on the left:
    /// moving left (`x` decreasing) increases width.
    /// `delta_sign`: pass `+1.0` for Right sidebar (left edge), `-1.0` for Left sidebar (right edge).
    pub fn update_resize_drag(&mut self, x: f64, delta_sign: f64) {
        if !self.resize_dragging {
            return;
        }
        let delta = (self.resize_drag_start_x - x) * delta_sign;
        self.width = (self.resize_drag_start_width + delta)
            .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
    }

    /// End the current resize drag.
    pub fn end_resize_drag(&mut self) {
        self.resize_dragging = false;
    }

    // -------------------------------------------------------------------------
    // Collapse / expand
    // -------------------------------------------------------------------------

    /// Toggle between collapsed and expanded states.
    pub fn toggle_collapse(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    // -------------------------------------------------------------------------
    // Tab switch
    // -------------------------------------------------------------------------

    /// Switch to the given tab.  Does NOT reset scroll for the new panel.
    pub fn set_active_tab(&mut self, id: impl Into<String>) {
        self.active_tab = Some(id.into());
    }
}
