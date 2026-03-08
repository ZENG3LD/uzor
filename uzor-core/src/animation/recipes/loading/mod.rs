//! Loading animation recipes
//!
//! Pre-configured loading spinners, skeleton screens, and progress indicators
//! following Material Design, iOS, SpinKit, and modern web best practices.
//!
//! Based on research from:
//! - Material Design 3 Progress Indicators
//! - SpinKit CSS animations
//! - Loading shimmer patterns
//! - SVG stroke drawing techniques

pub mod types;
pub mod presets;
pub mod defaults;
pub mod builders;

pub use types::*;
pub use presets::*;
pub use defaults::*;
pub use builders::*;
