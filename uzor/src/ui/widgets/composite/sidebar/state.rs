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
///
/// Used only as a hard floor for `Left` / `Right` sidebars. `Top` / `Bottom`
/// kinds use [`MIN_SIDEBAR_HEIGHT`] instead.
pub const MIN_SIDEBAR_WIDTH: f64 = 280.0;
/// Minimum allowed sidebar height in pixels (for `Top` / `Bottom` kinds).
pub const MIN_SIDEBAR_HEIGHT: f64 = 60.0;
/// Default opening size as a fraction of the viewport's relevant axis.
/// Left/Right → fraction of viewport width; Top/Bottom → fraction of viewport height.
pub const DEFAULT_SIDEBAR_VIEWPORT_FRAC: f64 = 0.20;
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
            // Sentinel — `0.0` means "not yet sized". The composite (or
            // [`SidebarState::ensure_sized`]) fills this in from the current
            // viewport on first render so the sidebar opens at a sensible
            // fraction (default: 20%) instead of a hardcoded pixel value.
            width: 0.0,
            active_tab: None,
            scroll_per_panel: HashMap::new(),
            resize_dragging: false,
            resize_drag_start_x: 0.0,
            resize_drag_start_width: 0.0,
            header_action_hovered: None,
        }
    }
}

impl SidebarState {
    // -------------------------------------------------------------------------
    // Default sizing
    // -------------------------------------------------------------------------

    /// First-render initialiser: if `self.width` hasn't been set yet (sentinel
    /// `0.0` from `Default::default`), fill it from the viewport at
    /// `DEFAULT_SIDEBAR_VIEWPORT_FRAC` of the relevant axis.
    ///
    /// `is_horizontal_kind` — `true` for `Left` / `Right` / `WithTypeSelector`
    /// (size = fraction of viewport WIDTH), `false` for `Top` / `Bottom`
    /// (size = fraction of viewport HEIGHT).
    ///
    /// Composite `register_layout_manager_sidebar` calls this automatically.
    /// Once set, future calls are no-ops — the user's resize stays sticky.
    pub fn ensure_sized(&mut self, viewport_w: f64, viewport_h: f64, is_horizontal_kind: bool) {
        if self.width > 0.0 {
            return;
        }
        let axis = if is_horizontal_kind { viewport_w } else { viewport_h };
        let min  = if is_horizontal_kind { MIN_SIDEBAR_WIDTH } else { MIN_SIDEBAR_HEIGHT };
        self.width = (axis * DEFAULT_SIDEBAR_VIEWPORT_FRAC).max(min);
        self.resize_drag_start_width = self.width;
    }

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
