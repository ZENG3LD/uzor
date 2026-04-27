//! BlackboxPanel style trait.

pub trait BlackboxPanelStyle {}

#[derive(Default)]
pub struct DefaultBlackboxPanelStyle;

impl BlackboxPanelStyle for DefaultBlackboxPanelStyle {}
