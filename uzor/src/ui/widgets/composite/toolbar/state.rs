//! Toolbar persistent state.

use crate::input::core::coordinator::InputCoordinator;
use crate::types::Rect;

use super::super::resize_drag::ResizeDrag;
use super::types::SplitButtonHoverZone;

/// All per-toolbar instance state.
///
/// Fields are stored flat — fields irrelevant to the active `ToolbarRenderKind`
/// are never touched.
#[derive(Debug, Default, Clone)]
pub struct ToolbarState {
    // --- Item interaction ---

    /// Id of the item that was last clicked / activated (persists across frames).
    pub active_item_id: Option<String>,

    /// Id of the item the pointer is currently hovering over.
    pub hovered_item_id: Option<String>,

    /// Id of the item currently held down (pointer pressed, not yet released).
    pub pressed_item_id: Option<String>,

    // --- Dropdown ---

    /// Id of the item whose dropdown is currently open.
    /// `None` = no dropdown open.
    pub dropdown_open: Option<String>,

    // --- Scroll (Horizontal overflow) ---

    /// Horizontal (or vertical for Vertical toolbar) scroll offset in pixels.
    /// `0.0` = no scroll.
    pub scroll_offset: f64,

    // --- Tooltip ---

    /// Screen-space anchor rect of the item that should show a tooltip.
    /// `None` = no tooltip pending.
    pub tooltip_anchor: Option<Rect>,

    /// Id of the item that is the current tooltip target.
    pub tooltip_item_id: Option<String>,

    // --- Split button ---

    /// Split button currently being interacted with: `(item_id, zone)`.
    /// `None` = no split interaction.
    pub dragging_split: Option<(String, SplitButtonHoverZone)>,

    // --- ChromeStrip ---

    /// Index of the currently active (selected) chrome tab.
    pub active_chrome_tab: usize,

    /// Index of the hovered chrome tab (`None` = none).
    pub hovered_chrome_tab: Option<usize>,

    /// Whether the window-control close button is hovered.
    pub chrome_close_hovered: bool,

    /// Whether the window-control minimize button is hovered.
    pub chrome_minimize_hovered: bool,

    /// Whether the window-control maximize button is hovered.
    pub chrome_maximize_hovered: bool,

    // --- Resize ---

    /// Resize drag in progress (set by `start_resize`).
    pub resize_drag: Option<ResizeDrag>,

    /// User-resized override for thickness (height for horizontal /
    /// width for vertical toolbars). `0.0` = use the measured value.
    pub resized_thickness: f64,
}

impl ToolbarState {
    /// Mark an item as active (e.g. after click).
    pub fn activate(&mut self, id: impl Into<String>) {
        self.active_item_id = Some(id.into());
    }

    /// Clear the active item selection.
    pub fn clear_active(&mut self) {
        self.active_item_id = None;
    }

    /// Open the dropdown for `id`, closing any previously open one.
    pub fn open_dropdown(&mut self, id: impl Into<String>) {
        self.dropdown_open = Some(id.into());
    }

    /// Close the open dropdown.
    pub fn close_dropdown(&mut self) {
        self.dropdown_open = None;
    }

    /// Returns `true` when a dropdown is open for the given item id.
    pub fn is_dropdown_open(&self, id: &str) -> bool {
        self.dropdown_open.as_deref() == Some(id)
    }

    /// Apply a horizontal scroll delta (positive = scroll right).
    ///
    /// `min` / `max` define the valid scroll range in pixels.
    pub fn scroll(&mut self, delta: f64, min: f64, max: f64) {
        self.scroll_offset = (self.scroll_offset + delta).clamp(min, max);
    }

    /// Reset transient hover/press state — call at frame start.
    pub fn clear_transient(&mut self) {
        self.hovered_item_id = None;
        self.pressed_item_id = None;
        self.tooltip_anchor = None;
        self.tooltip_item_id = None;
        self.dragging_split = None;
    }

    /// Set the tooltip target for this frame.
    pub fn set_tooltip(&mut self, id: impl Into<String>, anchor: Rect) {
        self.tooltip_item_id = Some(id.into());
        self.tooltip_anchor = Some(anchor);
    }

    /// Start a split-button interaction.
    pub fn begin_split_drag(&mut self, id: impl Into<String>, zone: SplitButtonHoverZone) {
        self.dragging_split = Some((id.into(), zone));
    }

    /// End a split-button interaction.
    pub fn end_split_drag(&mut self) {
        self.dragging_split = None;
    }

    /// Sync the hovered-item id from the coordinator's hovered widget.
    ///
    /// `widget_id_prefix` — the `"{toolbar_widget_id}:"` prefix used at
    /// registration time. When the coord's hovered widget id starts with
    /// this prefix, the suffix becomes the new `hovered_item_id`. Otherwise
    /// `hovered_item_id` is cleared.
    ///
    /// Composite registration helpers call this automatically — apps don't
    /// need to forward `coord.hovered_widget()` by hand.
    pub fn sync_hover_from(&mut self, coord: &InputCoordinator, widget_id_prefix: &str) {
        self.hovered_item_id = coord
            .hovered_widget()
            .map(|id| id.0.as_str())
            .filter(|s| s.starts_with(widget_id_prefix))
            .map(|s| s[widget_id_prefix.len()..].to_owned());
    }

    /// Begin a thickness-resize drag.
    ///
    /// Toolbars only resize on a single axis (height for horizontal,
    /// width for vertical), so the dummy zero dimension on the inactive
    /// axis is sized to a large value to keep the drag math simple.
    pub fn start_resize(
        &mut self,
        edge:        crate::layout::ResizeEdge,
        start_rect:  Rect,
        cursor:      (f64, f64),
        min_size:    f64,
        cap_size:    f64,
    ) {
        self.resize_drag = Some(ResizeDrag::begin(
            edge,
            start_rect,
            cursor,
            (min_size, min_size),
            (cap_size, cap_size),
        ));
    }

    /// Update the resized thickness from a fresh cursor position.
    pub fn update_resize(&mut self, cursor: (f64, f64), is_vertical: bool) {
        if let Some(drag) = self.resize_drag {
            let rect = drag.resolve(cursor);
            self.resized_thickness = if is_vertical { rect.width } else { rect.height };
        }
    }

    /// End any active resize drag.
    pub fn end_resize(&mut self) {
        self.resize_drag = None;
    }
}
