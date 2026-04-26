//! Tab colour palette.

pub trait TabTheme {
    fn bg_normal(&self)   -> &str;
    fn bg_hover(&self)    -> &str;
    fn bg_active(&self)   -> &str;
    fn text_normal(&self) -> &str;
    fn text_active(&self) -> &str;
    fn accent(&self)      -> &str;
    fn close_normal(&self) -> &str;
    fn close_hover(&self)  -> &str;
}

#[derive(Default)]
pub struct DefaultTabTheme;

impl TabTheme for DefaultTabTheme {
    fn bg_normal(&self)   -> &str { "#1e1e1e" }
    fn bg_hover(&self)    -> &str { "#2a2a2a" }
    fn bg_active(&self)   -> &str { "#1e3a5f" }
    fn text_normal(&self) -> &str { "#787b86" }
    fn text_active(&self) -> &str { "#ffffff" }
    fn accent(&self)      -> &str { "#2962ff" }
    fn close_normal(&self) -> &str { "#787b86" }
    fn close_hover(&self)  -> &str { "#ffffff" }
}
