//! `Viewport` — a stateful, DOMAIN-based visible-window model: pan/zoom/
//! fit/reset over a scale's own domain, harvested (concept-only) from
//! `mylittlechart`'s `Viewport` (`mlc-core/src/chart/types/viewport.rs`)
//! per `nemo/docs/uzor-engines/research/mlc-chart-engine-harvest-2026-07-24.md`
//! §3. This crate's own docs already flagged zoom/pan as absent — this
//! module closes that gap.
//!
//! ## Domain-based, not bar-index-based — the one thing NOT copied verbatim
//!
//! MLC's `Viewport` is `view_start: f64` (a FRACTIONAL BAR INDEX) +
//! `bar_spacing: f64` (pixels per bar) — every coordinate conversion goes
//! through "which bar, at what pixel spacing." That shape is correct for
//! MLC's own charts (every series there is an ordered array of bars at a
//! uniform cadence), but `uzor-figures` renders continuous numeric, time,
//! AND categorical domains through one shared [`crate::scale::Scale`]
//! trait — there is no universal "index" a `CurveFigure` over a
//! [`crate::scale::TimeScale`] or a plain numeric [`crate::scale::LinearScale`]
//! could window over. [`Viewport`] here is therefore expressed entirely in
//! DOMAIN units (whatever a scale's own `min`/`max` already mean — Unix
//! seconds, a raw value, ...), never an array position. A pan of `3.7`
//! domain units means exactly that, not "3.7 bars."
//!
//! ## The window model
//!
//! A [`Viewport`] tracks two DISTINCT domain intervals:
//! - [`Viewport::data_domain`] — the full extent of the underlying data
//!   (what a figure's own auto-computed [`Scale`](crate::scale::Scale)
//!   would show with no viewport at all). This crate's existing figures
//!   already recompute this fresh every render (e.g.
//!   [`crate::figure::CurveFigure::x_scale`]) — a [`Viewport`] does not
//!   replace that, it sits ALONGSIDE it (see [`Viewport::set_data_domain`]).
//! - [`Viewport::window`] — the currently VISIBLE sub-interval, produced by
//!   [`Viewport::pan`]/[`Viewport::zoom_at`]/[`Viewport::fit_to_data`]/
//!   [`Viewport::reset`]. A figure renders through THIS interval instead of
//!   the full domain when a caller opts in (see
//!   [`crate::figure::CurveFigure::render_with_viewport`]).
//!
//! ## Single-writer / intent-method discipline (copied as a DESIGN RULE)
//!
//! Per the harvest doc's own §3 verdict: MLC's `ChartContainer` keeps its
//! `viewport`/`price_scale` fields PRIVATE, mutated only through named
//! INTENT methods (`pan`, `zoom_*`, `reconcile_*`) — never a raw `_mut`
//! accessor. This type follows the same rule: every field is private,
//! every mutation is a named method (`pan`/`zoom_at`/`zoom_in`/`zoom_out`/
//! `fit_to_data`/`reset`/`set_data_domain`/`set_config`) that keeps the
//! window's own invariants (span limits, overscroll policy) intact —
//! there is no way to poke `window_min`/`window_max` into an inconsistent
//! state from outside this module.
//!
//! ## Ownership: caller-owned, figure-BORROWED (design law #3)
//!
//! A [`Viewport`] is NOT something a [`crate::figure`] figure retains —
//! same rule this crate's [`crate::interact::focus::FocusSet`]/
//! [`crate::interact::brush::BrushState`] already follow. A figure's own
//! `render_with_viewport` entry point takes `Option<&Viewport>`
//! (borrowed, read-only) and resolves its own scale through it fresh every
//! render; the CALLER (a demo, an app, a test) owns the `Viewport` value
//! across frames and drives it via the intent methods above in response to
//! real input events (drag, scroll-wheel, a "reset zoom" button, ...).
//!
//! ## Seam for the not-yet-built `ScaleMode` (Wave 3)
//!
//! `nemo/docs/uzor-engines/plans/engine-strengthening-arc-2026-07-24.md`'s
//! Wave 3 (NOT built yet, and deliberately not started by this module)
//! calls for a `ScaleMode` (Manual/Auto/Focus) runtime auto-RANGE policy —
//! MLC's own analog is ORTHOGONAL to its `Viewport` (a Y-axis auto-fit
//! question, not an X-window question). The seam this module leaves:
//! [`crate::scale::Scale::windowed`] (`(&self, min, max) -> Option<Box<dyn
//! Scale>>`) is the SAME "rebuild this scale's own concrete kind over a
//! different `(min, max)`" primitive [`Viewport`] uses to turn a window
//! into a renderable scale — a future `ScaleMode::Auto`/`Focus`
//! implementation resolving "what Y range should this frame show" would
//! reuse that exact method with bounds derived from whichever points are
//! currently visible (through THIS module's own [`Viewport::window`]),
//! rather than inventing a second "rebuild a scale over new bounds"
//! mechanism. Nothing in this module assumes or forecloses that — it only
//! windows the axis a caller explicitly hands it a `Viewport` for.
//!
//! ## Configurability (owner doctrine: no private constant governs visible
//! behavior)
//!
//! Every behavioral knob is a real, caller-settable [`ViewportConfig`]
//! field: [`ViewportConfig::min_span`]/`max_span` (zoom limits),
//! [`ViewportConfig::zoom_step`] (the per-`zoom_in`/`zoom_out` factor),
//! [`ViewportConfig::overscroll`] (hard-clamp at the data edges, or allow
//! panning past them — see [`OverscrollPolicy`]), and
//! [`ViewportConfig::pan_sensitivity`] (a multiplier [`Viewport::pan_px`]
//! applies on top of the natural "the window follows the cursor 1:1"
//! pixel-to-domain conversion). The ONE non-configurable floor
//! ([`Viewport`]'s internal safety floor guarding a truly zero/negative
//! span against a divide-by-zero in [`Viewport::zoom_at`]) is a
//! correctness guard, not a behavioral default — see
//! [`Viewport::safety_floor`]'s own doc comment; it sits at `~1e-12`
//! relative to the data's own magnitude, far below any `min_span` a real
//! caller would ever configure or any real interactive zoom session would
//! ever reach.

