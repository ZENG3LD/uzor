//! Widget convenience functions over [`PanelCx`] and [`OverlayCx`].
//!
//! One call registers the widget with the hook's [`Widgets`] face and draws
//! it through the library paint function (design §6.4, brief F8). The
//! chainable [`lm`] builders are the same surface the [`view!`](crate::view)
//! macro lowers to (brief M1), retargeted off `LayoutManager` onto a
//! [`ContentCx`]. [`raw`] re-exports the level-1 register functions for
//! callers that draw themselves.
//!
//! Blackbox panels are not in this module. Overlay chain builders for
//! modal/dropdown/context menu/popup/panel/sidebar/toolbar were left
//! unported in F8 (they held `LayoutManager` handles); M1 does not revive
//! them.

mod atomics;
mod button;
mod composites;
mod host;
pub mod lm;
mod paint;
pub mod raw;

#[cfg(test)]
mod golden;

pub use atomics::{checkbox, chevron, separator, text, toggle, tooltip};
pub use button::button;
pub use composites::{chrome, context_menu, dropdown, modal, panel, popup, sidebar, toolbar};
pub use host::ContentCx;
