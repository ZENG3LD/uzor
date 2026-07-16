//! 1D X-brush — a horizontal drag-to-select interval over a figure's plot
//! area. State-machine shape ported from mlc's `ChartInputState` +
//! `DefaultChartInputHandler` drag bookkeeping
//! (`engine/input/handler/{traits,default}.rs`), generalized to exactly
//! one gesture (X-only range select) instead of chart pan/zoom/kinetic-
//! scroll/primitive-drag/pane-resize — V2 scope is crosshair + one brush,
//! nothing else needs drag state yet. Kept as its own small
//! semantic-action-driven struct (not `uzor`'s `Sense`/`WidgetResponse`
//! widget-drag machinery, design law #4's other named building block):
//! that machinery is for widget-tree drag targets with many instances
//! (`uzor-graph`'s per-node `DragController`); a single-instance
//! canvas-level gesture is exactly what mlc's own harvested reference
//! (`ChartInputState`) already is — a plain struct fed by semantic drag
//! actions, same precedent `uzor-graph`'s own `GraphEngine::on_event`
//! camera-pan bookkeeping follows (`PointerMode::PanningCamera { last,
//! total }`, `uzor-graph/src/engine.rs:306-335`).

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::interact::action::{VizInputAction, VizOutputAction};
use crate::scale::Scale;
use crate::theme::VizTheme;

/// Width (px) of the two edge-handle bars drawn at the brush boundaries.
const HANDLE_WIDTH: f64 = 3.0;
/// Fill alpha for the translucent brush-selection rectangle.
const FILL_ALPHA: f64 = 0.18;

/// Live 1D horizontal-brush state. `anchor_px` is where the drag started,
/// `current_px` tracks the live (or final) drag position. `active` means
/// a selection exists — it stays `true` after `DragEnd`/[`BrushState::update`]
/// (the selection persists, visible, until [`BrushState::clear`] — the
/// usual brush UX: the highlighted range doesn't vanish on mouse-up).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BrushState {
    pub anchor_px: f64,
    pub current_px: f64,
    pub active: bool,
}

impl BrushState {
    /// Begin a new selection anchored at `px`.
    pub fn start(&mut self, px: f64) {
        self.anchor_px = px;
        self.current_px = px;
        self.active = true;
    }

    /// Update the live end of the selection. No-op if not active.
    pub fn update(&mut self, px: f64) {
        if self.active {
            self.current_px = px;
        }
    }

    /// Discard the selection entirely.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Screen-pixel `(left, right)` extent of the current selection,
    /// ordered regardless of drag direction.
    pub fn pixel_range(&self) -> (f64, f64) {
        (self.anchor_px.min(self.current_px), self.anchor_px.max(self.current_px))
    }

    /// Domain-space `(min, max)` interval via [`Scale::invert`] — the same
    /// normalized-`[0, 1]` round trip [`PlotArea::x`] uses for render
    /// (design law #1), so a brush rendered on screen and its reported
    /// domain interval never disagree. `None` when inactive, or when
    /// `area` has zero width (nothing to invert against).
    pub fn domain_interval(&self, area: &PlotArea, scale: &dyn Scale) -> Option<(f64, f64)> {
        if !self.active {
            return None;
        }
        let width = area.rect.width;
        if width.abs() < f64::EPSILON {
            return None;
        }
        let (lo_px, hi_px) = self.pixel_range();
        let t0 = ((lo_px - area.rect.x) / width).clamp(0.0, 1.0);
        let t1 = ((hi_px - area.rect.x) / width).clamp(0.0, 1.0);
        let d0 = scale.invert(t0);
        let d1 = scale.invert(t1);
        Some((d0.min(d1), d0.max(d1)))
    }

    /// Feed a semantic action into the brush. Only `DragStart`/`DragMove`/
    /// `DragEnd` are meaningful here — everything else is another
    /// component's concern (crosshair hover, click-select, ...) and
    /// yields [`VizOutputAction::None`].
    pub fn handle(&mut self, action: &VizInputAction, area: &PlotArea, scale: &dyn Scale) -> VizOutputAction {
        match *action {
            VizInputAction::DragStart { x, .. } => {
                self.start(x);
                VizOutputAction::Redraw
            }
            VizInputAction::DragMove { x, .. } if self.active => {
                self.update(x);
                VizOutputAction::BrushChanged { interval: self.domain_interval(area, scale) }
            }
            VizInputAction::DragEnd { x, .. } if self.active => {
                self.update(x);
                VizOutputAction::BrushChanged { interval: self.domain_interval(area, scale) }
            }
            _ => VizOutputAction::None,
        }
    }
}