use crate::scale::Scale;

/// The multiplicative factor one `zoom_in`/`zoom_out` "step" applies when
/// [`ViewportConfig::zoom_step`] isn't overridden (or is set to an invalid
/// value, see [`ViewportConfig`]'s own doc comment) — `1.2` narrows/widens
/// the window by 20% per step, a common, gentle interactive-zoom default
/// (roughly matching d3-zoom's own wheel-delta scaling).
pub const DEFAULT_ZOOM_STEP: f64 = 1.2;

/// How [`Viewport::clamp_window`] treats a window that would extend past
/// the data domain's own edges (via [`Viewport::pan`]/[`Viewport::pan_px`]/
/// [`Viewport::zoom_at`] moving it there) — see [`ViewportConfig::overscroll`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverscrollPolicy {
    /// The window may never move past `[data_min, data_max]` — panning (or
    /// zooming out) that would cross an edge stops exactly AT that edge
    /// instead. Also implicitly caps the maximum reachable span at the
    /// data domain's own span (showing more than "all the data" would
    /// require overscrolling past an edge, which this policy forbids) —
    /// see [`Viewport::clamp_window`]'s own doc comment.
    Clamp,
    /// The window may extend past either edge — the conventional
    /// "rubber-band"/kinetic-scroll overscroll feel. `margin_fraction`
    /// bounds HOW FAR: `Some(f)` caps the overscroll at `f` times the
    /// window's OWN current span on that side (so the allowed slack
    /// scales with zoom level, the same way MLC's own "1-bar overscan"
    /// convention scales with `bar_spacing`); `None` allows unlimited
    /// overscroll (still bounded by [`ViewportConfig::max_span`] on the
    /// SPAN itself, just not on POSITION).
    Allow { margin_fraction: Option<f64> },
}

impl Default for OverscrollPolicy {
    /// [`OverscrollPolicy::Clamp`] — the safer, more conventional default
    /// for a fresh [`Viewport`] with no caller opinion yet (a brand-new
    /// pan/zoom session showing exactly the data, never blank margins).
    fn default() -> Self {
        OverscrollPolicy::Clamp
    }
}

