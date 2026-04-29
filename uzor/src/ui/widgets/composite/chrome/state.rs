//! Chrome persistent state.
//!
//! All fields are stored flat so the caller holds a single `ChromeState`
//! regardless of which `ChromeRenderKind` is active.

use crate::ui::widgets::atomic::tooltip::TooltipState;
use crate::ui::widgets::composite::context_menu::ContextMenuState;

use super::types::{ChromeColors, ChromeHit};

// ---------------------------------------------------------------------------
// TabState
// ---------------------------------------------------------------------------

/// Per-tab transient interaction state.
#[derive(Debug, Clone, Default)]
pub struct TabState {
    /// Stable string id — must match `ChromeTabConfig::id`.
    pub id: String,
    /// Pointer is currently over the tab body.
    pub hovered: bool,
    /// Tab body is currently pressed (pointer-down).
    pub pressed: bool,
    /// Pointer is over the close-X on this tab.
    pub close_hovered: bool,
}

impl TabState {
    /// Create a new `TabState` for the given tab id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            hovered: false,
            pressed: false,
            close_hovered: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ChromeState
// ---------------------------------------------------------------------------

/// All per-chrome-instance persistent state.
#[derive(Debug, Clone, Default)]
pub struct ChromeState {
    // --- Per-frame hit result (set each frame, not persisted across renders) ---

    /// Which zone the pointer is currently over.
    pub hovered: ChromeHit,

    // --- Window state ---

    /// Whether the window is currently maximized.
    pub is_maximized: bool,
    /// Whether a window-drag gesture is in progress (set by parent).
    pub dragging_window: bool,

    // --- Tab state ---

    /// Per-tab hover / press state.  Caller must keep length in sync with
    /// the `tabs` slice passed to `ChromeView`.
    pub tabs_state: Vec<TabState>,
    /// Id of the currently active tab.
    pub active_tab_id: Option<String>,
    /// Pre-computed tab widths (pixels) for the current frame.
    ///
    /// Updated each frame by `update_tab_widths` before registration.
    pub tab_widths: Vec<f64>,

    // --- Overlays ---

    /// Tooltip state for button labels and tab names.
    pub tooltip: TooltipState,
    /// Context-menu state (right-click on tab or button area).
    pub context_menu: ContextMenuState,

    // --- Theme colours ---

    /// Live colour tokens.  Caller may swap these to reflect theme changes.
    pub colors: ChromeColors,

    // --- Legacy field (kept so existing callers compile) ---

    /// Which titlebar button the pointer is over (coarse — use `hovered` for
    /// fine-grained hit data).
    pub chrome_button: Option<super::types::ChromeButton>,

    // --- Accessibility / window manager ---

    /// Window title (not rendered in the strip; passed to the OS title bar).
    pub title: String,
}

impl ChromeState {
    /// Create a new `ChromeState` with default colours.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sync `tabs_state` length to `tab_count`, inserting or removing entries
    /// as needed while preserving existing state for stable tab ids.
    pub fn sync_tabs(&mut self, ids: &[&str]) {
        // Remove entries whose ids no longer appear.
        self.tabs_state.retain(|ts| ids.contains(&ts.id.as_str()));

        // Append new entries for ids that didn't exist.
        for &id in ids {
            if !self.tabs_state.iter().any(|ts| ts.id == id) {
                self.tabs_state.push(TabState::new(id));
            }
        }

        // Reorder to match `ids` order.
        let ordered: Vec<TabState> = ids
            .iter()
            .filter_map(|&id| {
                self.tabs_state.iter().find(|ts| ts.id == id).cloned()
            })
            .collect();
        self.tabs_state = ordered;
    }

    /// Pre-compute tab pixel widths.
    ///
    /// `text_widths` — caller-measured label widths (one per tab).
    /// `tab_padding_h` — horizontal padding per side.
    /// `tab_close_size` — width of the close-X zone.
    pub fn update_tab_widths(
        &mut self,
        text_widths: &[f64],
        tab_padding_h: f64,
        tab_close_size: f64,
    ) {
        self.tab_widths = text_widths
            .iter()
            .map(|&tw| tab_padding_h + tw + tab_close_size + tab_padding_h)
            .collect();
    }

    /// Clear all transient hover/press state (call at end of frame if needed).
    pub fn clear_hover(&mut self) {
        self.hovered = ChromeHit::None;
        self.chrome_button = None;
        for ts in &mut self.tabs_state {
            ts.hovered = false;
            ts.pressed = false;
            ts.close_hovered = false;
        }
    }
}
