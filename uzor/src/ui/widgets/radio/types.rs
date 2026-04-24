//! Radio group type definitions

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

#[derive(Debug, Clone, PartialEq)]
pub enum RadioType {
    /// Standard radio group with circle indicators
    Standard,
    /// Compact radio group (smaller circles, tighter spacing)
    Compact,
}

impl WidgetCapabilities for RadioType {
    fn sense(&self) -> Sense {
        Sense::CLICK
    }
}
