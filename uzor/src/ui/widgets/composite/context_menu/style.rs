//! ContextMenu style trait.

pub trait ContextMenuStyle {}

#[derive(Default)]
pub struct DefaultContextMenuStyle;

impl ContextMenuStyle for DefaultContextMenuStyle {}
