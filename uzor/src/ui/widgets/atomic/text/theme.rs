//! Text widget color theme.

/// Color palette for the Text widget.
pub trait TextTheme: Send + Sync {
    /// Default text color when `view.color` is `None` and not hovered.
    fn text_color(&self) -> &str;
    /// Text color when hovered.
    fn text_color_hover(&self) -> &str;
}

/// Dark-UI default: muted white idle, pure white on hover.
#[derive(Default)]
pub struct DefaultTextTheme;

impl TextTheme for DefaultTextTheme {
    fn text_color(&self)       -> &str { "#d1d4dc" }
    fn text_color_hover(&self) -> &str { "#ffffff" }
}