/// Every behavioral knob a [`Viewport`] exposes — see this module's own
/// top-level "Configurability" section for the owner doctrine this
/// satisfies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportConfig {
    /// Smallest allowed window span (a hard zoom-IN limit) — `None`
    /// (default) means no explicit limit (still bounded by
    /// [`Viewport`]'s own internal safety floor, see that type's own doc
    /// comment — never a real-world-reachable limit).
    pub min_span: Option<f64>,
    /// Largest allowed window span (a hard zoom-OUT limit) — `None`
    /// (default) means no EXPLICIT cap; under
    /// [`OverscrollPolicy::Clamp`] the span is still implicitly bounded by
    /// the data domain's own span (see [`OverscrollPolicy::Clamp`]'s own
    /// doc comment), under [`OverscrollPolicy::Allow`] an unset `max_span`
    /// truly has no ceiling.
    pub max_span: Option<f64>,
    /// The multiplicative factor [`Viewport::zoom_in`]/[`Viewport::zoom_out`]
    /// apply per call — must be `> 1.0`; a non-finite or `<= 1.0` value is
    /// sanitized to [`DEFAULT_ZOOM_STEP`] (never silently produces a
    /// no-op or inverted zoom step).
    pub zoom_step: f64,
    /// How far panning/zooming may move the window past the data domain's
    /// own edges — see [`OverscrollPolicy`].
    pub overscroll: OverscrollPolicy,
    /// Multiplier [`Viewport::pan_px`] applies on top of the natural
    /// (span-per-pixel) domain delta — `1.0` (default) is "the window
    /// follows the cursor exactly," `< 1.0` feels slower, `> 1.0` feels
    /// faster. A non-finite value is sanitized to `1.0`.
    pub pan_sensitivity: f64,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            min_span: None,
            max_span: None,
            zoom_step: DEFAULT_ZOOM_STEP,
            overscroll: OverscrollPolicy::default(),
            pan_sensitivity: 1.0,
        }
    }
}

/// Guard a caller-supplied `(min, max)` domain against non-finite/reversed
/// input — same "never let a degenerate constructor argument propagate
/// NaN/inf through every later computation" convention
/// [`crate::scale::linear::nice_domain`] already establishes for this
/// crate's scale layer: a non-finite bound or `min > max` falls back to a
/// unit interval anchored at whichever bound IS finite (or `0.0`).
/// `min == max` (a genuine single-point domain) is left AS-IS — that's a
/// real, legal (if degenerate) domain, not an error; [`Viewport::safety_floor`]
/// is what keeps a zero-span WINDOW numerically safe, not this guard.
fn sanitize_domain(domain: (f64, f64)) -> (f64, f64) {
    let (min, max) = domain;
    if !min.is_finite() || !max.is_finite() || min > max {
        let anchor = if min.is_finite() { min } else { 0.0 };
        (anchor, anchor + 1.0)
    } else {
        (min, max)
    }
}

fn sanitize_config(config: ViewportConfig) -> ViewportConfig {
    let zoom_step = if config.zoom_step.is_finite() && config.zoom_step > 1.0 { config.zoom_step } else { DEFAULT_ZOOM_STEP };
    let min_span = config.min_span.filter(|s| s.is_finite() && *s > 0.0);
    let max_span = config.max_span.filter(|s| s.is_finite() && *s > 0.0);
    let pan_sensitivity = if config.pan_sensitivity.is_finite() { config.pan_sensitivity } else { 1.0 };
    ViewportConfig { min_span, max_span, zoom_step, overscroll: config.overscroll, pan_sensitivity }
}

/// A stateful, domain-based visible-window model — see this module's own
/// top-level docs for the full design (window model, single-writer
/// discipline, ownership, the `ScaleMode` seam).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    data_min: f64,
    data_max: f64,
    /// The window captured at construction (or the last [`Viewport::with_config`]
    /// call) — what [`Viewport::reset`] returns to. Deliberately NOT
    /// re-based by [`Viewport::set_data_domain`] — see that method's own
    /// doc comment for why `reset` and [`Viewport::fit_to_data`] are
    /// separate, independently useful operations.
    initial_min: f64,
    initial_max: f64,
    window_min: f64,
    window_max: f64,
    config: ViewportConfig,
}

impl Viewport {
    /// A fresh viewport over `data_domain`, showing all of it (window ==
    /// data domain) under [`ViewportConfig::default`].
    pub fn new(data_domain: (f64, f64)) -> Self {
        Self::with_config(data_domain, ViewportConfig::default())
    }

