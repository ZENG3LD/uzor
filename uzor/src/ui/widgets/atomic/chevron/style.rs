//! Chevron geometry parameters.

pub trait ChevronStyle {
    /// Default visual size (square) when caller doesn't pre-size the rect.
    /// Default: 16.0.
    fn size(&self) -> f64 { 16.0 }

    /// Inset of the V-shape from the edge of the rect (Stroked variant only).
    /// Default: 4.0.
    fn inset(&self) -> f64 { 4.0 }

    /// Stroke thickness of the V-shape (Stroked variant only). Default: 1.5.
    fn thickness(&self) -> f64 { 1.5 }

    /// Side length of the filled triangle (Filled variant). Default: 6.0.
    fn triangle_size(&self) -> f64 { 6.0 }

    /// Font size when rendering as Glyph. Default: 12.0.
    fn glyph_size(&self) -> f64 { 12.0 }

    /// Corner radius of the hover background fill. Default: 4.0.
    fn hover_bg_radius(&self) -> f64 { 4.0 }
}

#[derive(Default)]
pub struct DefaultChevronStyle;
impl ChevronStyle for DefaultChevronStyle {}
