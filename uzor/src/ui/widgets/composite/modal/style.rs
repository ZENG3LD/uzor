//! Modal style trait.

pub trait ModalStyle {}

#[derive(Default)]
pub struct DefaultModalStyle;

impl ModalStyle for DefaultModalStyle {}
