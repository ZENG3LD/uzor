//! ChromeTab style trait.

pub trait ChromeTabStyle {}

#[derive(Default)]
pub struct DefaultChromeTabStyle;

impl ChromeTabStyle for DefaultChromeTabStyle {}
