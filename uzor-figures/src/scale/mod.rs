//! Scale layer — domain (data space) <-> normalized `[0, 1]` <-> (via
//! [`crate::coord::PlotArea`]) screen pixels.
//!
//! One [`Scale`] trait, six domain<->normalized-range implementations:
//! [`LinearScale`], [`LogScale`], [`BandScale`], [`TimeScale`]
//! (calendar-aware tick generation, harvested from mlc's ~1900-line module
//! — see [`mod@time`]'s own docs for exactly what was ported/dropped),
//! [`SymlogScale`] (linear near zero, log beyond — the scale [`LogScale`]
//! cannot substitute for once data crosses zero or goes negative; see
//! [`mod@symlog`]'s own docs), and [`PowScale`] (sign-preserving exponent
//! mapping, e.g. [`PowScale::sqrt`] for area-to-value bubble-radius
//! encoding; see [`mod@pow`]'s own docs).
//!
//! [`color::ColorScale`] is a DIFFERENT kind of scale — continuous value
//! -> CSS hex color (not -> normalized `[0, 1]`), so it does not implement
//! [`Scale`] itself; it lives in this module because it's still a
//! domain-mapping primitive over the same "figures need a value ->
//! something" shape, driving heatmap/business-chart color encoding (see
//! `color`'s own module docs for the OKLCH machinery it reuses).
//! [`color::CategoricalScale`] is the DISCRETE sibling of `ColorScale` —
//! value(index) -> one of N distinct category hues, never interpolated
//! (see `color`'s own module doc for why it's a separate type).
//!
//! [`bin::ClassScale`] (implemented by [`bin::QuantizeScale`]/
//! [`bin::ThresholdScale`]/[`bin::QuantileScale`]) is a FOURTH kind of
//! primitive — continuous value -> discrete class INDEX (not -> `[0, 1]`,
//! not -> color) — the foundation for risk/class colouring and for a
//! legend that reads as classes rather than a ramp; see [`mod@bin`]'s own
//! docs for the three flavors and the output-type justification.
//!
//! [`format::NumberFormat`] is a FIFTH kind of primitive again — value ->
//! display STRING (not -> `[0, 1]`, not -> color, not -> class index) —
//! used by axis tick labels ([`crate::guide::axis::draw_x_axis_formatted`]/
//! `draw_y_axis_formatted`) and any other caller wanting Si/Percent/
//! Currency-formatted numbers instead of this crate's default thousands-
//! grouped decimal (see `format`'s own module docs).

pub mod band;
pub mod bin;
pub mod color;
pub mod format;
pub mod linear;
pub mod log;
pub mod pow;
pub mod symlog;
pub mod time;

pub use band::BandScale;
pub use bin::{ClassScale, QuantileScale, QuantizeScale, ThresholdScale};
pub use color::{CategoricalScale, ColorScale};
pub use format::NumberFormat;
pub use linear::LinearScale;
pub use log::LogScale;
pub use pow::PowScale;
pub use symlog::SymlogScale;
pub use time::TimeScale;

