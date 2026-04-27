//! Dropdown style trait.

pub trait DropdownStyle {}

#[derive(Default)]
pub struct DefaultDropdownStyle;

impl DropdownStyle for DefaultDropdownStyle {}
