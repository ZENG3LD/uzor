//! The shaping seam — [`LineShaper`] is the swap point (cosmic-text today,
//! parley later if ever, per the design doc's crate architecture).

mod cosmic;

pub use cosmic::CosmicShaper;

use uzor::render::WrappedLine;

use crate::model::FontSpec;

/// Word-wraps `text` in `font` to `max_width`, returning one
/// [`WrappedLine`] per visual line.
///
/// Implementations are stateless (design law 3): every call is a pure
/// function of `(text, font, max_width)`. [`CosmicShaper`] is the only
/// implementation today.
pub trait LineShaper {
    fn shape_wrapped(&self, text: &str, font: &FontSpec, max_width: f64) -> Vec<WrappedLine>;
}
