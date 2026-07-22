//! Generic named-stage frame profiler + the shared EMA helper it (and
//! `uzor-desktop::Manager`'s own whole-frame `fps_ema`) build on.
//!
//! Lifted from `foxhound-app-shell-native`'s own `FrameProfile`
//! (`nemo/foxhound/crates/foxhound-app-shell-native/src/main.rs` —
//! per-frame `Instant`-bracketed stage timings, EMA-smoothed, published
//! as JSON over its own `BlackboxAgentSurface::agent_state()` at
//! `"frame_profile_ema_ms"`; see
//! `nemo/docs/uzor-engines/research_foxhound_lift_candidates.md` §1). The
//! *stage names* there (`snapshot_sync`, `grid`, `guides`, ...) were
//! 100% app-specific; the *mechanism* — bracket named stages, EMA-smooth
//! each one independently, publish as a map — is fully generic, so stage
//! names here are caller-supplied `&'static str` keys, not a fixed enum.
//!
//! **This module owns NO clock.** It lives in the platform-agnostic core
//! (`std::time::Instant` panics at runtime on `wasm32-unknown-unknown`,
//! and this crate compiles for every window backend), so the aggregator
//! only ever accepts caller-measured durations via
//! [`FrameProfiler::record_ms`] — pure math + naming. The measuring edge
//! stays wherever a real monotonic clock exists (e.g.
//! `uzor-desktop::Manager` brackets with `Instant` and feeds the result
//! in; a web backend would feed `performance.now()` deltas).
//!
//! [`EmaF64`] deduplicates a smoothing formula that used to be
//! hand-rolled independently in two places: the source app's own
//! `FrameProfile::ema` (`previous * 0.92 + sample * 0.08`) and
//! `Manager`'s own whole-frame `fps_ema` (`previous * 0.9 + sample *
//! 0.1`). `Manager::fps_ema`'s public behavior (the `f32` field, its
//! exact smoothing formula/seed) is unchanged — it now computes that
//! same formula through an [`EmaF64`] tracker instead of the formula
//! written out inline a second time.

use std::collections::HashMap;

/// Exponential moving average over `f64` samples, with an
/// alpha-per-`update`-call smoothing factor (rather than a
/// fixed-at-construction one) so a single small primitive can back both
/// this module's per-stage profiler (alpha `0.08`, the source app's own
/// constant) and `uzor-desktop::Manager`'s own whole-frame `fps_ema`
/// (alpha `0.1`) without needing two copies of the formula.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmaF64 {
    value: f64,
    initialized: bool,
}

impl Default for EmaF64 {
    fn default() -> Self {
        Self { value: 0.0, initialized: false }
    }
}

impl EmaF64 {
    /// A fresh, uninitialized tracker — its very first [`EmaF64::update`]
    /// call wins outright (no smoothing against `0.0`).
    pub fn new() -> Self {
        Self::default()
    }

    /// A tracker pre-seeded with `value`, marked initialized — the first
    /// real [`EmaF64::update`] call blends against `value` instead of
    /// replacing it outright. Useful when a caller has a sensible
    /// starting guess (e.g. an assumed `60.0` fps before the first frame
    /// has actually been timed) rather than wanting the very first
    /// sample to jump straight to its own raw value.
    pub fn seeded(value: f64) -> Self {
        Self { value, initialized: true }
    }

    /// Fold `sample` into the running average at smoothing factor
    /// `alpha` (`0.0..=1.0`; higher = more responsive to new samples,
    /// lower = smoother/slower) and return the new value. The very first
    /// call on an uninitialized (`Self::new`/`Self::default`) tracker
    /// always wins outright — matches both source call sites' own
    /// `if initialized { blend } else { sample }` cold-start rule,
    /// avoiding a synthetic warm-up period biased toward whatever value
    /// the struct happened to start at.
    pub fn update(&mut self, sample: f64, alpha: f64) -> f64 {
        self.value = if self.initialized { self.value * (1.0 - alpha) + sample * alpha } else { sample };
        self.initialized = true;
        self.value
    }

    /// Current smoothed value (`0.0` if never updated and not seeded).
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Whether at least one [`EmaF64::update`] has run, or the tracker
    /// was constructed via [`EmaF64::seeded`].
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Named-stage, per-frame profiler with independent EMA smoothing per
/// stage — the generic mechanism behind the source app's own
/// `FrameProfile`. Stage names are caller-supplied `&'static str` keys
/// (e.g. `"grid"`, `"scene_build"`, `"overlay"`), not a fixed enum, so
/// any app's own frame-phase vocabulary ports over unchanged.
///
/// Clock-free by design (module doc) — the caller measures each stage
/// with whatever monotonic clock its platform has and feeds the elapsed
/// milliseconds into [`FrameProfiler::record_ms`].
#[derive(Debug, Clone)]
pub struct FrameProfiler {
    alpha: f64,
    stages: HashMap<&'static str, EmaF64>,
    frames: u64,
}

impl FrameProfiler {
    /// The source app's own smoothing constant
    /// (`previous * 0.92 + sample * 0.08`).
    pub const DEFAULT_ALPHA: f64 = 0.08;

    /// A profiler smoothing every stage at `alpha` (see [`EmaF64::update`]).
    pub fn new(alpha: f64) -> Self {
        Self { alpha, stages: HashMap::new(), frames: 0 }
    }

