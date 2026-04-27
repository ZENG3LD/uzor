//! Chrome theme trait.

pub trait ChromeTheme {}

#[derive(Default)]
pub struct DefaultChromeTheme;

impl ChromeTheme for DefaultChromeTheme {}
