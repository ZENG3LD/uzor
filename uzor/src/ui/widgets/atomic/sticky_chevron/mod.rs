//! Sticky chevron atomic — overlay attached to one edge / corner of a
//! host. Owns its own rect + hit zone; clicks go to the chevron, not
//! the host.

pub mod render;
pub mod types;

pub use render::{draw_sticky_chevron, register_sticky_chevron};
pub use types::{
    place_sticky_chevron, StickyAnchor, StickyChevronSpec, StickyVisibility,
};
