//! Separator drag state.

use std::collections::HashMap;

use crate::types::WidgetId;

// =============================================================================
// Generic widget-level drag state (used by the uzor separator widget)
// =============================================================================

/// Drag state for a single separator resize handle.
///
/// Maps to mlc `SeparatorDragState` (widget level, §2.5).
///
/// Fields:
/// - `field_id`: which separator widget is being dragged.
/// - `start_pos`: cursor position at drag-start (axis-aligned px).
/// - `start_value`: the separator's logical value (e.g. ratio, px offset) at
///   drag-start — used to compute absolute new value each frame.
/// - `target_pane_id`: opaque ID of the pane/leaf being resized by this drag
///   (mlc: `instance_id: u64` for sub-pane, `sep_idx: usize` for split-panel).
/// - `min_size_constraint`: minimum size (px) the resized pane must maintain.
/// - `max_size_constraint`: maximum size (px) the resized pane may reach
///   (`None` = no upper bound).
#[derive(Debug, Default, Clone)]
pub struct SeparatorDragState {
    pub field_id: Option<WidgetId>,
    pub start_pos: f64,
    pub start_value: f64,
    /// Opaque numeric ID of the pane / leaf currently being resized.
    pub target_pane_id: Option<u64>,
    /// Minimum size (px) enforced during this drag.
    pub min_size_constraint: f64,
    /// Maximum size (px) enforced during this drag (`None` = unbounded).
    pub max_size_constraint: Option<f64>,
}

impl SeparatorDragState {
    pub fn is_active(&self) -> bool {
        self.field_id.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

// =============================================================================
// Multi-separator hover tracking
// =============================================================================

/// Per-separator hover state for screens that render multiple separators
/// simultaneously (e.g. sub-pane list, split-panel grid).
///
/// Keyed by the same `WidgetId` used in `register_separator`.
#[derive(Debug, Default, Clone)]
pub struct SeparatorHoverState {
    hovered: HashMap<WidgetId, bool>,
}

impl SeparatorHoverState {
    pub fn set_hovered(&mut self, id: &WidgetId, hovered: bool) {
        if hovered {
            self.hovered.insert(id.clone(), true);
        } else {
            self.hovered.remove(id);
        }
    }

    pub fn is_hovered(&self, id: &WidgetId) -> bool {
        self.hovered.get(id).copied().unwrap_or(false)
    }

    pub fn clear_all(&mut self) {
        self.hovered.clear();
    }
}

// =============================================================================
// SeparatorController snap-back state
// =============================================================================
//
// NOTE: `uzor::docking::panels::separator::SeparatorController` exists and
// implements snap-back machinery (returns `None` from `update_drag` when a
// min-size constraint is violated). However, chart-app (mylittlechart) does NOT
// call `SeparatorController` for any of its separator drag paths:
//
//   - Sub-pane drag:    `ChartPanelGrid::drag_pane_separator()` — direct ratio update, no snap-back.
//   - Split-panel drag: `ChartPanelGrid::apply_separator_drag()` — cascading proportion update, no snap-back.
//   - Sidebar drag:     inline in `on_drag_move`, no snap-back.
//
// Decision: keep `SeparatorController` in `uzor::docking` where it lives.
// It is NOT used by or re-exported from this widget module. If a future caller
// needs snap-back, use `crate::docking::panels::separator::SeparatorController`
// directly.