/// One tick mark: a domain value plus its display label.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// How strongly [`crate::guide::axis`]'s label-COLLISION skip should
/// PROTECT this tick's own label from being dropped when it overlaps a
/// neighbor — a SEPARATE, independent concern from [`Scale::tick_weight`]'s
/// existing visual-STYLE weight ([`crate::guide::axis::AxisTickWeightStyle`]'s
/// tick length/stroke width/major label color, a rendering-LOOK question
/// only reachable through the OPT-IN `_weighted` entry points).
/// [`TickPriority`] governs WHICH label survives a collision on the
/// DEFAULT, always-on entry points (`draw_x_axis`/`draw_y_axis`/
/// `draw_x_axis_overflow`) — a rendering-CORRECTNESS question: the defect
/// this type fixes is a [`crate::scale::SymlogScale`] axis silently
/// dropping its own `0.0` zero-crossing tick to a merely-intermediate
/// neighbor, with no notion that some ticks matter more than others. See
/// [`crate::guide::axis::resolve_label_priority`] for the exact
/// three-tier degrade order this drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickPriority {
    /// Absolute must-keep — dropped only if it would collide with
    /// ANOTHER `Critical` tick (in practice at most one per axis: the
    /// zero crossing on a scale whose domain spans zero).
    Critical,
    /// Protected — dropped only once neither a `Critical` tick nor a
    /// higher-preference `Major` neighbor already claims the space (see
    /// [`crate::guide::axis::resolve_label_priority`]'s own "extremes
    /// preferred" tie-break for the rare case where majors alone still
    /// collide).
    Major,
    /// Ordinary — every scale's DEFAULT (see [`Scale::tick_priority`]'s
    /// own doc comment), and the FIRST tier dropped once labels collide.
    #[default]
    Minor,
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

    /// Optional per-tick "how visually important is this boundary"
    /// classifier — `None` by default (every scale except [`TimeScale`],
    /// which overrides this with [`time::boundary_weight`]). Exists so a
    /// GENERIC axis/grid caller ([`crate::guide::axis::draw_x_axis_weighted`]/
    /// [`crate::guide::grid::draw_x_grid_weighted`] and their Y-axis
    /// counterparts) can style a major/medium calendar boundary distinctly
    /// from a minor one WITHOUT downcasting to a concrete scale type — the
    /// "tested hierarchy computed and thrown away" gap: [`TimeScale`]'s own
    /// [`time::TickMarkWeight`] hierarchy was fully built and unit-tested
    /// but never consulted by the shared axis/grid guides every
    /// `CurveFigure::with_x_scale(TimeScale)`/`ScatterFigure::
    /// with_x_scale(TimeScale)` actually renders through. Every scale that
    /// keeps the default (`None`) renders EXACTLY as before through the
    /// weighted entry points too — see those functions' own doc comments.
    fn tick_weight(&self, _v: f64) -> Option<time::TickMarkWeight> {
        None
    }

    /// This tick's own label-collision priority — see [`TickPriority`]'s
    /// own doc comment for the full "why this exists" reasoning.
    /// `Minor` by default for every scale. With EVERY tick reporting the
    /// same tier (this default, or any scale that never overrides it),
    /// [`crate::guide::axis::resolve_label_priority`] provably degenerates
    /// to EXACTLY this crate's pre-existing plain left-to-right greedy
    /// skip — proven by that function's own
    /// `uniform_priority_matches_the_plain_greedy_skip` regression test —
    /// so this method existing changes NOTHING for a scale that doesn't
    /// opt in. [`crate::scale::SymlogScale`] (zero crossing +decade
    /// boundaries), [`crate::scale::LogScale`] (decade boundaries),
    /// [`crate::scale::LinearScale`]/[`crate::scale::PowScale`] (zero,
    /// only when it's an actually-generated tick), and [`TimeScale`]
    /// (reusing its own [`time::boundary_weight`] hierarchy) all override
    /// this.
    fn tick_priority(&self, _v: f64) -> TickPriority {
        TickPriority::Minor
    }

    /// Construct a NEW scale of the SAME concrete kind as `self`, with
    /// domain `(min, max)` in place of this scale's own — the hook a
    /// [`crate::interact::viewport::Viewport`]-driven figure uses to
    /// render a DOMAIN WINDOW (whatever pan/zoom currently shows) instead
    /// of this scale's own full extent (see that module's own docs for
    /// the window model this closes the "NOT in this crate yet: zoom/pan"
    /// gap with). `None` (the default) means this scale kind hasn't opted
    /// in — every scale kind THIS crate ships overrides it
    /// ([`crate::scale::LinearScale`]/[`crate::scale::LogScale`]/
    /// [`crate::scale::SymlogScale`]/[`crate::scale::PowScale`]/
    /// [`crate::scale::TimeScale`]; [`crate::scale::BandScale`]
    /// deliberately does NOT — see that scale's own doc comment for why
    /// categorical windowing is out of scope this pass). Nothing calls
    /// this method before this item existed, so adding the default
    /// changes NO existing behavior for any scale that doesn't override
    /// it.
    ///
    /// This is also the seam a future `ScaleMode` (Manual/Auto/Focus
    /// runtime auto-range policy — `nemo/docs/uzor-engines/plans/
    /// engine-strengthening-arc-2026-07-24.md` Wave 3, NOT built yet) is
    /// expected to reuse: resolving "what scale should this frame actually
    /// render with" is the SAME "rebuild this scale's own kind over a
    /// different `(min, max)`" operation whether the new bounds came from
    /// a [`crate::interact::viewport::Viewport`]'s pan/zoom state or from
    /// an auto-range recompute over the currently-visible data.
    fn windowed(&self, _min: f64, _max: f64) -> Option<Box<dyn Scale>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_weight_defaults_to_none_for_a_scale_that_does_not_override_it() {
        let scale = LinearScale::new(0.0, 100.0);
        assert_eq!(scale.tick_weight(50.0), None, "a non-time scale must report no tick weight, leaving weighted axis/grid rendering unaffected");
    }

    #[test]
    fn tick_priority_defaults_to_minor_for_a_scale_that_does_not_override_it() {
        let scale = BandScale::new(vec!["a".to_owned()], 0.1);
        assert_eq!(scale.tick_priority(0.0), TickPriority::Minor, "a scale that doesn't override tick_priority must report Minor for every tick");
    }
}
