//! `SelectionBus` — single-threaded shared interaction state, linking
//! otherwise-independent figures in a multi-panel layout (engine design
//! doc §3, "linked hover/brush across figures"; the report's #2<->#5
//! shared-time-brush requirement). A plain struct: figures read it via
//! [`crate::figure::FigureOverlay`] each frame, a demo/app's input routing
//! writes it. No channels, no async — the same per-frame "borrowed
//! snapshot, nothing retained across frames beyond this small bit of
//! state" discipline every other draw function in this crate follows
//! (design law #3), just hoisted one layer up to the app-composition
//! boundary that owns more than one figure at a time.

/// Screen-pixel hover position. Deliberately NOT a domain-space value:
/// different figures in the same layout can have entirely incompatible
/// domains (see the linked-brush demo, where a curve's index axis and a
/// histogram's sample-value axis share nothing) — screen pixels are the
/// only value every figure in a layout can interpret the same way
/// (namely: "is this inside MY OWN plot rect", via
/// [`crate::interact::hit::hit_zone`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverInfo {
    pub x: f64,
    pub y: f64,
}

/// Cross-figure interaction state for one multi-panel layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionBus {
    pub hover: Option<HoverInfo>,
    /// Domain-space `(min, max)` brush interval, already converted via
    /// [`crate::interact::brush::BrushState::domain_interval`] by whichever
    /// figure owns the drag. Every OTHER figure re-derives its own screen
    /// position for this interval through its own scale (design law #1) —
    /// never the dragging figure's pixels directly.
    pub brush: Option<(f64, f64)>,
    pub generation: u64,
}

impl SelectionBus {
    /// Set (or clear) the hover position. Returns `true` if it changed.
    pub fn set_hover(&mut self, hover: Option<HoverInfo>) -> bool {
        if self.hover == hover {
            return false;
        }
        self.hover = hover;
        self.generation += 1;
        true
    }

    /// Set (or clear) the linked brush interval. Returns `true` if it
    /// changed.
    pub fn set_brush(&mut self, brush: Option<(f64, f64)>) -> bool {
        if self.brush == brush {
            return false;
        }
        self.brush = brush;
        self.generation += 1;
        true
    }

    /// Clear both hover and brush. Returns `true` if either was set.
    pub fn clear(&mut self) -> bool {
        if self.hover.is_none() && self.brush.is_none() {
            return false;
        }
        self.hover = None;
        self.brush = None;
        self.generation += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_hover_bumps_generation_only_on_change() {
        let mut bus = SelectionBus::default();
        assert!(bus.set_hover(Some(HoverInfo { x: 1.0, y: 2.0 })));
        assert_eq!(bus.generation, 1);
        assert!(!bus.set_hover(Some(HoverInfo { x: 1.0, y: 2.0 })));
        assert_eq!(bus.generation, 1);
        assert!(bus.set_hover(None));
        assert_eq!(bus.generation, 2);
    }

    #[test]
    fn set_brush_bumps_generation_only_on_change() {
        let mut bus = SelectionBus::default();
        assert!(bus.set_brush(Some((10.0, 20.0))));
        assert!(!bus.set_brush(Some((10.0, 20.0))));
        assert!(bus.set_brush(Some((10.0, 25.0))));
        assert_eq!(bus.generation, 2);
    }

    #[test]
    fn clear_resets_both_fields_and_reports_whether_anything_changed() {
        let mut bus = SelectionBus::default();
        assert!(!bus.clear());
        bus.set_hover(Some(HoverInfo { x: 0.0, y: 0.0 }));
        bus.set_brush(Some((0.0, 1.0)));
        assert!(bus.clear());
        assert!(bus.hover.is_none());
        assert!(bus.brush.is_none());
    }
}