    /// Same as [`Viewport::new`], with an explicit [`ViewportConfig`].
    pub fn with_config(data_domain: (f64, f64), config: ViewportConfig) -> Self {
        let (data_min, data_max) = sanitize_domain(data_domain);
        let mut viewport = Self {
            data_min,
            data_max,
            initial_min: data_min,
            initial_max: data_max,
            window_min: data_min,
            window_max: data_max,
            config: sanitize_config(config),
        };
        viewport.clamp_window();
        viewport
    }

    /// This viewport's current config.
    pub fn config(&self) -> ViewportConfig {
        self.config
    }

    /// Replace this viewport's config, immediately re-legalizing the
    /// current window against the new limits (e.g. tightening `max_span`
    /// below the current span shrinks the window on the spot, centered on
    /// its own current center).
    pub fn set_config(&mut self, config: ViewportConfig) {
        self.config = sanitize_config(config);
        self.clamp_window();
    }

    /// The full extent of the underlying data — what a figure's own
    /// auto-computed scale would show with no viewport at all.
    pub fn data_domain(&self) -> (f64, f64) {
        (self.data_min, self.data_max)
    }

    /// The currently visible domain interval — what a figure renders
    /// through when handed this viewport (see
    /// [`crate::figure::CurveFigure::render_with_viewport`]).
    pub fn window(&self) -> (f64, f64) {
        (self.window_min, self.window_max)
    }

    /// `window().1 - window().0` — always `> 0.0` (never zero/negative,
    /// see [`Viewport::safety_floor`]).
    pub fn span(&self) -> f64 {
        self.window_max - self.window_min
    }

    /// Update the data domain (e.g. new data replaced the old). The
    /// current WINDOW is preserved as-is and then re-clamped against the
    /// NEW domain/limits (via [`Viewport::clamp_window`]) — never silently
    /// re-fit to the new extent (that's [`Viewport::fit_to_data`], an
    /// explicit, separate call). Does NOT touch the [`Viewport::reset`]
    /// snapshot — `reset` returning to the ORIGINAL view (captured at
    /// construction) regardless of how many times the underlying data set
    /// has since changed is the whole reason these are two distinct
    /// operations: `fit_to_data` tracks the LIVE data extent, `reset`
    /// returns to a frozen one.
    pub fn set_data_domain(&mut self, data_domain: (f64, f64)) {
        let (data_min, data_max) = sanitize_domain(data_domain);
        self.data_min = data_min;
        self.data_max = data_max;
        self.clamp_window();
    }

    /// Shift the window by `delta` domain units (positive = later/larger
    /// values move into view on the right/top). Clamped per
    /// [`Viewport::clamp_window`]. A non-finite `delta` is ignored
    /// (no-op) rather than corrupting the window.
    pub fn pan(&mut self, delta: f64) {
        if !delta.is_finite() {
            return;
        }
        self.window_min += delta;
        self.window_max += delta;
        self.clamp_window();
    }

    /// Convenience: pan by a SCREEN-PIXEL delta, converted to a domain
    /// delta via this window's own `span / plot_width_px` ratio (the exact
    /// inverse of [`crate::coord::PlotArea::x`]'s own mapping), scaled by
    /// [`ViewportConfig::pan_sensitivity`]. No-op for a non-finite
    /// `delta_px` or a `plot_width_px` too close to zero to divide by.
    pub fn pan_px(&mut self, delta_px: f64, plot_width_px: f64) {
        if !delta_px.is_finite() || plot_width_px.abs() < f64::EPSILON {
            return;
        }
        let domain_per_px = self.span() / plot_width_px;
        self.pan(delta_px * domain_per_px * self.config.pan_sensitivity);
    }

    /// Zoom so the window's span is scaled by `1.0 / factor` (`factor >
    /// 1.0` zooms IN/narrows, `0.0 < factor < 1.0` zooms OUT/widens),
    /// keeping `focal` (a domain value, NOT a pixel) fixed at the same
    /// FRACTIONAL position within the window — the standard "zoom to
    /// point" idiom (MLC's own `zoom_at`; d3-zoom's own wheel-anchor
    /// behavior) and the one users actually feel, since a plain
    /// center-anchored zoom drifts away from whatever the cursor was over.
    /// No-op for a non-finite/non-positive `focal`/`factor`, or a
    /// non-positive current span (defensive — [`Viewport::clamp_window`]
    /// never leaves the window at a non-positive span in practice).
    pub fn zoom_at(&mut self, focal: f64, factor: f64) {
        if !focal.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let span = self.span();
        if span <= 0.0 {
            return;
        }
        let frac = ((focal - self.window_min) / span).clamp(0.0, 1.0);
        let new_span = span / factor;
        self.window_min = focal - frac * new_span;
        self.window_max = self.window_min + new_span;
        self.clamp_window();
    }

