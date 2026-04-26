//! Tooltip colour palette.

pub trait TooltipTheme {
    fn bg(&self)     -> &str;
    fn border(&self) -> &str;
    fn text(&self)   -> &str;
}

#[derive(Default)]
pub struct DefaultTooltipTheme;

impl TooltipTheme for DefaultTooltipTheme {
    fn bg(&self)     -> &str { "#2a2a2a" }
    fn border(&self) -> &str { "#3a3a3a" }
    fn text(&self)   -> &str { "#ffffff" }
}
