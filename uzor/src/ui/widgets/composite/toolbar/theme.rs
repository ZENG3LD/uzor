//! Toolbar theme trait.

pub trait ToolbarTheme {}

#[derive(Default)]
pub struct DefaultToolbarTheme;

impl ToolbarTheme for DefaultToolbarTheme {}
