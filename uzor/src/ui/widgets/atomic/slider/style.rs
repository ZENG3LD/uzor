//! Slider geometry — values from mlc research.

pub trait SliderStyle {
    // ── Track ────────────────────────────────────────────────────────────────

    /// Track height (mlc default 4.0).
    fn track_height(&self) -> f64;
    /// Track corner radius (mlc 2.0).
    fn track_radius(&self) -> f64;

    // ── Handle ───────────────────────────────────────────────────────────────

    /// Handle (thumb) radius (mlc 7.0).
    fn handle_radius(&self) -> f64;
    /// Extra radius added on top of `handle_radius` to form the hover halo
    /// (mlc stroke width 2.0 → treated as extra ring, so we use 3.0 to keep
    ///  the halo slightly larger than the handle).
    fn handle_hover_ring(&self) -> f64;
    /// Handle border stroke thickness (mlc 1.0).
    fn handle_border_width(&self) -> f64;

    // ── Layout ───────────────────────────────────────────────────────────────

    /// Gap between label text and track left edge (mlc 12.0).
    fn label_spacing(&self) -> f64;
    /// Gap between track right edge and inline value input (mlc 12.0).
    fn track_input_spacing(&self) -> f64;

    // ── Label / text ─────────────────────────────────────────────────────────

    /// Font size for label + value text (mlc 12.0).
    fn font_size(&self) -> f64;

    // ── Input box ────────────────────────────────────────────────────────────

    /// Inner horizontal padding of the value input box (mlc 4.0).
    fn input_padding(&self) -> f64;
    /// Corner radius of the value input box (mlc 4.0).
    fn input_radius(&self) -> f64;
    /// Border thickness of value input box, normal state (mlc 1.0).
    fn input_border_width_normal(&self) -> f64;
    /// Border thickness of value input box, focused / editing state (mlc 1.5).
    fn input_border_width_focused(&self) -> f64;

    // ── Line-width variant (1.5) ──────────────────────────────────────────────

    /// Handle radius for the manual line-width slider (mlc 6.0).
    fn lw_handle_radius(&self) -> f64;
    /// Pixels reserved right of the line-width track for the value label
    /// (e.g. "3.5px") (mlc 52.0 total, gap 8.0).
    fn lw_label_gap(&self) -> f64;
    /// Gap between label text and track (mlc compare_settings LABEL_W already
    /// accounts for this; exposed here so callers can replicate it).
    fn lw_label_reserved(&self) -> f64;
}

pub struct DefaultSliderStyle;

impl Default for DefaultSliderStyle {
    fn default() -> Self {
        Self
    }
}

impl SliderStyle for DefaultSliderStyle {
    fn track_height(&self)               -> f64 { 4.0 }
    fn track_radius(&self)               -> f64 { 2.0 }
    fn handle_radius(&self)              -> f64 { 7.0 }
    fn handle_hover_ring(&self)          -> f64 { 3.0 }
    fn handle_border_width(&self)        -> f64 { 1.0 }
    fn label_spacing(&self)              -> f64 { 12.0 }
    fn track_input_spacing(&self)        -> f64 { 12.0 }
    fn font_size(&self)                  -> f64 { 12.0 }
    fn input_padding(&self)              -> f64 { 4.0 }
    fn input_radius(&self)               -> f64 { 4.0 }
    fn input_border_width_normal(&self)  -> f64 { 1.0 }
    fn input_border_width_focused(&self) -> f64 { 1.5 }
    fn lw_handle_radius(&self)           -> f64 { 6.0 }
    fn lw_label_gap(&self)               -> f64 { 8.0 }
    fn lw_label_reserved(&self)          -> f64 { 52.0 }
}
