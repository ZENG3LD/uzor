//! Text widget layout style.

/// Layout and typography style for the Text widget.
pub trait TextStyle: Send + Sync {
    /// Default font CSS shorthand used when `view.font` is `None`.
    fn font(&self) -> &str;
    /// Inset from the left edge of the rect (pixels). Applied for `Left`-aligned text.
    fn padding_left(&self)  -> f64 { 4.0 }
    /// Inset from the right edge of the rect (pixels). Applied for `Right`-aligned and
    /// ellipsis budget computation.
    fn padding_right(&self) -> f64 { 4.0 }
}

/// Default style — 13 px Roboto, 4 px side padding.
#[derive(Default)]
pub struct DefaultTextStyle;

impl TextStyle for DefaultTextStyle {
    fn font(&self) -> &str { "13px Roboto" }
}
