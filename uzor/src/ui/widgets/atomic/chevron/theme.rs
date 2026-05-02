//! Chevron colours.

pub trait ChevronTheme {
    fn color(&self)          -> &str;
    fn color_hover(&self)    -> &str;
    fn color_pressed(&self)  -> &str;
    fn color_disabled(&self) -> &str;
    fn color_active(&self)   -> &str;
    fn bg_hover(&self)       -> &str;
}

#[derive(Default)]
pub struct DefaultChevronTheme;

impl ChevronTheme for DefaultChevronTheme {
    fn color(&self)          -> &str { "#a0a8b8" }
    fn color_hover(&self)    -> &str { "#d1d4dc" }
    fn color_pressed(&self)  -> &str { "#8088a0" }
    fn color_disabled(&self) -> &str { "#404858" }
    fn color_active(&self)   -> &str { "#4080ff" }
    fn bg_hover(&self)       -> &str { "rgba(255,255,255,0.08)" }
}
