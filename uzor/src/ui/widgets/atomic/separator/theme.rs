//! Separator colour palette.

pub trait SeparatorTheme {
    fn line(&self)          -> &str;
    fn handle_hover(&self)  -> &str;
    fn handle_active(&self) -> &str;
}

#[derive(Default)]
pub struct DefaultSeparatorTheme;

impl SeparatorTheme for DefaultSeparatorTheme {
    fn line(&self)          -> &str { "#3a3a3a" }
    fn handle_hover(&self)  -> &str { "#787b86" }
    fn handle_active(&self) -> &str { "#2962ff" }
}
