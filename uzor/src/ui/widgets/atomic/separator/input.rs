//! Separator input registration and drag helpers.
//!
//! Provides:
//! - `register_separator`  — register hit zone with the input coordinator
//! - `start_separator_drag`  — capture drag-start state with constraints
//! - `update_separator_drag` — compute clamped new value from current cursor
//! - `end_separator_drag`    — finalize and clear
//!
//! Double-click reset is NOT implemented: mlc has no double-click reset for
//! any separator variant (§3, §6 — no modifier / special gesture handlers found).

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::SeparatorDragState;
use super::types::SeparatorType;

// =============================================================================
// Separator kind for hit-zone sizing
// =============================================================================

/// Which mlc separator variant is being registered.
///
/// Determines the sense and hit-zone semantics passed to the coordinator.
/// Callers expand the `rect` to the appropriate hit zone before calling
/// `register_separator` — this enum tells the coordinator what interaction
/// to expect.
pub enum SeparatorKind {
    /// Visual-only divider: `Sense::NONE`.  No hit zone needed; `rect` may be
    /// the exact 1 px visual rect.
    Divider,
    /// Draggable resize handle: `Sense::DRAG`.  `rect` should be the expanded
    /// hit zone (e.g. 12 px for sub-pane, 8 px for split-panel/sidebar).
    ResizeHandle,
}

// =============================================================================
// register_separator
// =============================================================================

/// Register a separator widget with the input coordinator.
///
/// `kind` controls whether the registered sense is `NONE` (Divider) or
/// `DRAG` (ResizeHandle).  The caller is responsible for expanding `rect` to
/// the correct hit-zone width/height before this call.
///
/// Equivalent to the existing `register` function but accepts a `SeparatorKind`
/// directly so callers do not need to construct a full `SeparatorType`.
pub fn register_separator(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: SeparatorKind,
    layer: &LayerId,
) {
    let sense = match kind {
        SeparatorKind::Divider => Sense::NONE,
        SeparatorKind::ResizeHandle => Sense::DRAG,
    };
    coord.register_atomic(id, WidgetKind::Separator, rect, sense, layer);
}

/// Convenience wrapper over the original `SeparatorType`-based registration.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: &SeparatorType,
    layer: &LayerId,
) {
    let sense = if kind.is_interactive() {
        Sense::DRAG
    } else {
        Sense::NONE
    };
    coord.register_atomic(id, WidgetKind::Separator, rect, sense, layer);
}

// =============================================================================
// start_separator_drag
// =============================================================================

/// Capture drag-start state for a separator resize handle.
///
/// `widget_id`         — widget that was clicked / drag-started.
/// `cursor_pos`        — cursor position along the separator axis (px).
/// `current_value`     — current logical value of the separator
///                       (e.g. `height_ratio * available_h` for sub-pane,
///                        or absolute px offset for sidebar).
/// `target_pane_id`    — opaque ID of the pane/leaf being resized.
/// `min_size`          — minimum size (px) the resized pane must maintain.
/// `max_size`          — maximum size (px) the resized pane may reach
///                       (`None` = unbounded, caller computes from context).
pub fn start_separator_drag(
    state: &mut SeparatorDragState,
    widget_id: WidgetId,
    cursor_pos: f64,
    current_value: f64,
    target_pane_id: Option<u64>,
    min_size: f64,
    max_size: Option<f64>,
) {
    state.field_id = Some(widget_id);
    state.start_pos = cursor_pos;
    state.start_value = current_value;
    state.target_pane_id = target_pane_id;
    state.min_size_constraint = min_size;
    state.max_size_constraint = max_size;
}

// =============================================================================
// update_separator_drag
// =============================================================================

/// Compute clamped new separator value from the current cursor position.
///
/// Returns `None` when no drag is active (`state.field_id` is `None`).
/// Returns `Some(clamped_value)` otherwise.
///
/// Formula (matches mlc absolute-position model for sidebar / watchlist):
/// ```text
/// new_value = start_value + (cursor_pos - start_pos)
/// new_value = clamp(new_value, min_size_constraint, max_size_constraint)
/// ```
///
/// For delta-based paths (sub-pane, split-panel) the caller computes the
/// delta separately and does not need this function — it calls the crate-
/// specific drag handler directly.
pub fn update_separator_drag(state: &SeparatorDragState, cursor_pos: f64) -> Option<f64> {
    state.field_id.as_ref()?;

    let new_value = state.start_value + (cursor_pos - state.start_pos);
    let clamped = new_value.max(state.min_size_constraint);
    let clamped = match state.max_size_constraint {
        Some(max) => clamped.min(max),
        None => clamped,
    };
    Some(clamped)
}

// =============================================================================
// end_separator_drag
// =============================================================================

/// Finalize a separator drag and clear state.
///
/// Returns `Some((widget_id, final_value))` computed from the last cursor
/// position, or `None` if no drag was active.
pub fn end_separator_drag(
    state: &mut SeparatorDragState,
    cursor_pos: f64,
) -> Option<(WidgetId, f64)> {
    let id = state.field_id.clone()?;
    let value = update_separator_drag(state, cursor_pos).unwrap_or(state.start_value);
    state.clear();
    Some((id, value))
}

// ── Level 1 / Level 2 entry points ───────────────────────────────────────────

/// Level 1 — register a separator with an explicit `InputCoordinator`.
///
/// `kind` determines whether it is a visual divider (`Sense::NONE`) or a
/// draggable resize handle (`Sense::DRAG`).
pub fn register_input_coordinator_separator(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: SeparatorKind,
    layer: &LayerId,
    state: &mut SeparatorDragState,
) {
    let _ = state; // drag state is managed by start/update/end helpers
    register_separator(coord, id, rect, kind, layer);
}

/// Level 2 — register a separator via `ContextManager`, pulling `SeparatorDragState`
/// from the registry.
pub fn register_context_manager_separator(
    ctx: &mut ContextManager,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: SeparatorKind,
    layer: &LayerId,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), SeparatorDragState::default);
    register_input_coordinator_separator(&mut ctx.input, id, rect, kind, layer, state);
}
