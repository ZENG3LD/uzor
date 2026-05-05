//! L4 framework widget surface — declarative `lm::*` builders only.
//!
//! Framework apps drive the UI through [`lm`] exclusively.  Lower
//! tiers (raw `InputCoordinator` registration, `ContextManager`
//! paint-and-register helpers) live next to their owning manager
//! and are explicitly NOT re-exported here so app authors aren't
//! tempted to mix layers.
//!
//! Looking for the legacy shortcuts?
//! - L1: [`crate::input::builders`]
//! - L2: [`crate::app_context::builders`]

pub mod lm;
