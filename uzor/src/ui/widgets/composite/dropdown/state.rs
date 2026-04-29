//! Dropdown persistent state.
//!
//! `DropdownState` is a flat struct — fields irrelevant to the active
//! `DropdownRenderKind` are never touched.

use crate::types::Rect;

/// All per-dropdown instance state.
#[derive(Debug, Clone)]
pub struct DropdownState {
    // --- Lifecycle ---

    /// Whether the dropdown panel is currently visible.
    pub open: bool,

    // --- Position ---

    /// Top-left of the open panel in screen coordinates.
    /// Re-computed each frame from `anchor_rect` + orientation.
    pub origin: (f64, f64),

    /// Trigger button rect — used to re-anchor when layout reflows.
    /// `None` = caller provides position via `open_position_override`.
    pub anchor_rect: Option<Rect>,

    // --- Selection ---

    /// Id of the last-selected item (persistent across open/close cycles).
    /// Used for accent bar in presets menu; optional for other menus.
    pub selected_id: Option<String>,

    // --- Hover ---

    /// Id of the currently hovered item within the open list.
    pub hovered_id: Option<String>,

    // --- Scroll ---

    /// Vertical scroll offset in pixels for long item lists.
    /// `0.0` = top of list fully visible.
    pub scroll_offset: f64,

    // --- Submenu ---

    /// Id of the item whose submenu is currently open.
    /// `None` = no submenu open.
    pub submenu_open: Option<String>,

    /// Screen-space top-left position for the open submenu panel.
    /// Computed as `(parent_menu.right() + gap, trigger_item.y)`.
    pub submenu_origin: (f64, f64),

    // --- Sizing constraints ---

    /// Maximum height of the panel in pixels.
    /// `0.0` = no height limit (scroll disabled).
    pub max_height: f64,

    /// Minimum width of the panel in pixels.
    /// Defaults to `180.0`; inline variant uses button width instead.
    pub min_width: f64,

    // --- Primed state ---

    /// "Primed" item id: last quick-selected tool shown with accent on the trigger.
    /// Specific to toolbar drawing-tool groups; `None` for all other kinds.
    pub primed_id: Option<String>,

    // --- Custom position override ---

    /// When `Some`, overrides anchor-based positioning.
    /// Used by external callers (chrome button, context trigger) that need to
    /// open the dropdown at an arbitrary screen coordinate.
    /// Cleared when the dropdown closes.
    pub open_position_override: Option<(f64, f64)>,
}

impl Default for DropdownState {
    fn default() -> Self {
        Self {
            open: false,
            origin: (0.0, 0.0),
            anchor_rect: None,
            selected_id: None,
            hovered_id: None,
            scroll_offset: 0.0,
            submenu_open: None,
            submenu_origin: (0.0, 0.0),
            max_height: 0.0,
            min_width: 180.0,
            primed_id: None,
            open_position_override: None,
        }
    }
}

impl DropdownState {
    /// Open the dropdown, computing origin from an anchor rect.
    ///
    /// `anchor` — trigger button rect in screen coordinates.
    /// `gap` — pixels between the bottom of the trigger and the panel top.
    pub fn open_below(&mut self, anchor: Rect, gap: f64) {
        self.open = true;
        self.anchor_rect = Some(anchor);
        self.origin = (anchor.x, anchor.y + anchor.height + gap);
        self.open_position_override = None;
        self.hovered_id = None;
        self.submenu_open = None;
        self.scroll_offset = 0.0;
    }

    /// Open the dropdown at an explicit screen-space position.
    pub fn open_at(&mut self, x: f64, y: f64) {
        self.open = true;
        self.anchor_rect = None;
        self.origin = (x, y);
        self.open_position_override = Some((x, y));
        self.hovered_id = None;
        self.submenu_open = None;
        self.scroll_offset = 0.0;
    }

    /// Close the dropdown and reset transient state.
    pub fn close(&mut self) {
        self.open = false;
        self.hovered_id = None;
        self.submenu_open = None;
        self.open_position_override = None;
        self.scroll_offset = 0.0;
    }

    /// Select an item by id (persists across open/close cycles).
    pub fn select(&mut self, id: impl Into<String>) {
        self.selected_id = Some(id.into());
    }

    /// Returns the effective panel origin: override > anchor-derived.
    pub fn effective_origin(&self) -> (f64, f64) {
        self.open_position_override.unwrap_or(self.origin)
    }
}
