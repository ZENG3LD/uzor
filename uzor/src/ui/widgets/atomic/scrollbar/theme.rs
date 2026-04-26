//! Scrollbar colour palette.

pub trait ScrollbarTheme {
    fn track(&self)         -> &str;
    fn thumb_normal(&self)  -> &str;
    fn thumb_hover(&self)   -> &str;
    fn thumb_active(&self)  -> &str;
}

#[derive(Default)]
pub struct DefaultScrollbarTheme;

impl ScrollbarTheme for DefaultScrollbarTheme {
    fn track(&self)        -> &str { "#1e1e1e" }
    fn thumb_normal(&self) -> &str { "#5a5a5a" }
    fn thumb_hover(&self)  -> &str { "#787b86" }
    fn thumb_active(&self) -> &str { "#9aa0a6" }
}