    /// Zoom IN by one [`ViewportConfig::zoom_step`], anchored at `focal`
    /// (a domain value) or the window's own current center when `None`.
    pub fn zoom_in(&mut self, focal: Option<f64>) {
        let focal = focal.unwrap_or_else(|| (self.window_min + self.window_max) / 2.0);
        self.zoom_at(focal, self.config.zoom_step);
    }

    /// Zoom OUT by one [`ViewportConfig::zoom_step`] — see [`Viewport::zoom_in`].
    pub fn zoom_out(&mut self, focal: Option<f64>) {
        let focal = focal.unwrap_or_else(|| (self.window_min + self.window_max) / 2.0);
        self.zoom_at(focal, 1.0 / self.config.zoom_step);
    }

    /// Reset the window to show the CURRENT full data domain — a
    /// deliberate escape hatch: it may exceed [`ViewportConfig::max_span`]
    /// (showing everything sometimes needs a wider window than a
    /// configured zoom-out cap allows) and ignores
    /// [`OverscrollPolicy::Allow`]'s own margin. The very next
    /// [`Viewport::pan`]/[`Viewport::zoom_at`] call re-legalizes the
    /// window against the configured limits as usual — this method only
    /// grants a temporary, explicit "show all of it" view, not a
    /// permanent exemption. Guards ONLY against a literally zero/negative
    /// span (a degenerate `data_domain`, e.g. a single data point) via
    /// [`Viewport::safety_floor`] — never applies `min_span`/`max_span`
    /// beyond that.
    pub fn fit_to_data(&mut self) {
        self.window_min = self.data_min;
        self.window_max = self.data_max;
        let floor = self.safety_floor();
        if self.window_max - self.window_min < floor {
            let center = (self.window_min + self.window_max) / 2.0;
            self.window_min = center - floor / 2.0;
            self.window_max = center + floor / 2.0;
        }
    }

    /// Reset the window to the ORIGINAL view captured at construction (or
    /// the last [`Viewport::with_config`] call) — distinct from
    /// [`Viewport::fit_to_data`], which tracks whatever the CURRENT data
    /// domain is after any [`Viewport::set_data_domain`] calls. `reset`
    /// answers "undo every pan/zoom back to where this viewport started";
    /// `fit_to_data` answers "show all of whatever data exists right now."
    /// Re-clamped against the current config (unlike `fit_to_data`) — the
    /// initial window was already legal under whatever config existed at
    /// construction, but the config may have changed since via
    /// [`Viewport::set_config`].
    pub fn reset(&mut self) {
        self.window_min = self.initial_min;
        self.window_max = self.initial_max;
        self.clamp_window();
    }

    /// Internal division-by-zero guard, NOT a behavioral zoom limit — see
    /// this module's own top-level "Configurability" section. Scaled to
    /// the data domain's own magnitude (`~1e-12` relative) so it never
    /// distorts behavior for a domain of any real-world scale (nanosecond
    /// timestamps through billions), and is always far below any
    /// `min_span` a real caller would configure.
    fn safety_floor(&self) -> f64 {
        (self.data_min.abs().max(self.data_max.abs()).max(1.0)) * 1e-12
    }

