//! Chrome style trait.

pub trait ChromeStyle {}

#[derive(Default)]
pub struct DefaultChromeStyle;

impl ChromeStyle for DefaultChromeStyle {}
