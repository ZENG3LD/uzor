//! Animation coordination module
//!
//! Bridges uzor-animation primitives with uzor-core's per-frame render loop.
//! Provides an AnimationCoordinator that manages active animations keyed by
//! (WidgetId, property_name) pairs.

mod coordinator;
mod types;

pub use coordinator::AnimationCoordinator;
pub use types::{ActiveAnimation, AnimationDriver, AnimationKey};
