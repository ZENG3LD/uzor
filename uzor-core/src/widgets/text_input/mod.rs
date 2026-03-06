//! Text input widget - Text, Number, Search, Password inputs

pub mod types;
pub mod theme;
pub mod state;
pub mod input;
pub mod behavior;

// Re-export types (production use)
pub use types::*;
pub use behavior::*;
pub use state::TextInputState;
pub use input::{TextInputHandler, DefaultTextInputHandler, KeyModifiers};
