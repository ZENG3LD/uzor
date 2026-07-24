//! Top-level CPU backend — walks `Scene::commands`, dispatches to
//! per-primitive rasterisers, emits metrics.

use std::time::Instant;

use uzor_urx_core::math::{Affine, Brush, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_core::validate::{validate_command, ValidationIssue};

/// Predicate: is this scene eligible for the tile pipeline?
/// Tile path supports: FillRect (Solid brush only, no radii, axis-aligned
/// transform with finite coeffs and finite rect), PushClipRect, PopClip.
/// Anything else (incl. ANY non-finite coordinate) forces the scanline
/// fallback — the scanline path itself silently skips non-finite cmds.
fn tile_eligible(scene: &Scene) -> bool {
    for cmd in &scene.commands {
        match cmd {
            DrawCommand::FillRect { rect, radii, brush, transform } => {
                if !uzor_urx_core::validate::is_finite_rect(*rect)
                    || !uzor_urx_core::validate::is_finite_affine(*transform)
                    || !uzor_urx_core::validate::is_finite_radii_opt(radii)
                {
                    return false;
                }
                if let Some(r) = radii {
                    if r.iter().any(|v| *v > 0.0) { return false; }
                }
                if !matches!(brush, Brush::Solid(_)) { return false; }
                let c = transform.as_coeffs();
                if c[1].abs() > 1e-6 || c[2].abs() > 1e-6 { return false; }
            }
            DrawCommand::PushClipRect { rect, transform } => {
                if !uzor_urx_core::validate::is_finite_rect(*rect)
                    || !uzor_urx_core::validate::is_finite_affine(*transform)
                {
                    return false;
                }
                let c = transform.as_coeffs();
                if c[1].abs() > 1e-6 || c[2].abs() > 1e-6 { return false; }
            }
            DrawCommand::PopClip => {}
            _ => return false,
        }
    }
    true
}

use crate::blend::{LayerStack, PopOutcome};
use crate::clip::ClipStack;
use crate::color::brush_to_color;
use crate::fill::fill_rect_aa;
use crate::pixmap::Pixmap;
use crate::stroke::{stroke_line_aa, stroke_rect_aa};

#[derive(Debug, Clone, Copy)]
pub enum RenderError {
    /// `Scene::commands` had unbalanced PushClip/PopClip — too many pops.
    ClipUnderflow,
    /// `render_parallel` cannot handle the scene because it contains
    /// primitives that need shared per-scene state (paths, glyphs, images).
    /// Caller should drop back to the sequential `render()` entry point.
    /// Carries the (zero-indexed) command position so callers can locate
    /// the offending primitive.
    ParallelUnsupported(usize),
}

/// The CPU backend. Holds a [`UrxConfig`] (cheap to clone — plain
/// data, ~120 B). Per-region caches still live one layer up in the
/// engine, so `CpuBackend` itself stays `Send + Sync` for the future
/// rayon-per-region path.
///
/// Construct with [`CpuBackend::new`] for default tuning (matches
/// pre-config 1.4.1 behaviour byte-for-byte) or
/// [`CpuBackend::with_config`] to tune.
#[derive(Debug, Default, Clone)]
pub struct CpuBackend {
    pub(crate) config: uzor_urx_core::config::UrxConfig,
}

impl CpuBackend {
    /// Backend with default config. Output identical to 1.4.1 — no
    /// constant changed values when this knob was introduced.
    pub fn new() -> Self { Self::default() }

    /// Backend with consumer-supplied config. Caller is responsible
    /// for `cfg.validate()` — but we also assert on construction so
    /// bad configs fail fast.
    pub fn with_config(cfg: uzor_urx_core::config::UrxConfig) -> Self {
        cfg.validate().expect("invalid UrxConfig");
        Self { config: cfg }
    }

    /// Read the backend's config (mostly useful for tests + benches).
    pub fn config(&self) -> &uzor_urx_core::config::UrxConfig { &self.config }

    /// Render a whole scene into a pixmap. Does NOT clear the pixmap
    /// first — caller decides background fill (gives us a free
    /// `LoadOp::Load` equivalent for dirty-rect re-paint).
    ///
    /// **Auto-routing**: if the scene contains ONLY FillRect commands
    /// with simple brushes (Solid) and axis-aligned transforms, AND
    /// command count ≥ `config.tile_route_min_cmds`, the tile pipeline
    /// is used (bumpalo arena + rayon parallel band flush +
    /// bg-replacement). Otherwise falls through to the per-primitive
    /// scanline backend.
    pub fn render(&self, scene: &Scene, pixmap: &mut Pixmap) -> Result<(), RenderError> {
        // `uzor_urx_glyph` is an OPTIONAL dep (Cargo.toml `glyph =
        // ["dep:uzor-urx-glyph"]`) — resolving the configured LUT must
        // stay behind the same feature gate the `GlyphRun` draw arm
        // itself uses, or a `glyph`-feature-less build fails to link.
        #[cfg(feature = "glyph")]
        let gamma_lut = self.config.text_gamma_enabled.then(uzor_urx_glyph::configured_text_gamma_lut);
        #[cfg(not(feature = "glyph"))]
        let gamma_lut = None;
        self.render_with_gamma_lut(scene, pixmap, gamma_lut)
    }

    /// Same as [`Self::render`], but with the glyph gamma LUT supplied
    /// explicitly instead of resolved from `self.config` — lets the URX
    /// text-gamma calibration sweep
    /// (`uzor-examples/src/l3/dashboard.rs`'s `text_gamma_calibration`
    /// test module, design §3.1) try an ARBITRARY candidate curve in one
    /// process without touching the production `TEXT_GAMMA_CURVE`
    /// constant or `UrxConfig::text_gamma_enabled`. `#[doc(hidden)]` —
    /// a calibration-tooling entry point, not part of the stable public
    /// API surface, same convention as `uzor_urx_glyph::_clear_caches_for_tests`.
    #[doc(hidden)]
    pub fn render_with_gamma_lut_for_test(
        &self,
        scene: &Scene,
        pixmap: &mut Pixmap,
        gamma_lut: Option<&uzor_urx_core::text_gamma::TextGammaLut>,
    ) -> Result<(), RenderError> {
        self.render_with_gamma_lut(scene, pixmap, gamma_lut)
    }

    fn render_with_gamma_lut(
        &self,
        scene: &Scene,
        pixmap: &mut Pixmap,
        gamma_lut: Option<&uzor_urx_core::text_gamma::TextGammaLut>,
    ) -> Result<(), RenderError> {
        // Only the `GlyphRun` arm's `#[cfg(feature = "glyph")]` body
        // reads `gamma_lut` — without that feature it's genuinely
        // unused for this whole function, same as `glyphs`/`font`/etc
        // in that arm's own `#[cfg(not(feature = "glyph"))]` branch.
        #[cfg(not(feature = "glyph"))]
        let _ = gamma_lut;

        use uzor_urx_core::metrics_keys::{
            render_submit_us_key, render_submit_count_key,
            KEY_TICK_SUBMIT_US, KEY_TICK_FRAMES,
            KEY_RENDER_PRIMITIVES,
            KEY_RENDER_SKIPPED_NONFINITE,
        };

        let t0 = Instant::now();

        if scene.commands.len() >= self.config.tile_route_min_cmds
            && tile_eligible(scene)
        {
            crate::tile::render_tiled_with_config(scene, pixmap, &self.config);
            let elapsed_us = t0.elapsed().as_micros() as u64;
            metrics::histogram!(KEY_TICK_SUBMIT_US).record(elapsed_us as f64);
            metrics::counter!(KEY_TICK_FRAMES).increment(1);
            metrics::histogram!(render_submit_us_key("urx_cpu_tile")).record(elapsed_us as f64);
            metrics::counter!(render_submit_count_key("urx_cpu_tile")).increment(1);
            metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
            return Ok(());
        }

        let bounds = Rect::new(0.0, 0.0, pixmap.width() as f64, pixmap.height() as f64);
        let mut clip = ClipStack::new(bounds);
        // Wave 3 Commit 1: real offscreen-pixmap blend-layer isolation
        // (closes `cpu_blend_layer_dropped`, design §5/§6.2). `clip` is
        // completely UNCHANGED by layers — one shared clip stack spans
        // layer boundaries (design §0.2); only the DRAW TARGET switches
        // via `layer_stack.current_target(pixmap)` below. Every
        // per-primitive rasteriser here takes `pixmap` purely as its
        // write target (verified by direct read of every arm's
        // callee signature) — none re-derives bounds from it, so
        // redirecting the target to a same-viewport-sized layer pixmap
        // (§0.2) is a safe, purely mechanical substitution.
        let mut layer_stack = LayerStack::new(self.config.blend_layer_max_depth);

        for cmd in &scene.commands {
            // Non-finite input (NaN, ±Inf in any coord/transform) is
            // an upstream bug — silently skip + counter rather than
            // panic or corrupt the pixmap. Degenerate geometry passes
            // through (existing per-primitive code handles zero-area
            // rejection).
            if let Err(ValidationIssue::NonFinite) = validate_command(cmd) {
                metrics::counter!(KEY_RENDER_SKIPPED_NONFINITE).increment(1);
                continue;
            }
            match cmd {
                DrawCommand::FillRect { rect, radii, brush, transform } => {
                    // Corner radii → push a transient rounded clip,
                    // draw the rect, pop. Same path the consumer would
                    // have to write by hand otherwise.
                    let _radii_guard = if let Some(r) = radii {
                        if r.iter().any(|v| *v > 0.0) {
                            let rr = uzor_urx_core::math::RoundedRect::from_rect(
                                *rect,
                                uzor_urx_core::math::RoundedRectRadii::new(
                                    r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64,
                                ),
                            );
                            clip.push_rounded_rect(rr, transform);
                            true
                        } else { false }
                    } else { false };
                    let target = layer_stack.current_target(pixmap);
                    if matches!(brush, uzor_urx_core::math::Brush::Gradient(_)) {
                        if crate::gradient::try_fill_rect_gradient(target, &clip, *rect, brush, transform).is_some() {
                            if _radii_guard { clip.pop(); }
                            continue;
                        }
                    }
                    let color = brush_to_color(brush);
                    fill_rect_aa(target, &clip, *rect, color, transform);
                    if _radii_guard { clip.pop(); }
                }
                DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    let target = layer_stack.current_target(pixmap);
                    if let Some(r) = radii {
                        if r.iter().any(|v| *v > 0.0) {
                            // Round-corner stroke = stroke a flattened
                            // rounded path (uses scanline + capsules).
                            let rr = uzor_urx_core::math::RoundedRect::from_rect(
                                *rect,
                                uzor_urx_core::math::RoundedRectRadii::new(
                                    r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64,
                                ),
                            );
                            use kurbo::Shape as _;
                            let path: uzor_urx_core::math::BezPath = rr.into_path(0.25);
                            crate::path::stroke_path_aa(target, &clip, &path, stroke, color, transform);
                            continue;
                        }
                    }
                    stroke_rect_aa(target, &clip, *rect, stroke.width, color, transform);
                }
                DrawCommand::Line { from, to, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    let target = layer_stack.current_target(pixmap);
                    stroke_line_aa(target, &clip, *from, *to, stroke.width, color, transform);
                }
                DrawCommand::FillPath { path, rule, brush, transform } => {
                    let target = layer_stack.current_target(pixmap);
                    // Coordinator's 2026-07-25 fix: a real gradient now
                    // renders on `FillPath`, mirroring `FillRect`'s own
                    // `try_fill_rect_gradient` dispatch pattern above —
                    // see `crate::gradient::GradientSampler`'s doc
                    // comment for why every OTHER primitive still falls
                    // back to `brush_to_color`'s first-stop colour.
                    if matches!(brush, uzor_urx_core::math::Brush::Gradient(_)) {
                        if crate::gradient::try_fill_path_gradient(target, &clip, path, *rule, brush, transform).is_some() {
                            continue;
                        }
                    }
                    let color = brush_to_color(brush);
                    crate::path::fill_path_aa(target, &clip, path, *rule, color, transform);
                }
                DrawCommand::StrokePath { path, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    let target = layer_stack.current_target(pixmap);
                    crate::path::stroke_path_aa(target, &clip, path, stroke, color, transform);
                }
                DrawCommand::GlyphRun { glyphs, font, font_size, brush, transform, text: _ } => {
                    #[cfg(feature = "glyph")]
                    {
                        // peniko 0.6: byte channels via `to_rgba8()` (the
                        // old direct `r/g/b/a` fields are gone) — this arm
                        // had bit-rotted unnoticed because no consumer
                        // enabled the `glyph` feature until the URX
                        // family-parity Wave 0 (2026-07-24).
                        let rgba = brush_to_color(brush).to_rgba8();
                        let coeffs = transform.as_coeffs();
                        let (tx, ty) = (coeffs[4] as f32, coeffs[5] as f32);
                        let target = layer_stack.current_target(pixmap);
                        let pw = target.width();
                        let ph = target.height();
                        // URX text-gamma design, 2026-07-26, §2.4 — the
                        // LUT is resolved by the caller (either from
                        // config via `render()`, or an explicit override
                        // via `render_with_gamma_lut_for_test`, see
                        // `render_with_gamma_lut`'s own doc comment);
                        // `None` is `draw_glyph_run`'s zero-added-cost
                        // path.
                        //
                        // Clip fix (2026-07-24): `GlyphRun` was the ONLY
                        // primitive in this match that never threaded
                        // `&clip` through — every other arm
                        // (FillRect/StrokeRect/Line/FillPath/StrokePath/
                        // Image) already does. Same two-tier shape as
                        // `fill_rect_aa`'s own `use_mask = !clip.all_rect()`
                        // split: `bounds` (from `clip.current()`) is a
                        // cheap per-pixel bbox test that alone is exact
                        // for the common plain-rect-clip case; `mask` (a
                        // closure over `clip.pixel_coverage`) only gets
                        // built — and only gets called inside
                        // `draw_glyph_run`'s own pixel loop — when a
                        // rounded clip is actually on the stack.
                        let clip_current = clip.current();
                        let clip_bounds = (
                            clip_current.x0 as f32,
                            clip_current.y0 as f32,
                            clip_current.x1 as f32,
                            clip_current.y1 as f32,
                        );
                        let clip_sampler = |px: i64, py: i64| clip.pixel_coverage(px, py);
                        let glyph_clip = uzor_urx_glyph::GlyphClip {
                            bounds: clip_bounds,
                            mask: (!clip.all_rect()).then_some(&clip_sampler as &dyn Fn(i64, i64) -> u8),
                        };
                        let _ = uzor_urx_glyph::draw_glyph_run(
                            target.pixels_mut(),
                            pw, ph,
                            tx, ty,
                            glyphs,
                            *font,
                            *font_size,
                            [rgba.r, rgba.g, rgba.b, rgba.a],
                            gamma_lut,
                            Some(glyph_clip),
                        );
                    }
                    #[cfg(not(feature = "glyph"))]
                    {
                        let _ = (glyphs, font, font_size, brush, transform);
                        metrics::counter!(
                            uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                            "kind" => "glyph_run_skipped_no_feature",
                        ).increment(1);
                    }
                }
                DrawCommand::Image { src, src_rect, dest, transform } => {
                    let target = layer_stack.current_target(pixmap);
                    let _ = crate::image_draw::draw_image_aa(
                        target, &clip, *src, *src_rect, *dest, transform,
                    );
                }
                DrawCommand::PushClipRect { rect, transform } => {
                    clip.push_rect(*rect, transform);
                }
                DrawCommand::PushClipRoundedRect { rect, transform } => {
                    clip.push_rounded_rect(*rect, transform);
                }
                DrawCommand::PopClip => {
                    clip.pop();
                }
                DrawCommand::PushBlendLayer { mode, alpha, transform } => {
                    // `Affine` derives `PartialEq` in the pinned kurbo
                    // 0.13 (confirmed by reading `kurbo::Affine`'s own
                    // struct definition — a plain `!=` works directly,
                    // no `as_coeffs()` dance needed).
                    if *transform != Affine::IDENTITY {
                        metrics::counter!(
                            KEY_RENDER_PRIMITIVES,
                            "kind" => "cpu_blend_layer_transform_ignored",
                        ).increment(1);
                    }
                    if !layer_stack.push(*mode, *alpha, pixmap.width(), pixmap.height()) {
                        metrics::counter!(
                            KEY_RENDER_PRIMITIVES,
                            "kind" => "cpu_blend_layer_depth_exceeded",
                        ).increment(1);
                    }
                }
                DrawCommand::PopBlendLayer => {
                    // `pixmap` here is the TRUE ROOT binding, never a
                    // redirected `current_target` — `LayerStack::pop`
                    // resolves its own parent internally (see
                    // `blend.rs`'s module doc for why: the design
                    // sketch's `layer_stack.pop(layer_stack.current_target_parent(pixmap))`
                    // double-borrows `layer_stack`, genuinely uncompilable).
                    match layer_stack.pop(pixmap) {
                        PopOutcome::Composited | PopOutcome::Suppressed => {}
                        PopOutcome::Underflow => {
                            metrics::counter!(
                                KEY_RENDER_PRIMITIVES,
                                "kind" => "cpu_blend_layer_pop_underflow",
                            ).increment(1);
                        }
                    }
                }
            }
        }

        // Unbalanced-scene guard (design §3.3's GPU-side
        // `native_blend_layer_force_closed_at_scene_end`, mirrored here)
        // — any layer still open at this point had no matching
        // `PopBlendLayer`. Force-composite each one (LIFO) onto its
        // parent so its content stays visible instead of being silently
        // dropped along with `layer_stack` at the end of this function.
        let force_closed = layer_stack.force_close_all(pixmap);
        if force_closed > 0 {
            metrics::counter!(
                KEY_RENDER_PRIMITIVES,
                "kind" => "cpu_blend_layer_force_closed_at_scene_end",
            ).increment(force_closed as u64);
        }

        let elapsed_us = t0.elapsed().as_micros() as u64;
        metrics::histogram!(KEY_TICK_SUBMIT_US).record(elapsed_us as f64);
        metrics::counter!(KEY_TICK_FRAMES).increment(1);
        metrics::histogram!(render_submit_us_key("urx_cpu")).record(elapsed_us as f64);
        metrics::counter!(render_submit_count_key("urx_cpu")).increment(1);
        metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
        Ok(())
    }
}
