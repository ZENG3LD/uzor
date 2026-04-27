//! Popup style trait.

pub trait PopupStyle {}

#[derive(Default)]
pub struct DefaultPopupStyle;

impl PopupStyle for DefaultPopupStyle {}
