//! Sidebar persistent state.
//!
//! `SidebarState` is flat — fields irrelevant to the active `SidebarRenderKind`
//! are simply never touched.

use std::collections::HashMap;

use crate::types::{Rect, ScrollState};
use super::super::resize_drag::ResizeDrag;

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

    /// Resize drag in progress (set by [`Self::start_resize`]).
    /// Mirrors `ToolbarState::resize_drag` — single source of truth for
    /// the drag math; legacy bool/start fields removed.
    pub resize_drag: Option<ResizeDrag>,

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
            resize_drag: None,
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

    /// Begin a resize drag. Composite's `consume_event` calls this when it
    /// matches `ResizeHandleDragStarted` for its host id.
    pub fn start_resize(
        &mut self,
        edge:       crate::layout::ResizeEdge,
        start_rect: Rect,
        cursor:     (f64, f64),
        min_size:   f64,
        cap_size:   f64,
    ) {
        self.resize_drag = Some(ResizeDrag::begin(
            edge, start_rect, cursor, (min_size, min_size), (cap_size, cap_size),
        ));
    }

    /// Update `width` from a fresh cursor position while a drag is live.
    /// `is_horizontal_axis` — true for Left/Right (width axis), false for
    /// Top/Bottom (sidebar's `width` field stores its height).
    pub fn update_resize(&mut self, cursor: (f64, f64), is_horizontal_axis: bool) {
        if let Some(drag) = self.resize_drag {
            let r = drag.resolve(cursor);
            let raw = if is_horizontal_axis { r.width } else { r.height };
            self.width = raw.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        }
    }

    /// End any active resize drag.
    pub fn end_resize(&mut self) {
        self.resize_drag = None;
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

    // -------------------------------------------------------------------------
    // Scroll helpers (wheel)
    // -------------------------------------------------------------------------

    /// Apply a mouse-wheel delta to the `"default"` scroll panel.
    ///
    /// Clamps the offset to `[0, (content_h − viewport_h).max(0)]`.
    /// The magic `40.0`-px header offset and `30.0`-px per-panel row height
    /// that were previously inlined in app code are captured here.
    ///
    /// `rect`      — sidebar frame rect (height used to derive viewport_h).
    /// `dy`        — raw wheel delta in pixels (positive = down).
    /// `content_h` — total scrollable content height.
    pub fn handle_wheel(&mut self, rect: crate::types::Rect, dy: f64, content_h: f64) {
        const HEADER_H: f64 = 40.0;
        const SCROLL_STEP: f64 = 30.0;
        let viewport_h = (rect.height - HEADER_H).max(0.0);
        let max = (content_h - viewport_h).max(0.0);
        let scroll = self.get_or_insert_scroll("default");
        scroll.offset = (scroll.offset - dy * SCROLL_STEP).clamp(0.0, max);
    }

    /// Compute the scrollbar track rect from the sidebar's frame rect.
    ///
    /// The track occupies the right-hand 8 px strip of the sidebar body
    /// (below the 40 px header).  Returns the track rect in screen space.
    ///
    /// `sidebar_rect` — full sidebar frame rect (including header).
    pub fn scrollbar_track_rect(sidebar_rect: crate::types::Rect) -> crate::types::Rect {
        const HEADER_H:  f64 = 40.0;
        const TRACK_W:   f64 =  8.0;
        let viewport_h = (sidebar_rect.height - HEADER_H).max(0.0);
        crate::types::Rect::new(
            sidebar_rect.x + sidebar_rect.width - TRACK_W,
            sidebar_rect.y + HEADER_H,
            TRACK_W,
            viewport_h,
        )
    }
}
