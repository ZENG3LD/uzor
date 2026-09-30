//! Headless event-synthesis + a11y-tree query test harness.
//!
//! Feature `testing` — enable only in a test crate's `[dev-dependencies]`
//! (Cargo unifies a dependency's features across dep-kinds when building
//! test targets, so this is active only for `cargo test`, never for a
//! normal `cargo build --release` of a consumer that never enables it).
//!
//! Drives the SAME `InputCoordinator` / `EventProcessor` / `PlatformEvent`
//! pipeline real window backends use — no parallel input model to keep in
//! sync with production code.

mod a11y_query;
mod events;
mod harness;
mod render;

pub use a11y_query::{A11yQuery, A11yQueryError};
pub use events::EventSynthesizer;
pub use harness::{FrameOutcome, TestHarness};
pub use render::{DrawOp, NullRenderContext, RecordingRenderContext};
