//! BlackboxPanel theme trait.

pub trait BlackboxPanelTheme {}

#[derive(Default)]
pub struct DefaultBlackboxPanelTheme;

impl BlackboxPanelTheme for DefaultBlackboxPanelTheme {}
