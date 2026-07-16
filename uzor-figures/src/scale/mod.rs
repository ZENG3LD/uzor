//! Scale layer — domain (data space) <-> normalized `[0, 1]` <-> (via
//! [`crate::coord::PlotArea`]) screen pixels.
//!
//! One [`Scale`] trait, four implementations: [`LinearScale`],
//! [`LogScale`], [`BandScale`], [`TimeScale`] (calendar-aware tick
//! generation, harvested from mlc's ~1900-line module — see
//! [`mod@time`]'s own docs for exactly what was ported/dropped).
//! `ColorScale` is a later milestone — not in this crate yet, see the
//! crate-root docs.

pub mod band;
pub mod linear;
pub mod log;
pub mod time;

pub use band::BandScale;
pub use linear::LinearScale;
pub use log::LogScale;
pub use time::TimeScale;

/// One tick mark: a domain value plus its display label.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// Domain <-> normalized-range mapping, shared by every mark/guide draw
/// function through [`crate::coord::PlotArea`] — design law #1 (one
/// transform for render, and for any future hit-test).
pub trait Scale {
    /// The scale's data-space domain: `(min, max)` for continuous scales,
    /// `(0, category_count)` for [`BandScale`].
    fn domain(&self) -> (f64, f64);

    /// Map a domain value to normalized `[0, 1]`. May extrapolate outside
    /// `[0, 1]` for values outside the domain — callers clip if needed.
    fn map(&self, v: f64) -> f64;

    /// Best-effort inverse of [`map`](Self::map): normalized `[0, 1]` back
    /// to a domain value.
    fn invert(&self, t: f64) -> f64;

    /// Tick marks for an axis/grid targeting roughly `target_count` ticks
    /// (an aim, not a guarantee — nice-number rounding and, for
    /// [`BandScale`], "one tick per category" both take precedence).
    fn ticks(&self, target_count: usize) -> Vec<Tick>;
}