/// Draw the live/committed brush selection: a translucent fill spanning
/// `[anchor_px, current_px]` across the plot's full height, plus two thin
/// edge-handle bars at the selection boundaries. No-op when `state` is
/// inactive.
pub fn draw_brush_overlay(ctx: &mut dyn RenderContext, area: &PlotArea, state: &BrushState, theme: &VizTheme) {
    if !state.active {
        return;
    }
    let (x0, x1) = state.pixel_range();
    let width = (x1 - x0).max(0.0);
    let accent = &theme.palette[0];

    ctx.set_fill_color(accent);
    ctx.set_global_alpha(FILL_ALPHA);
    ctx.fill_rect(x0, area.rect.y, width, area.rect.height);
    ctx.set_global_alpha(1.0);

    ctx.set_fill_color(accent);
    let left_handle_x = (x0 - HANDLE_WIDTH / 2.0).max(area.rect.x);
    let right_handle_x = (x1 - HANDLE_WIDTH / 2.0).min(area.rect.right() - HANDLE_WIDTH);
    ctx.fill_rect(left_handle_x, area.rect.y, HANDLE_WIDTH, area.rect.height);
    ctx.fill_rect(right_handle_x, area.rect.y, HANDLE_WIDTH, area.rect.height);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use uzor::types::Rect;

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0))
    }

    #[test]
    fn inactive_brush_reports_no_interval() {
        let brush = BrushState::default();
        assert!(!brush.is_active());
        assert_eq!(brush.domain_interval(&area(), &LinearScale::new(0.0, 100.0)), None);
    }

    #[test]
    fn drag_sequence_domain_interval_matches_scale_invert() {
        let a = area();
        let scale = LinearScale::new(0.0, 100.0);
        let mut brush = BrushState::default();

        brush.start(50.0); // px 50 -> t = 0.25 -> domain 25
        brush.update(150.0); // px 150 -> t = 0.75 -> domain 75
        let (d0, d1) = brush.domain_interval(&a, &scale).expect("active brush should report an interval");
        assert!((d0 - 25.0).abs() < 1e-9);
        assert!((d1 - 75.0).abs() < 1e-9);

        // Reverse drag direction — interval stays ordered (min, max).
        brush.update(10.0); // px 10 -> t = 0.05 -> domain 5
        let (d0, d1) = brush.domain_interval(&a, &scale).expect("still active after another update");
        assert!((d0 - 5.0).abs() < 1e-9);
        assert!((d1 - 25.0).abs() < 1e-9);

        brush.clear();
        assert_eq!(brush.domain_interval(&a, &scale), None);
    }

    #[test]
    fn handle_drag_lifecycle_produces_expected_outputs() {
        let a = area();
        let scale = LinearScale::new(0.0, 100.0);
        let mut brush = BrushState::default();

        let out = brush.handle(&VizInputAction::DragStart { x: 50.0, y: 0.0 }, &a, &scale);
        assert_eq!(out, VizOutputAction::Redraw);
        assert!(brush.is_active());

        let out = brush.handle(&VizInputAction::DragMove { x: 150.0, y: 0.0 }, &a, &scale);
        match out {
            VizOutputAction::BrushChanged { interval: Some((d0, d1)) } => {
                assert!((d0 - 25.0).abs() < 1e-9);
                assert!((d1 - 75.0).abs() < 1e-9);
            }
            other => panic!("expected BrushChanged with an interval, got {other:?}"),
        }

        let out = brush.handle(&VizInputAction::DragEnd { x: 150.0, y: 0.0 }, &a, &scale);
        assert!(matches!(out, VizOutputAction::BrushChanged { interval: Some(_) }));
        assert!(brush.is_active(), "selection persists after drag end");
    }

    #[test]
    fn handle_ignores_non_drag_actions() {
        let a = area();
        let scale = LinearScale::new(0.0, 100.0);
        let mut brush = BrushState::default();
        let out = brush.handle(&VizInputAction::Hover { x: 10.0, y: 10.0 }, &a, &scale);
        assert_eq!(out, VizOutputAction::None);
        assert!(!brush.is_active());
    }

    #[test]
    fn handle_ignores_drag_move_end_without_a_prior_start() {
        let a = area();
        let scale = LinearScale::new(0.0, 100.0);
        let mut brush = BrushState::default();
        let out = brush.handle(&VizInputAction::DragMove { x: 50.0, y: 0.0 }, &a, &scale);
        assert_eq!(out, VizOutputAction::None);
        assert!(!brush.is_active());
    }
}
