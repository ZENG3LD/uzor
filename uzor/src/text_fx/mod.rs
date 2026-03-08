//! Text animation effects for uzor UI framework.
//!
//! Provides rendering-agnostic text animation effects that compute
//! animation state without doing any actual rendering. Each effect outputs
//! values that can be used by any rendering backend.
//!
//! # Effects
//!
//! - **ShinyText**: Metallic shine sweep across text using animated gradient
//! - **DecryptedText**: Scramble/reveal effect with sequential or random modes
//! - **GradientText**: Animated multi-color gradient sweep
//! - **FuzzyText**: Scanline-style horizontal/vertical displacement

pub mod shiny;
pub mod decrypt;
pub mod gradient;
pub mod fuzzy;

pub use shiny::{ShinyTextConfig, ShinyTextState};
pub use decrypt::{DecryptedTextConfig, DecryptedTextState, RevealDirection};
pub use gradient::{GradientTextConfig, GradientTextState, GradientDirection};
pub use fuzzy::{FuzzyTextConfig, FuzzyTextState, FuzzyDirection};
