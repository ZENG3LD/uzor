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
//!
//! The ASCII cell-shader engine (`AsciiGrid`/`CellShader`/`GlitchLetter`,
//! formerly `cell_shader` here) moved to `uzor-text::ascii` in Arc 2 Phase 4
//! (`nemo/docs/uzor-viz/uzor_text_arc2_design.md` §4) — a hard cutover, no
//! compat shim left in core. Consumers: `use uzor_text::ascii::{...}`.

pub mod shiny;
pub mod decrypt;
pub mod gradient;
pub mod fuzzy;

pub use shiny::{ShinyTextConfig, ShinyTextState};
pub use decrypt::{DecryptedTextConfig, DecryptedTextState, RevealDirection};
pub use gradient::{GradientTextConfig, GradientTextState, GradientDirection};
pub use fuzzy::{FuzzyTextConfig, FuzzyTextState, FuzzyDirection};
