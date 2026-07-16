//! Semantic input/output actions — generalized from mlc's
//! `ChartInputAction`/`ChartOutputAction`
//! (`engine/input/events/action.rs:33-174`, `engine/input/handler/
//! default.rs:242-392`). Platform-specific mouse/key vocabulary
//! (`MouseButton`, `KeyCode`, `Modifiers`, `DragMode`) and every
//! chart/primitive-domain output variant (pan/zoom viewport commands,
//! sub-pane resize, drawing-primitive drag, undo, context menus, kinetic
//! scroll) are dropped — V2 only needs enough semantic vocabulary for
//! crosshair hover and one 1D X-brush.

/// Semantic, platform-agnostic input intent for a figure panel. A thin
/// platform adapter (e.g. `App::on_event` in a demo/app) translates raw
/// pointer events into these; [`crate::interact::brush::BrushState`] and
/// [`crate::interact::focus::FocusSet`] consume them and reply with a
/// [`FigureOutputAction`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FigureInputAction {
    /// Cursor moved to `(x, y)` (screen px) while inside a figure's panel.
    Hover { x: f64, y: f64 },
    /// Cursor left the panel — clears hover/crosshair state.
    Leave,
    /// Drag gesture started at `(x, y)`.
    DragStart { x: f64, y: f64 },
    /// Drag gesture continued to `(x, y)`.
    DragMove { x: f64, y: f64 },
    /// Drag gesture released at `(x, y)`.
    DragEnd { x: f64, y: f64 },
    /// Single click at `(x, y)` (no drag threshold crossed).
    Click { x: f64, y: f64 },
    /// Scroll-wheel step at `(x, y)`. `delta` follows platform convention
    /// (positive = scroll down / away from the user). No V2 consumer
    /// reacts to this yet (no zoom in this milestone) — carried for
    /// parity with mlc's semantic-action vocabulary and so a future zoom
    /// handler has a variant to match on without an enum-shape change.
    Wheel { delta: f64, x: f64, y: f64 },
}

impl FigureInputAction {
    /// `true` for the three drag-lifecycle variants (mirrors mlc's
    /// `ChartInputAction::is_drag_action`).
    #[inline]
    pub fn is_drag(&self) -> bool {
        matches!(self, Self::DragStart { .. } | Self::DragMove { .. } | Self::DragEnd { .. })
    }

    /// The screen position carried by this action, if any (`Leave` has
    /// none).
    pub fn position(&self) -> Option<(f64, f64)> {
        match *self {
            Self::Hover { x, y }
            | Self::DragStart { x, y }
            | Self::DragMove { x, y }
            | Self::DragEnd { x, y }
            | Self::Click { x, y }
            | Self::Wheel { x, y, .. } => Some((x, y)),
            Self::Leave => None,
        }
    }
}

/// Output of a small per-concern handler ([`crate::interact::brush::BrushState::handle`]
/// and similar). Mirrors mlc's `ChartOutputAction` shape (semantic command,
/// not a raw state mutation) at a fraction of the variant count — no
/// tooltip/cursor/undo/context-menu/primitive-drag commands, since V2 has
/// no primitives, no undo, and each figure derives its own tooltip content
/// straight from borrowed data at render time (design law #3) rather than
/// receiving it as a command.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FigureOutputAction {
    /// Something changed that needs a repaint, with no more specific
    /// payload than that.
    Redraw,
    /// The brush selection changed; `interval` is the new domain-space
    /// `(min, max)` (or `None` if the brush just got cleared).
    BrushChanged { interval: Option<(f64, f64)> },
    /// Hover/selection state changed (see [`crate::interact::focus::FocusSet`]).
    HoverChanged,
    /// No action needed.
    #[default]
    None,
}

impl FigureOutputAction {
    /// `true` for every variant except `None` — a caller can use this to
    /// decide whether a redraw is warranted without matching every
    /// variant itself.
    #[inline]
    pub fn needs_redraw(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_drag_matches_only_drag_lifecycle_variants() {
        assert!(FigureInputAction::DragStart { x: 0.0, y: 0.0 }.is_drag());
        assert!(FigureInputAction::DragMove { x: 0.0, y: 0.0 }.is_drag());
        assert!(FigureInputAction::DragEnd { x: 0.0, y: 0.0 }.is_drag());
        assert!(!FigureInputAction::Hover { x: 0.0, y: 0.0 }.is_drag());
        assert!(!FigureInputAction::Click { x: 0.0, y: 0.0 }.is_drag());
        assert!(!FigureInputAction::Leave.is_drag());
    }

    #[test]
    fn position_extracts_xy_except_for_leave() {
        assert_eq!(FigureInputAction::Hover { x: 1.0, y: 2.0 }.position(), Some((1.0, 2.0)));
        assert_eq!(FigureInputAction::Wheel { delta: 1.0, x: 3.0, y: 4.0 }.position(), Some((3.0, 4.0)));
        assert_eq!(FigureInputAction::Leave.position(), None);
    }

    #[test]
    fn needs_redraw_is_false_only_for_none() {
        assert!(FigureOutputAction::Redraw.needs_redraw());
        assert!(FigureOutputAction::HoverChanged.needs_redraw());
        assert!(FigureOutputAction::BrushChanged { interval: None }.needs_redraw());
        assert!(!FigureOutputAction::None.needs_redraw());
        assert_eq!(FigureOutputAction::default(), FigureOutputAction::None);
    }
}
