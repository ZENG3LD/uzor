//! Tab geometry.

pub trait TabStyle {
    fn radius(&self)            -> f64;
    fn padding_x(&self)         -> f64;
    fn padding_y(&self)         -> f64;
    fn font_size(&self)         -> f64;
    fn icon_size(&self)         -> f64;
    fn gap(&self)               -> f64;
    fn close_btn_size(&self)    -> f64;
    /// Active accent bar thickness (left edge of vertical tab).
    fn accent_bar(&self)        -> f64;
}

pub struct DefaultTabStyle;

impl Default for DefaultTabStyle {
    fn default() -> Self {
        Self
    }
}

impl TabStyle for DefaultTabStyle {
    fn radius(&self)         -> f64 { 4.0 }
    fn padding_x(&self)      -> f64 { 12.0 }
    fn padding_y(&self)      -> f64 { 6.0 }
    fn font_size(&self)      -> f64 { 13.0 }
    fn icon_size(&self)      -> f64 { 14.0 }
    fn gap(&self)            -> f64 { 6.0 }
    fn close_btn_size(&self) -> f64 { 14.0 }
    fn accent_bar(&self)     -> f64 { 3.0 }
}