    /// Re-legalize `window_min`/`window_max` against `data_min`/`data_max`
    /// and `config` — called at the end of every mutating method except
    /// [`Viewport::fit_to_data`] (see that method's own doc comment for
    /// why it's a deliberate exception).
    ///
    /// Order of operations: (1) resolve the effective `[min_span,
    /// max_span]` span bounds — under [`OverscrollPolicy::Clamp`], the
    /// effective max span can never exceed the data domain's own span
    /// (you cannot show "more than all the data" without overscrolling
    /// past an edge, which `Clamp` forbids); (2) if the current span falls
    /// outside those bounds, resize it (anchored on the window's own
    /// current center); (3) reposition the window so it respects
    /// `config.overscroll`'s own edge policy.
    fn clamp_window(&mut self) {
        let data_span = (self.data_max - self.data_min).max(0.0);
        let floor = self.safety_floor();

        let min_span = self.config.min_span.unwrap_or(floor).max(floor);
        let clamp_edges = matches!(self.config.overscroll, OverscrollPolicy::Clamp);
        let implicit_max = if clamp_edges { data_span.max(min_span) } else { f64::INFINITY };
        let max_span = self.config.max_span.unwrap_or(f64::INFINITY).min(implicit_max).max(min_span);

        let mut span = (self.window_max - self.window_min).max(floor);
        if span < min_span || span > max_span {
            let center = (self.window_min + self.window_max) / 2.0;
            span = span.clamp(min_span, max_span);
            self.window_min = center - span / 2.0;
            self.window_max = center + span / 2.0;
        }

        match self.config.overscroll {
            OverscrollPolicy::Clamp => {
                if span >= data_span {
                    // Wide enough to show everything — center on the data
                    // rather than leaving it wherever a prior pan left it
                    // (never partially off to one side of the data once
                    // the whole domain fits).
                    let center = (self.data_min + self.data_max) / 2.0;
                    self.window_min = center - span / 2.0;
                    self.window_max = center + span / 2.0;
                } else {
                    if self.window_min < self.data_min {
                        self.window_min = self.data_min;
                        self.window_max = self.data_min + span;
                    }
                    if self.window_max > self.data_max {
                        self.window_max = self.data_max;
                        self.window_min = self.data_max - span;
                    }
                }
            }
            OverscrollPolicy::Allow { margin_fraction: Some(fraction) } => {
                let margin = fraction.max(0.0) * span;
                let lo = self.data_min - margin;
                let hi = self.data_max + margin;
                if self.window_min < lo {
                    self.window_min = lo;
                    self.window_max = lo + span;
                }
                if self.window_max > hi {
                    self.window_max = hi;
                    self.window_min = hi - span;
                }
            }
            OverscrollPolicy::Allow { margin_fraction: None } => {
                // Unlimited overscroll — nothing further to clamp beyond
                // the span bounds already resolved above.
            }
        }
    }
}

