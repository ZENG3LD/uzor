//! Container variant catalog. Layout (rect) is layout-layer concern.
//!
//! Note: a *composite* scrollable container (Container + Scrollbar) is
//! a separate widget that lives in `composite/scroll_container/`. This
//! atomic container only manages the bg/border/clip; scrollbar
//! composition happens at the composite level.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerType {
    /// Plain — bg + optional border, no clipping by default.
    Plain,
    /// Clipping — same as Plain but renderer wraps children in `clip_rect`.
    Clip,
    /// Card — Plain + drop shadow + rounded corners (mlc modal panels).
    Card,
}

impl WidgetCapabilities for ContainerType {
    fn sense(&self) -> Sense {
        Sense::NONE
    }
}
