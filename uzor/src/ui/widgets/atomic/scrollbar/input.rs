//! Scrollbar input helpers — ported 1:1 from mlc scroll_dispatch.rs +
//! ScrollState methods.
//!
//! All free functions operate on a mutable `ScrollState` reference; no
//! keyboard PgUp/PgDn handling (mlc routes those to PTY only).

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::types::{Rect, WidgetId};

use super::state::ScrollState;

// ── Hit-zone registration ─────────────────────────────────────────────────────

/// Register the scrollbar track rect for CLICK hit-testing.
pub fn register_track(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ScrollbarTrack, rect, Sense::CLICK, layer);
}

/// Register the scrollbar thumb rect for DRAG hit-testing.
///
/// `inflation_x` — extra pixels added to each horizontal side of the hit zone
/// (mlc uses 5.0 for standard, 10.0 for compact/profile-manager).
pub fn register_thumb(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    thumb_rect: Rect,
    inflation_x: f64,
    layer: &LayerId,
) {
    let inflated = Rect::new(
        thumb_rect.x - inflation_x,
        thumb_rect.y,
        thumb_rect.width + inflation_x * 2.0,
        thumb_rect.height,
    );
    coord.register_atomic(id, WidgetKind::ScrollbarHandle, inflated, Sense::DRAG, layer);
}

// ── Wheel ─────────────────────────────────────────────────────────────────────

/// Apply one mouse-wheel delta to `state`.
///
/// `delta_y` — signed scroll direction **after** caller sign-flip.
/// Positive = scroll down (offset increases).  Per-notch = `|delta_y| * 10 px`.
///
/// Returns `true` if the event was consumed (content overflows viewport).
pub fn handle_scroll_wheel(
    state: &mut ScrollState,
    delta_y: f64,
    content_height: f64,
    viewport_height: f64,
) -> bool {
    state.handle_wheel(delta_y, content_height, viewport_height)
}

// ── Thumb drag ────────────────────────────────────────────────────────────────

/// Begin a thumb drag at absolute screen Y `y`.
pub fn start_thumb_drag(state: &mut ScrollState, y: f64) {
    state.start_drag(y);
}

/// Continue a thumb drag — update offset proportionally.
///
/// `y`            — current absolute screen Y of the cursor.
/// `track_height` — rendered track pixel height.
pub fn update_thumb_drag(
    state: &mut ScrollState,
    y: f64,
    track_height: f64,
    content_height: f64,
    viewport_height: f64,
) {
    state.handle_drag(y, track_height, content_height, viewport_height);
}

/// End the thumb drag — clears drag state, offset is preserved.
pub fn end_thumb_drag(state: &mut ScrollState) {
    state.end_drag();
}

// ── Track click ───────────────────────────────────────────────────────────────

/// Proportional-jump on track click.
///
/// `click_y`      — absolute Y of the click.
/// `track_rect`   — rendered track rect (as returned by `draw_scrollbar`).
///
/// Skips if click lands on the handle — that is a drag start, not a track
/// click.  The caller is responsible for the guard; this fn always jumps.
pub fn handle_track_click(
    state: &mut ScrollState,
    click_y: f64,
    track_rect: Rect,
    content_height: f64,
    viewport_height: f64,
) {
    state.handle_track_click(
        click_y,
        track_rect.y,
        track_rect.height,
        content_height,
        viewport_height,
    );
}

// ── Batch dispatch helpers (mirrors mlc scroll_dispatch.rs) ───────────────────

/// Scrollable context — pairing the last-rendered geometry with the
/// corresponding mutable scroll state.
pub struct ScrollableInfo {
    /// Track rect as returned by `draw_scrollbar`.
    pub track_rect: Option<Rect>,
    /// Thumb rect as returned by `draw_scrollbar`.
    pub handle_rect: Option<Rect>,
    /// Total content height used during last render.
    pub content_height: f64,
    /// Viewport height used during last render.
    pub viewport_height: f64,
}

/// Hit-tolerance for track / handle X axis (mlc standard).
const HIT_TOLERANCE_X: f64 = 5.0;

/// Try to start a thumb drag on whichever entry contains `(x, y)`.
/// Returns `true` and calls `start_drag` on the matching state if found.
pub fn try_start_scrollbar_drag(
    x: f64,
    y: f64,
    entries: &mut [(&ScrollableInfo, &mut ScrollState)],
) -> bool {
    for (info, state) in entries.iter_mut() {
        if let Some(hr) = info.handle_rect {
            let hit = x >= hr.x - HIT_TOLERANCE_X
                && x <= hr.x + hr.width + HIT_TOLERANCE_X
                && y >= hr.y
                && y <= hr.y + hr.height;
            if hit {
                state.start_drag(y);
                return true;
            }
        }
    }
    false
}

