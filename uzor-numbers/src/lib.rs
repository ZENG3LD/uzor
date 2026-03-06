//! Animated number displays for uzor
//!
//! This crate provides rendering-agnostic number animation state computation.
//! It computes positions, opacities, and values that a renderer can use.
//!
//! # Components
//!
//! - [`Counter`] - Rolling slot-machine style digit display with spring physics
//! - [`CountUp`] - Spring-animated number that counts from start to end value

pub mod counter;
pub mod count_up;

pub use counter::{Counter, CounterState, DigitState, PlaceValue};
pub use count_up::{CountUp, CountUpState, Direction};
