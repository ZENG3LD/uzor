//! Modal theme trait.

pub trait ModalTheme {}

#[derive(Default)]
pub struct DefaultModalTheme;

impl ModalTheme for DefaultModalTheme {}
