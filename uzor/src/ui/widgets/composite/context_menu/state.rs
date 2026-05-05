//! ContextMenu persistent state.
//!
//! Unlike Dropdown, ContextMenu positions at raw cursor coordinates with
//! smart screen-edge clamping (`open_smart`). There is no anchor widget.

use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::InputCoordinator;
use crate::layout::LayoutManager;

/// All per-instance state for a context menu.
#[derive(Debug, Clone, Default)]
pub struct ContextMenuState {
    /// Whether the menu is currently visible.
    pub is_open: bool,

    /// Screen-space X of the menu top-left corner (after clamping).
    pub x: f64,

    /// Screen-space Y of the menu top-left corner (after clamping).
    pub y: f64,

    /// Opaque identifier of the object that was right-clicked.
    /// Caller stores semantic meaning; context menu treats it as an opaque tag.
    pub target_id: Option<String>,

    /// Index of the currently hovered item row (`None` = no hover).
    pub hovered_index: Option<usize>,

    /// "Primed" index — tracks the last item that received a pointer-down before
    /// release.  Used to confirm activation only when down + up land on same row.
    pub primed_index: Option<usize>,
}

impl ContextMenuState {
    /// Open the menu, clamping its position to screen bounds.
    ///
    /// `x`, `y`       — raw cursor position in screen coordinates.
    /// `screen_w/h`   — screen dimensions in pixels.
    /// `menu_w/h`     — estimated menu panel dimensions in pixels.
    /// `target_id`    — opaque right-click target (caller semantics).
    pub fn open_smart(
        &mut self,
        x: f64,
        y: f64,
        screen_w: f64,
        screen_h: f64,
        menu_w: f64,
        menu_h: f64,
        target_id: Option<String>,
    ) {
        let clamped_x = if x + menu_w > screen_w {
            (x - menu_w).max(0.0)
        } else {
            x
        };
        let clamped_y = if y + menu_h > screen_h {
            (y - menu_h).max(0.0)
        } else {
            y
        };

        self.is_open = true;
        self.x = clamped_x;
        self.y = clamped_y;
        self.target_id = target_id;
        self.hovered_index = None;
        self.primed_index = None;
    }

    /// Open the menu at raw cursor coordinates without screen-edge clamping.
    pub fn open_raw(&mut self, x: f64, y: f64, target_id: Option<String>) {
        self.is_open = true;
        self.x = x;
        self.y = y;
        self.target_id = target_id;
        self.hovered_index = None;
        self.primed_index = None;
    }

    /// Close the menu and reset all transient state.
    pub fn close(&mut self) {
        self.is_open = false;
        self.hovered_index = None;
        self.primed_index = None;
        // target_id and position are preserved for post-close action dispatch.
    }

    /// Returns `true` when the menu is visible.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Set the hovered item index (called each frame from input routing).
    pub fn set_hovered(&mut self, index: Option<usize>) {
        self.hovered_index = index;
    }

    /// Set the primed item index (pointer-down on a row).
    pub fn set_primed(&mut self, index: Option<usize>) {
        self.primed_index = index;
    }

    /// Sync the hovered-item index from the coordinator's hovered widget.
    ///
    /// **Deprecated** — use `sync_hover_from_layout` when a `LayoutManager`
    /// is available.  Kept for back-compat with L3 callers.
    pub fn sync_hover_from(&mut self, coord: &InputCoordinator, widget_id_prefix: &str) {
        if !self.is_open {
            return;
        }
        let hovered = coord.hovered_widget().map(|id| id.0.clone());
        self.apply_hover(hovered, widget_id_prefix);
    }

    /// Sync the hovered-item index from the `LayoutManager` (L3 authoritative hover).
    ///
    /// Preferred over `sync_hover_from`.
    pub fn sync_hover_from_layout<P: DockPanel>(
        &mut self,
        layout: &LayoutManager<P>,
        widget_id_prefix: &str,
    ) {
        if !self.is_open {
            return;
        }
        let hovered = layout.hovered_widget().map(|id| id.0.clone());
        self.apply_hover(hovered, widget_id_prefix);
    }

    fn apply_hover(&mut self, hovered: Option<String>, widget_id_prefix: &str) {
        self.hovered_index = hovered
            .as_deref()
            .filter(|s| s.starts_with(widget_id_prefix))
            .and_then(|s| s[widget_id_prefix.len()..].parse::<usize>().ok());
    }
}
