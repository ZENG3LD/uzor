//! Toolbar style trait.

pub trait ToolbarStyle {}

#[derive(Default)]
pub struct DefaultToolbarStyle;

impl ToolbarStyle for DefaultToolbarStyle {}