    /// Record a caller-measured sample in milliseconds for `stage`,
    /// folding it into that stage's own running EMA — the stage's
    /// tracker is created (uninitialized) on first use, so its own first
    /// sample wins outright rather than blending against a synthetic
    /// `0.0`.
    pub fn record_ms(&mut self, stage: &'static str, sample_ms: f64) {
        self.stages.entry(stage).or_default().update(sample_ms, self.alpha);
    }

    /// Current smoothed value (ms) for `stage`, or `0.0` if it has never
    /// been recorded.
    pub fn stage_ms(&self, stage: &str) -> f64 {
        self.stages.get(stage).map_or(0.0, EmaF64::value)
    }

    /// Every currently-tracked stage's smoothed value — iteration order
    /// is NOT guaranteed (backed by a `HashMap`); a caller that needs a
    /// fixed field order should read [`FrameProfiler::stage_ms`] per
    /// named stage instead.
    pub fn stages(&self) -> impl Iterator<Item = (&'static str, f64)> + '_ {
        self.stages.iter().map(|(&name, ema)| (name, ema.value()))
    }

    /// Total frames [`FrameProfiler::end_frame`] has been called for.
    pub fn frame_count(&self) -> u64 {
        self.frames
    }

    /// Bump the frame counter — call once per frame (typically after
    /// recording that frame's stages), independent of how many stages
    /// were actually recorded that frame.
    pub fn end_frame(&mut self) {
        self.frames = self.frames.wrapping_add(1);
    }

    /// Serialize every currently-tracked stage's smoothed value (ms) plus
    /// the running frame count into one JSON object — ready for an app's
    /// own `crate::layout::agent::BlackboxAgentSurface::agent_state()` to
    /// insert under whatever key it likes (e.g. `"frame_profile_ema_ms"`,
    /// mirroring the source app's own field), without hand-rolling a
    /// struct-of-`f64`-fields-plus-EMA-plus-JSON block per app.
    pub fn to_json(&self) -> serde_json::Value {
        let mut stages = serde_json::Map::new();
        for (name, ms) in self.stages() {
            stages.insert(name.to_owned(), serde_json::json!(ms));
        }
        serde_json::json!({ "frames": self.frames, "stages": stages })
    }
}

impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new(Self::DEFAULT_ALPHA)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_first_sample_wins_outright_when_uninitialized() {
        let mut ema = EmaF64::new();
        assert!(!ema.is_initialized());
        assert_eq!(ema.update(42.0, 0.1), 42.0);
        assert!(ema.is_initialized());
    }

    #[test]
    fn ema_blends_subsequent_samples_by_alpha() {
        let mut ema = EmaF64::new();
        ema.update(100.0, 0.1);
        let next = ema.update(0.0, 0.1);
        assert!((next - 90.0).abs() < 1e-9, "expected 100*0.9 + 0*0.1 = 90, got {next}");
    }

    #[test]
    fn ema_seeded_blends_the_seed_on_the_very_first_real_sample() {
        let mut ema = EmaF64::seeded(60.0);
        assert!(ema.is_initialized());
        let next = ema.update(120.0, 0.1);
        assert!((next - (60.0 * 0.9 + 120.0 * 0.1)).abs() < 1e-9);
    }

    #[test]
    fn ema_converges_toward_a_sustained_constant_sample() {
        let mut ema = EmaF64::new();
        for _ in 0..500 {
            ema.update(16.0, 0.2);
        }
        assert!((ema.value() - 16.0).abs() < 1e-6, "a sustained constant sample must converge, got {}", ema.value());
    }

    #[test]
    fn frame_profiler_default_alpha_matches_the_source_apps_own_smoothing_constant() {
        assert_eq!(FrameProfiler::DEFAULT_ALPHA, 0.08);
        let profiler = FrameProfiler::default();
        assert_eq!(profiler.frame_count(), 0);
    }

    #[test]
    fn frame_profiler_record_ms_creates_and_smooths_a_named_stage() {
        let mut profiler = FrameProfiler::new(0.08);
        profiler.record_ms("grid", 10.0);
        assert_eq!(profiler.stage_ms("grid"), 10.0, "first sample must win outright");
        let smoothed = {
            profiler.record_ms("grid", 20.0);
            profiler.stage_ms("grid")
        };
        assert!((smoothed - (10.0 * 0.92 + 20.0 * 0.08)).abs() < 1e-9);
    }

    #[test]
    fn frame_profiler_stage_ms_returns_zero_for_a_never_recorded_stage() {
        let profiler = FrameProfiler::new(0.08);
        assert_eq!(profiler.stage_ms("never_recorded"), 0.0);
    }

    #[test]
    fn frame_profiler_end_frame_increments_the_frame_counter_independent_of_stage_recording() {
        let mut profiler = FrameProfiler::new(0.08);
        profiler.end_frame();
        profiler.record_ms("grid", 1.0);
        profiler.end_frame();
        assert_eq!(profiler.frame_count(), 2);
    }

    #[test]
    fn frame_profiler_stages_iterates_every_recorded_stage() {
        let mut profiler = FrameProfiler::new(0.08);
        profiler.record_ms("grid", 1.0);
        profiler.record_ms("overlay", 2.0);
        let mut names: Vec<&str> = profiler.stages().map(|(name, _)| name).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["grid", "overlay"]);
    }

    #[test]
    fn frame_profiler_to_json_reports_every_recorded_stage_and_the_frame_count() {
        let mut profiler = FrameProfiler::new(0.08);
        profiler.record_ms("grid", 5.0);
        profiler.end_frame();
        let json = profiler.to_json();
        assert_eq!(json["frames"], 1);
        assert_eq!(json["stages"]["grid"], 5.0);
    }
}