/// Continue the active thumb drag for whichever entry has `is_dragging` set.
/// Returns `true` if any state was updated.
pub fn try_handle_scrollbar_drag(
    y: f64,
    entries: &mut [(&ScrollableInfo, &mut ScrollState)],
) -> bool {
    for (info, state) in entries.iter_mut() {
        if state.is_dragging {
            if let Some(tr) = info.track_rect {
                state.handle_drag(y, tr.height, info.content_height, info.viewport_height);
                return true;
            }
        }
    }
    false
}

/// End all active thumb drags across entries.
/// Returns `true` if at least one drag was ended.
pub fn try_end_scrollbar_drag(entries: &mut [(&ScrollableInfo, &mut ScrollState)]) -> bool {
    let mut any = false;
    for (_, state) in entries.iter_mut() {
        if state.is_dragging {
            state.end_drag();
            any = true;
        }
    }
    any
}

/// Route a wheel delta to whichever viewport rect contains `(x, y)`.
/// Returns `true` if an entry handled the event.
pub fn try_handle_wheel(
    x: f64,
    y: f64,
    delta_y: f64,
    entries: &mut [(&ScrollableInfo, &mut ScrollState)],
) -> bool {
    for (info, state) in entries.iter_mut() {
        if let Some(tr) = info.track_rect {
            let in_viewport = x >= tr.x && x <= tr.x + tr.width
                && y >= tr.y && y <= tr.y + tr.height;
            if in_viewport {
                return state.handle_wheel(delta_y, info.content_height, info.viewport_height);
            }
        }
    }
    false
}

/// Route a track click, skipping any entry where the click landed on the handle.
/// Returns `true` if an entry handled the event.
pub fn try_handle_track_click(
    x: f64,
    y: f64,
    entries: &mut [(&ScrollableInfo, &mut ScrollState)],
) -> bool {
    for (info, state) in entries.iter_mut() {
        if let Some(tr) = info.track_rect {
            let hit_track = x >= tr.x - HIT_TOLERANCE_X
                && x <= tr.x + tr.width + HIT_TOLERANCE_X
                && y >= tr.y
                && y <= tr.y + tr.height;
            if !hit_track {
                continue;
            }
            // Skip if click is on the handle
            if let Some(hr) = info.handle_rect {
                let on_handle = x >= hr.x - HIT_TOLERANCE_X
                    && x <= hr.x + hr.width + HIT_TOLERANCE_X
                    && y >= hr.y
                    && y <= hr.y + hr.height;
                if on_handle {
                    continue;
                }
            }
            state.handle_track_click(y, tr.y, tr.height, info.content_height, info.viewport_height);
            return true;
        }
    }
    false
}

// ── Level 1 / Level 2 entry points ───────────────────────────────────────────

/// Level 1 — register a scrollbar (track + thumb) with an explicit `InputCoordinator`.
///
/// `track_id` and `thumb_id` are the stable IDs for the track and thumb rects
/// respectively. `inflation_x` is the horizontal hit-zone inflation for the thumb
/// (use `5.0` for standard, `10.0` for compact variants).
pub fn register_input_coordinator_scrollbar(
    coord: &mut InputCoordinator,
    track_id: impl Into<WidgetId>,
    thumb_id: impl Into<WidgetId>,
    track_rect: Rect,
    thumb_rect: Rect,
    inflation_x: f64,
    layer: &LayerId,
    state: &mut ScrollState,
) {
    let _ = state; // state is read/written by the drag helpers, not registration
    register_track(coord, track_id, track_rect, layer);
    register_thumb(coord, thumb_id, thumb_rect, inflation_x, layer);
}

/// Level 2 — register a scrollbar via `ContextManager`, pulling `ScrollState` from the registry.
///
/// Uses `track_id` as the registry key. `inflation_x` is the horizontal hit-zone
/// inflation for the thumb.
pub fn register_context_manager_scrollbar(
    ctx: &mut ContextManager,
    track_id: impl Into<WidgetId>,
    thumb_id: impl Into<WidgetId>,
    track_rect: Rect,
    thumb_rect: Rect,
    inflation_x: f64,
    layer: &LayerId,
) {
    let track_id: WidgetId = track_id.into();
    let thumb_id: WidgetId = thumb_id.into();
    let state = ctx.registry.get_or_insert_with(track_id.clone(), ScrollState::default);
    register_input_coordinator_scrollbar(
        &mut ctx.input,
        track_id,
        thumb_id,
        track_rect,
        thumb_rect,
        inflation_x,
        layer,
        state,
    );
}
