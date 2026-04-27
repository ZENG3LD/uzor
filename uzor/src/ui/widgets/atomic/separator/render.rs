//! Separator rendering — one function per visual variant.
//!
//! Each function maps to a distinct mlc render pattern identified in the
//! forensic audit (§1).  All filled separators use `fill_rect`; the modal
//! section divider uses `stroke`.
//!
//! # Function map
//!
//! | Function                    | mlc variant        | Method        |
//! |-----------------------------|--------------------|---------------|
//! | `draw_separator`            | generic (existing) | fill_rect     |
//! | `draw_separator_line`       | §1.1 sub-pane      | fill_rect     |
//! | `draw_pane_resize_handle`   | §1.1 + sub-pane    | fill_rect     |
//! | `draw_split_panel_handle`   | §1.2 split-panel   | fill_rect     |
//! | `draw_sidebar_handle`       | §1.3 sidebar       | stroke        |
//! | `draw_modal_section_divider`| §1.9 modal         | stroke        |

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::SeparatorSettings;
use super::style::{SPLIT_PANEL_THICKNESS_HOVER_DRAG, SPLIT_PANEL_THICKNESS_IDLE};
use super::types::{SeparatorOrientation, SeparatorType};

// =============================================================================
// Shared view type
// =============================================================================

pub struct SeparatorView {
    pub kind: SeparatorType,
    /// Hovered (resize-handle highlighting).
    pub hovered: bool,
    pub dragging: bool,
}

// =============================================================================
// 1. Generic draw_separator (existing, unchanged behaviour)
// =============================================================================

/// Generic separator / resize handle.
///
/// Used for panels, toolbars, and any other UI area that needs a configurable
/// visual divider via `SeparatorSettings`.
pub fn draw_separator(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &SeparatorView,
    settings: &SeparatorSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let color = match (&view.kind, view.dragging, view.hovered) {
        (SeparatorType::ResizeHandle { .. }, true, _) => theme.handle_active(),
        (SeparatorType::ResizeHandle { .. }, false, true) => theme.handle_hover(),
        _ => theme.line(),
    };

    let t = style.thickness();
    let m = style.margin();

    let line_rect = match view.kind.orientation() {
        SeparatorOrientation::Horizontal => Rect::new(
            rect.x + m,
            rect.y + (rect.height - t) / 2.0,
            rect.width - m * 2.0,
            t,
        ),
        SeparatorOrientation::Vertical => Rect::new(
            rect.x + (rect.width - t) / 2.0,
            rect.y + m,
            t,
            rect.height - m * 2.0,
        ),
    };

    ctx.set_fill_color(color);
    ctx.fill_rect(line_rect.x, line_rect.y, line_rect.width, line_rect.height);
}

// =============================================================================
// 2. draw_separator_line — simple 1 px visual divider (fill_rect)
// =============================================================================

/// Simple non-interactive visual divider line (1 px, fill_rect).
///
/// Matches mlc sub-pane separator visual render (§1.1):
/// ```text
/// ctx.fill_rect(separator.x, separator.y, separator.width, separator.height);
/// ```
///
/// `thickness` — line thickness in pixels (pass 1.0 for sub-pane).
/// `color`     — hex colour string (e.g. `"#363a45"`).
pub fn draw_separator_line(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
) {
    ctx.set_fill_color(color);
    ctx.fill_rect(x, y, width, height);
}

// =============================================================================
// 3. draw_pane_resize_handle — sub-pane handle (1 px visual, expanded hit area)
// =============================================================================

/// Sub-pane separator resize handle.
///
/// Visual: 1 px horizontal filled line centered in `rect`.
/// Hit zone: the full `rect` height (caller must pass the expanded rect,
/// typically `rect.height = SUB_PANE_HIT_TOLERANCE * 2.0 = 12 px`).
///
/// No hover highlight on the visual line — only cursor changes in mlc.
/// Color: `theme.pane_handle_idle()`.
pub fn draw_pane_resize_handle(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    settings: &SeparatorSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let t = style.thickness(); // should be 1.0
    let line_y = rect.y + (rect.height - t) / 2.0;

    ctx.set_fill_color(theme.pane_handle_idle());
    ctx.fill_rect(rect.x, line_y, rect.width, t);
}

// =============================================================================
// 4. draw_split_panel_handle — split-panel (2 px idle / 4 px hover-drag)
// =============================================================================

/// Split-panel separator handle (between chart sub-windows in ChartPanelGrid).
///
/// Visual thickness: 2 px idle, 4 px hover/drag.
/// Color: idle = `theme.pane_handle_idle()`, hover/drag = `theme.pane_handle_hover()`.
/// Rendering is always `fill_rect`, centered on `position` (§1.2).
///
/// `position`    — separator center coordinate along its axis (px from container start).
/// `start`       — perpendicular axis start (px).
/// `length`      — perpendicular axis extent (px).
/// `orientation` — Vertical (|) or Horizontal (—).
/// `hovered`     — true when cursor is within the 8 px hit zone.
/// `dragging`    — true during active drag.
pub fn draw_split_panel_handle(
    ctx: &mut dyn RenderContext,
    position: f64,
    start: f64,
    length: f64,
    orientation: SeparatorOrientation,
    hovered: bool,
    dragging: bool,
    settings: &SeparatorSettings,
) {
    let theme = settings.theme.as_ref();

    let thickness = if hovered || dragging {
        SPLIT_PANEL_THICKNESS_HOVER_DRAG
    } else {
        SPLIT_PANEL_THICKNESS_IDLE
    };

    let color = if hovered || dragging {
        theme.pane_handle_hover()
    } else {
        theme.pane_handle_idle()
    };

    let (rx, ry, rw, rh) = match orientation {
        SeparatorOrientation::Vertical => {
            let x = position - thickness / 2.0;
            (x, start, thickness, length)
        }
        SeparatorOrientation::Horizontal => {
            let y = position - thickness / 2.0;
            (start, y, length, thickness)
        }
    };

    ctx.set_fill_color(color);
    ctx.fill_rect(rx, ry, rw, rh);
}

// =============================================================================
// 5. draw_sidebar_handle — sidebar separator (1 px stroke)
// =============================================================================

/// Sidebar separator between chart area and right sidebar (§1.3).
///
/// Visual: 1 px vertical stroke at `x` spanning `y .. y + height`.
/// No visual change on hover/drag — only cursor changes.
/// Color: `theme.sidebar_separator()`.
pub fn draw_sidebar_handle(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    height: f64,
    settings: &SeparatorSettings,
) {
    let theme = settings.theme.as_ref();

    ctx.set_stroke_color(theme.sidebar_separator());
    ctx.set_stroke_width(1.0);
    ctx.begin_path();
    ctx.move_to(x, y);
    ctx.line_to(x, y + height);
    ctx.stroke();
}

// =============================================================================
// 6. draw_modal_section_divider — stroked 1 px modal divider
// =============================================================================

/// Modal section divider (header/footer lines inside modals) (§1.9).
///
/// Visual: 1 px horizontal stroke from `(x, y)` to `(x + width, y)`.
/// Non-interactive.
/// Color: `theme.modal_divider()`.
pub fn draw_modal_section_divider(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    settings: &SeparatorSettings,
) {
    let theme = settings.theme.as_ref();

    ctx.set_stroke_color(theme.modal_divider());
    ctx.set_stroke_width(1.0);
    ctx.begin_path();
    ctx.move_to(x, y);
    ctx.line_to(x + width, y);
    ctx.stroke();
}
