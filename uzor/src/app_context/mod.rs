//! App-owned widget state registry helpers.
//!
//! The old context façade and micro-layout builders were removed in brief C1.
//! What remains is the typed state registry widgets and engines still use.

pub mod state;
pub mod widget_state;

pub use state::StateRegistry;
pub use widget_state::*;
