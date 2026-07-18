//! Scale layer — domain (data space) <-> normalized `[0, 1]` <-> (via
//! [`crate::coord::PlotArea`]) screen pixels.
//!
//! One [`Scale`] trait, four domain<->normalized-range implementations:
//! [`LinearScale`], [`LogScale`], [`BandScale`], [`TimeScale`]
//! (calendar-aware tick generation, harvested from mlc's ~1900-line module
//! — see [`mod@time`]'s own docs for exactly what was ported/dropped).
//!
//! [`color::ColorScale`] is a DIFFERENT kind of scale — continuous value
//! -> CSS hex color (not -> normalized `[0, 1]`), so it does not implement
//! [`Scale`] itself; it lives in this module because it's still a
//! domain-mapping primitive over the same "figures need a value ->
//! something" shape, driving heatmap/business-chart color encoding (see
//! `color`'s own module docs for the OKLCH machinery it reuses).
//!
//! [`format::NumberFormat`] is a THIRD kind of primitive again — value ->
//! display STRING (not -> `[0, 1]`, not -> color) — used by axis tick
//! labels ([`crate::guide::axis::draw_x_axis_formatted`]/
//! `draw_y_axis_formatted`) and any other caller wanting Si/Percent/
//! Currency-formatted numbers instead of this crate's default thousands-
//! grouped decimal (see `format`'s own module docs).

pub mod band;
pub mod color;
pub mod format;
pub mod linear;
pub mod log;
pub mod time;

pub use band::BandScale;
pub use color::ColorScale;
pub use format::NumberFormat;
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

    /// Format a domain value `v` as a display label suited to THIS scale's
    /// own data kind — e.g. a hover/tooltip/crosshair-cursor label for a
    /// value resolved through [`crate::interact::hit::nearest_point_x`].
    ///
    /// Default: derive a display step from this scale's own `ticks(6)`
    /// (roughly the precision an axis label at this scale would already
    /// show) and format through [`linear::format_value`]. [`TimeScale`]
    /// overrides this with a real calendar label instead of a raw
    /// Unix-second number — the whole point of giving this method a slot
    /// on the trait: a caller holding only a `&dyn Scale` (e.g.
    /// [`crate::figure::CurveFigure::with_x_scale`]'s override, or
    /// [`crate::guide::crosshair`]'s cursor label) can format a resolved
    /// value correctly without knowing the concrete scale type underneath
    /// — closes the "TimeScale override shows a raw number" gap flagged
    /// when `with_x_scale` first landed.
    fn format_value(&self, v: f64) -> String {
        let ticks = self.ticks(6);
        let step = if ticks.len() >= 2 {
            (ticks[1].value - ticks[0].value).abs().max(f64::EPSILON)
        } else {
            1.0
        };
        linear::format_value(v, step)
    }
}