/// Resolve a windowed [`Scale`] for `full_scale`'s own concrete kind over
/// `viewport`'s current window — the exact operation
/// [`crate::figure::CurveFigure::render_with_viewport`] performs, exposed
/// standalone so any future figure/consumer can reuse it without
/// duplicating the `Scale::windowed` call convention. `None` when
/// `full_scale`'s own kind doesn't override [`Scale::windowed`] (see that
/// method's own doc comment — every scale kind THIS crate ships does).
pub fn windowed_scale(full_scale: &dyn Scale, viewport: &Viewport) -> Option<Box<dyn Scale>> {
    let (min, max) = viewport.window();
    full_scale.windowed(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;

    #[test]
    fn new_viewport_shows_the_whole_data_domain() {
        let vp = Viewport::new((0.0, 100.0));
        assert_eq!(vp.window(), (0.0, 100.0));
        assert_eq!(vp.data_domain(), (0.0, 100.0));
    }

    #[test]
    fn pan_shifts_the_window_by_the_given_delta_when_it_stays_in_bounds() {
        let mut vp = Viewport::new((0.0, 1000.0));
        vp.zoom_at(500.0, 5.0); // narrow the window first so there's room to pan
        let (before_min, before_max) = vp.window();
        vp.pan(10.0);
        let (after_min, after_max) = vp.window();
        assert!((after_min - (before_min + 10.0)).abs() < 1e-9);
        assert!((after_max - (before_max + 10.0)).abs() < 1e-9);
    }

    #[test]
    fn clamp_overscroll_never_pans_past_either_data_edge() {
        let mut vp = Viewport::new((0.0, 100.0));
        vp.zoom_at(50.0, 4.0); // window now narrower than the data span
        vp.pan(-10_000.0); // try to pan far past the left edge
        let (min, _) = vp.window();
        assert!((min - 0.0).abs() < 1e-9, "Clamp must stop exactly at the left data edge, got min={min}");
        vp.pan(10_000.0); // try to pan far past the right edge
        let (_, max) = vp.window();
        assert!((max - 100.0).abs() < 1e-9, "Clamp must stop exactly at the right data edge, got max={max}");
    }

    #[test]
    fn allow_overscroll_with_no_margin_pans_freely_past_the_edges() {
        let config = ViewportConfig { overscroll: OverscrollPolicy::Allow { margin_fraction: None }, ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 100.0), config);
        vp.zoom_at(50.0, 4.0);
        vp.pan(-10_000.0);
        let (min, _) = vp.window();
        assert!(min < -1000.0, "unlimited overscroll must allow panning far past the left edge, got min={min}");
    }

    #[test]
    fn allow_overscroll_with_a_margin_caps_how_far_past_the_edge_it_can_go() {
        let config = ViewportConfig { overscroll: OverscrollPolicy::Allow { margin_fraction: Some(1.0) }, ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 100.0), config);
        vp.zoom_at(50.0, 4.0); // span now 25.0
        let span = vp.span();
        vp.pan(-10_000.0);
        let (min, _) = vp.window();
        // Margin is `1.0 * span` on that side — the window can go no
        // further left than `data_min - span`.
        assert!((min - (0.0 - span)).abs() < 1e-6, "a margin_fraction of 1.0 must cap overscroll at exactly one window-span past the edge, got min={min}");
    }

    #[test]
    fn zoom_at_keeps_the_focal_domain_value_at_the_same_fractional_position() {
        let mut vp = Viewport::new((0.0, 1000.0));
        let focal = 300.0;
        let (w0, w1) = vp.window();
        let frac_before = (focal - w0) / (w1 - w0);
        vp.zoom_at(focal, 3.0);
        let (w0, w1) = vp.window();
        let frac_after = (focal - w0) / (w1 - w0);
        assert!((frac_before - frac_after).abs() < 1e-9, "zoom_at must keep the focal value's fractional window position fixed");
    }

    #[test]
    fn zoom_at_with_factor_greater_than_one_narrows_the_window() {
        let mut vp = Viewport::new((0.0, 1000.0));
        let before = vp.span();
        vp.zoom_at(500.0, 2.0);
        let after = vp.span();
        assert!(after < before, "factor > 1.0 must narrow the window (before={before}, after={after})");
    }

    #[test]
    fn zoom_in_and_zoom_out_use_the_configured_zoom_step_and_are_inverses() {
        let config = ViewportConfig { zoom_step: 2.0, ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 1_000_000.0), config);
        vp.zoom_at(500_000.0, 100.0); // narrow well below data span so zoom_out has real room
        let span0 = vp.span();
        vp.zoom_in(None);
        let span1 = vp.span();
        assert!((span1 - span0 / 2.0).abs() < 1e-6, "zoom_in must narrow by exactly the configured zoom_step");
        vp.zoom_out(None);
        let span2 = vp.span();
        assert!((span2 - span0).abs() < 1e-6, "zoom_out must exactly undo the matching zoom_in");
    }

    #[test]
    fn non_finite_or_non_positive_zoom_step_falls_back_to_the_default() {
        let config = ViewportConfig { zoom_step: 0.5, ..ViewportConfig::default() };
        assert_eq!(Viewport::with_config((0.0, 10.0), config).config().zoom_step, DEFAULT_ZOOM_STEP);
        let config = ViewportConfig { zoom_step: f64::NAN, ..ViewportConfig::default() };
        assert_eq!(Viewport::with_config((0.0, 10.0), config).config().zoom_step, DEFAULT_ZOOM_STEP);
    }

    #[test]
    fn min_span_prevents_zooming_in_past_the_configured_limit() {
        let config = ViewportConfig { min_span: Some(10.0), ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 1000.0), config);
        vp.zoom_at(500.0, 1000.0); // try to zoom in far past the limit
        assert!(vp.span() >= 10.0 - 1e-9, "span must never go below the configured min_span, got {}", vp.span());
    }

    #[test]
    fn max_span_prevents_zooming_out_past_the_configured_limit_even_under_allow_overscroll() {
        let config = ViewportConfig {
            max_span: Some(50.0),
            overscroll: OverscrollPolicy::Allow { margin_fraction: None },
            ..ViewportConfig::default()
        };
        let mut vp = Viewport::with_config((0.0, 1000.0), config);
        vp.zoom_at(500.0, 100.0); // narrow first
        vp.zoom_at(500.0, 0.001); // then try to zoom out far past max_span
        assert!(vp.span() <= 50.0 + 1e-9, "span must never exceed the configured max_span, got {}", vp.span());
    }

    #[test]
    fn under_clamp_the_span_can_never_exceed_the_data_domain_even_with_a_larger_max_span_configured() {
        let config = ViewportConfig { max_span: Some(10_000.0), overscroll: OverscrollPolicy::Clamp, ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 100.0), config);
        vp.zoom_at(50.0, 10.0);
        vp.zoom_at(50.0, 0.0001); // try to zoom out far past the data span
        assert!(vp.span() <= 100.0 + 1e-6, "Clamp must cap the span at the data domain's own span regardless of a larger configured max_span");
    }

    #[test]
    fn fit_to_data_shows_the_full_current_data_domain_regardless_of_a_narrower_max_span() {
        let config = ViewportConfig { max_span: Some(1.0), ..ViewportConfig::default() };
        let mut vp = Viewport::with_config((0.0, 100.0), config);
        vp.fit_to_data();
        assert_eq!(vp.window(), (0.0, 100.0), "fit_to_data is a deliberate escape hatch past a configured max_span");
    }

    #[test]
    fn fit_to_data_tracks_the_live_data_domain_after_set_data_domain() {
        let mut vp = Viewport::new((0.0, 100.0));
        vp.zoom_at(50.0, 5.0);
        vp.set_data_domain((0.0, 500.0));
        vp.fit_to_data();
        assert_eq!(vp.window(), (0.0, 500.0), "fit_to_data must reflect the NEW data domain, not the one captured at construction");
    }

    #[test]
    fn reset_returns_to_the_view_captured_at_construction_even_after_the_data_domain_changed() {
        let mut vp = Viewport::new((0.0, 100.0));
        vp.zoom_at(50.0, 5.0);
        vp.set_data_domain((0.0, 500.0)); // data changed — fit_to_data would now show (0, 500)
        vp.reset();
        assert_eq!(vp.window(), (0.0, 100.0), "reset must return to the ORIGINAL construction-time view, independent of set_data_domain");
    }

    #[test]
    fn set_data_domain_re_clamps_a_window_that_no_longer_fits_the_shrunk_domain() {
        let mut vp = Viewport::new((0.0, 1000.0));
        vp.zoom_at(900.0, 2.0); // window now roughly [650, 1150]-ish before clamp, clamped to <=1000
        vp.set_data_domain((0.0, 500.0)); // shrink the domain well below the current window position
        let (min, max) = vp.window();
        assert!(min >= 0.0 - 1e-9 && max <= 500.0 + 1e-9, "window must be re-clamped inside the NEW, smaller domain, got ({min}, {max})");
    }

    // ── degenerate cases (zero-extent domain, single data point) ───────

    #[test]
    fn zero_extent_domain_does_not_panic_and_produces_a_usable_nonzero_window() {
        let vp = Viewport::new((5.0, 5.0));
        let (min, max) = vp.window();
        assert!(min.is_finite() && max.is_finite());
        assert!(max > min, "even a single-point domain must resolve to a strictly positive window span");
    }

    #[test]
    fn zero_extent_domain_zoom_and_pan_do_not_panic() {
        let mut vp = Viewport::new((5.0, 5.0));
        vp.pan(1.0);
        vp.zoom_at(5.0, 2.0);
        vp.zoom_in(None);
        vp.zoom_out(None);
        vp.fit_to_data();
        vp.reset();
        let (min, max) = vp.window();
        assert!(min.is_finite() && max.is_finite());
    }

    #[test]
    fn non_finite_or_reversed_domain_falls_back_to_a_safe_unit_domain() {
        let vp = Viewport::new((f64::NAN, 10.0));
        let (min, max) = vp.data_domain();
        assert!(min.is_finite() && max.is_finite() && max > min);

        let vp = Viewport::new((10.0, -10.0)); // reversed (min > max)
        let (min, max) = vp.data_domain();
        assert!(min.is_finite() && max.is_finite() && max > min);
    }

    // ── windowed_scale free function ────────────────────────────────────

    #[test]
    fn windowed_scale_rebuilds_the_scale_over_the_viewports_own_window() {
        let full = LinearScale::new(0.0, 1000.0);
        let mut vp = Viewport::new((0.0, 1000.0));
        vp.zoom_at(500.0, 4.0);
        let windowed = windowed_scale(&full, &vp).expect("LinearScale supports windowing");
        assert_eq!(windowed.domain(), vp.window());
    }
}
