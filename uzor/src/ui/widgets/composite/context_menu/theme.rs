//! ContextMenu theme trait.

pub trait ContextMenuTheme {}

#[derive(Default)]
pub struct DefaultContextMenuTheme;

impl ContextMenuTheme for DefaultContextMenuTheme {}
