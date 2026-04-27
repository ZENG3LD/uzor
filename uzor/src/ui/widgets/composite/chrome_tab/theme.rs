//! ChromeTab theme trait.

pub trait ChromeTabTheme {}

#[derive(Default)]
pub struct DefaultChromeTabTheme;

impl ChromeTabTheme for DefaultChromeTabTheme {}
