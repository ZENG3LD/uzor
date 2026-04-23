//! Scroll-linked animation effects for uzor UI framework.
//!
//! Provides three scroll-based animation effects:
//! - `ScrollReveal`: Word-by-word text reveal with opacity, blur, and rotation
//! - `ScrollVelocity`: Infinite horizontal scroll with velocity-based speed
//! - `ScrollFloat`: Parallax character float effect

pub mod scroll_reveal;
pub mod scroll_velocity;
pub mod scroll_float;

pub use scroll_reveal::{ScrollReveal, ScrollRevealConfig, WordState};
pub use scroll_velocity::{ScrollVelocity, ScrollVelocityConfig};
pub use scroll_float::{ScrollFloat, ScrollFloatConfig, CharState};
