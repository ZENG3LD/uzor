//! Panel style trait.

pub trait PanelStyle {}

#[derive(Default)]
pub struct DefaultPanelStyle;

impl PanelStyle for DefaultPanelStyle {}
