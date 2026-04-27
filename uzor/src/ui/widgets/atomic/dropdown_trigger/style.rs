//! DropdownTrigger geometry presets (sections 32-33).

/// Geometry for the `Split` render kind (section 32).
///
/// Controls overall corner radius, text padding, the chevron zone width,
/// and font size.
pub trait SplitDropdownStyle {
    /// Corner radius.  mlc: 4.0.
    fn radius(&self) -> f64;
    /// Left text padding from field left edge.  mlc: 8.0.
    fn text_padding_x(&self) -> f64;
    /// Width of the right chevron zone (vertical separator + chevron icon).
    /// mlc: 20.0.
    fn chevron_width(&self) -> f64;
    /// Font size for the current-value label.  mlc: 13.0.
    fn font_size(&self) -> f64;
    /// Border stroke width.  mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default `SplitDropdown` style.  Matches chart_settings.rs prototype values.
pub struct DefaultSplitDropdownStyle;

impl SplitDropdownStyle for DefaultSplitDropdownStyle {
    fn radius(&self)         -> f64 { 4.0 }
    fn text_padding_x(&self) -> f64 { 8.0 }
    fn chevron_width(&self)  -> f64 { 20.0 }
    fn font_size(&self)      -> f64 { 13.0 }
    fn border_width(&self)   -> f64 { 1.0 }
}

/// Geometry for the `Field` render kind (section 33).
///
/// Single-zone trigger styled as a form input with an inline chevron icon.
pub trait DropdownFieldStyle {
    /// Corner radius.  mlc: 4.0.
    fn radius(&self) -> f64;
    /// Left text padding.  mlc: 8.0.
    fn text_padding_x(&self) -> f64;
    /// Chevron icon side length.  mlc: 12.0.
    fn chevron_size(&self) -> f64;
    /// Gap between chevron right edge and field right edge.  mlc: 6.0.
    fn chevron_margin_right(&self) -> f64;
    /// Font size.  mlc: 13.0.
    fn font_size(&self) -> f64;
    /// Border stroke width.  mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default `DropdownField` style.  Matches alert_settings.rs prototype values.
pub struct DefaultDropdownFieldStyle;

impl DropdownFieldStyle for DefaultDropdownFieldStyle {
    fn radius(&self)               -> f64 { 4.0 }
    fn text_padding_x(&self)       -> f64 { 8.0 }
    fn chevron_size(&self)         -> f64 { 12.0 }
    fn chevron_margin_right(&self) -> f64 { 6.0 }
    fn font_size(&self)            -> f64 { 13.0 }
    fn border_width(&self)         -> f64 { 1.0 }
}
