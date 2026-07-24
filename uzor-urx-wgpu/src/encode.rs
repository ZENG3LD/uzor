//! `Scene` → native-pipeline instance encoder.
//!
//! Wave 1 Commit 1 covered `FillRect`/`StrokeRect` (`Brush::Solid` —
//! gradient/image degrade to a first-stop/transparent solid, counted).
//! Commit 2 added `Line` and the painter's-order batching machinery
//! (`Batch`/`BatchKind`) that keeps Quad/Line draws interleaved
//! correctly. Commit 3 added `FillPath`/`StrokePath` (lyon
//! tessellation via `TessCache`) and routes
//! `FillRect{Brush::Gradient(Linear), radii}` through the same
//! tessellated-triangle path with real per-vertex gradient colour
//! (design §5) — the ONLY `FillRect` gradient kind that gets full
//! support in Wave 1; `Radial`/`Sweep` still degrade to solid, and
//! gradients on `Line`/`FillPath`/`StrokePath` still degrade to
//! first-stop solid (design: "only FillRect gets gradients in Wave
//! 1"). Commit 4 added the real `ClipStack` (`PushClipRect`/`PopClip`
//! feed every instance's `clip_rect` field; `PushClipRoundedRect`
//! degrades to its bounding rect) and finalized the degrade-label
//! inventory for every `DrawCommand` variant this crate didn't render
//! natively yet (`GlyphRun`, `Image` — Wave 4, blend layers — Wave 3).
//!
//! **Wave 2 Commit 2** closes `GlyphRun` (`encode_glyph_run`,
//! `docs/uzor-engines/plans/urx-wave2-native-glyph-atlas-design-2026-07-25.md`
//! §6) — per-glyph subpixel-binned atlas lookup via
//! `crate::atlas::NativeGlyphAtlas`, rasterised through
//! `uzor_urx_glyph::rasterise_glyph` (the SAME rasteriser
//! `uzor-urx-cpu`'s `GlyphRun` arm uses). `encode_scene` gains an
//! `atlas: Option<&mut NativeGlyphAtlas>` parameter — see that
//! function's doc comment for why it's `Option` rather than the
//! design's literal `&mut` (a deliberate, disclosed deviation).
//!
//! **Wave 3 Commit 2** closed `PushClipRoundedRect` (real stencil clip,
//! `ClipStack`'s `Rounded` frame variant, `MaskOp`-tagged batches) —
//! see that commit's history in `ClipStack`'s own doc comment below.
//!
//! **Wave 3 Commit 3** closes `PushBlendLayer`/`PopBlendLayer`
//! (`docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
//! §3) — `EncodedFrame::batches: Vec<Batch>` becomes `ops: Vec<FrameOp>`
//! (`Draw(Batch)` plus `PushLayer`/`PopLayer` markers bracketing a
//! blend-layer scope exactly where the Push/Pop occurred in the scan).
//! An encode-time `LayerStack` (parallel to, and fully independent of,
//! `ClipStack` — Push/PopBlendLayer never resets or nests the clip
//! stack, design §0.2) tracks nesting depth against
//! `UrxConfig::blend_layer_max_depth` (threaded in as a plain `usize`
//! parameter, consistent with `tess_cache`/`atlas`'s already-resolved-
//! value style — not the whole config struct). The multi-pass EXECUTOR
//! that actually opens/closes render passes per `FrameOp::PushLayer`/
//! `PopLayer` lives in `renderer.rs`; this module only produces the op
//! list, oblivious to how many passes it costs.
//!
//! Every unimplemented `DrawCommand` variant is a counted, never-silent
//! degrade via `uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES`
//! with a `native_*` `kind` tag.
//!
//! **Wave 4 Commit 4** (design §0.2/§5/§6) closes the two structural
//! gaps the "Tessellation-cache transform semantics" section right
//! below used to document as open: (1) stroke width is now
//! DEVICE-CONSTANT under scale on every mechanism (Quad SDF, Line SDF,
//! Path tessellation via `tess_stroke_scaled`'s local-width pre-scale),
//! matching CPU exactly; (2) `project_local`'s mesh reprojection is now
//! the FULL 6-coefficient affine, not translate+scale-only — closing
//! the last asymmetry `transform_gradient_params`'s own doc comment
//! used to flag. `decompose_similarity` (introduced Commit 3 for
//! `encode_image`) now ALSO drives `FillRect`/`StrokeRect`'s own
//! Quad-SDF-vs-Triangle-pipeline routing decision (§5.2): a similarity
//! transform with uniform radii stays on the fast rotation-capable Quad
//! SDF (`_pad0[0]` now carries the rotation angle, §5.3); a genuine
//! shear/reflection OR non-uniform per-corner radii routes through the
//! Triangle pipeline instead (§6.2), closing
//! `native_per_corner_radii_uniform_approx` entirely. The
//! curve-flattening-tolerance-under-scale limitation below is
//! UNCHANGED by this commit — still open, still disclosed, not fixed.
//!
//! **Wave 4 CLOSED at Commits 5+6** (design §9/§10) — the parity
//! harness (`uzor-urx-wgpu/tests/{fixtures,parity}.rs`) gained 6 new
//! CPU-vs-GPU cases (Radial/Sweep gradients, per-corner radii, a
//! rotated rect, a scaled-stroke-width regression proof, rotated
//! images) plus 3 GPU-only correctness tests (rotated-rect corners,
//! Commit 4's own `renderer.rs` test; a sheared rect; a gradient on
//! `StrokePath`) — measurement surfaced three genuinely new,
//! previously-undocumented CPU-side findings along the way, all
//! disclosed in their respective fixtures' doc comments rather than
//! silently worked around: (1) CPU's rounded-clip MASK mis-places
//! itself under a genuine rotation (worse than the already-documented
//! bbox-approximation, §0.3); (2) CPU never renders a real gradient on
//! anything but `FillRect`; (3) an ODD stroke width recreates the
//! integer-grid-alignment worst case for the border BAND's own derived
//! edges even under the established `.5`-offset rect convention. None
//! of these are `uzor-urx-cpu` bugs this wave fixes (off-limits) — each
//! is either avoided at the fixture level or (case 2) moved to a
//! GPU-only correctness test instead of a CPU comparison that could
//! never meaningfully pass.
//!
//! ## Tessellation-cache transform semantics — honest write-up (design
//! item 3 investigation, Wave 1; stroke-width UPDATE Wave 4 Commit 4)
//!
//! `uzor-urx-cpu::path::stroke_path_aa` transforms the PATH POINTS
//! through the full affine (translation/scale/rotation/shear all
//! honoured, `path.rs:261,272,277`) but then strokes with
//! `stroke.width` **used exactly as given, never scaled by the
//! transform** (`path.rs:299` `let half: f64 = stroke.width as f64 *
//! 0.5;` — no `sx`/`sy` factor anywhere in that function). CPU's
//! stroke width is therefore a DEVICE-space constant, independent of
//! the transform's scale.
//!
//! This crate's `TessCache` (design §6) tessellates in LOCAL
//! (pre-transform) space. Through Wave 1-3, `stroke.width` was fed to
//! lyon as a raw LOCAL geometric quantity, so the NATIVE pipeline's
//! apparent on-screen stroke width scaled with the transform while
//! CPU's stayed constant — a genuine, structural semantic difference,
//! inert only because every fixture through Wave 3 used
//! `Affine::IDENTITY`. **Wave 4 Commit 4 closes this**: `tess_cache`
//! callers now pre-divide `stroke.width` by the transform's own average
//! scale magnitude before tessellating (`tess_stroke_scaled`), so the
//! reprojected mesh's on-screen width comes out device-constant,
//! matching CPU exactly (the Quad-SDF/Line-SDF mechanisms simply stopped
//! multiplying by scale at all, being analytic rather than
//! cache-based). The curve-flattening-tolerance gap is UNCHANGED and
//! still open: CPU adapts its flatten tolerance to the transform's
//! scale (`path.rs::screen_flatten_tolerance`) so a zoomed-in curve
//! stays smooth; this crate tessellates once at a fixed LOCAL tolerance
//! (`tessellate::TESS_TOLERANCE_PX`), so a heavily-scaled-up cached mesh
//! could look faceted on screen — a real, currently-untested (no
//! fixture zooms a cached path enough to notice) architecture limitation
//! of the local-space caching design, explicitly NOT fixed this wave
//! (design §5.4).
//!
//! ## Gradient-routing finding (design item 5 investigation)
//!
//! Direct read of `uzor-urx-cpu::backend.rs`'s `FillRect` arm shows
//! that CPU's `Brush::Gradient` branch calls
//! `gradient::try_fill_rect_gradient` → `fill_rect_gradient_aa`, which
//! iterates pixels bounded ONLY by `clip.current()` (the clip stack's
//! bounding-box intersection) and never calls
//! `clip.pixel_coverage()` — i.e. it never consults the per-pixel
//! rounded-clip MASK that `radii: Some(...)` pushes for the SOLID fill
//! path (`fill_rect_aa`'s `use_mask` branch). **CPU silently ignores
//! `radii` for gradient fills** — a genuine, pre-existing gap (out of
//! scope to fix; `uzor-urx-cpu` is off-limits) that only affects the
//! gradient code path (solid `FillRect`+radii still rounds correctly
//! on CPU). This native pipeline renders the gradient through a REAL
//! `RoundedRect` tessellation (correctly rounded corners), so
//! `linear_gradient_rect`'s corner regions are an EXPECTED, honestly-
//! documented mismatch between the two backends — sized (fixture rect/
//! radius choice) to stay inside the design §7 tolerance budget, same
//! engineering call as `solid_rects_with_radii` in Commit 1.
//!
//! Separately: CPU's `fill_rect_gradient_aa` uses the gradient axis
//! (`LinearGradientPosition.start/end`) DIRECTLY against screen-space
//! pixel coordinates — it never applies `transform` to the gradient
//! axis at all (only to `rect`). This native pipeline applies the SAME
//! transform (translate+scale through Wave 3; the FULL affine as of
//! Wave 4 Commit 4, §5.4) to both the rect's tessellated mesh AND the
//! gradient axis, which is more correct under a non-identity transform
//! but is, again, a difference from CPU's literal behaviour — invisible
//! at `Affine::IDENTITY` (every Wave 1-3 fixture).
//!
//! Stop interpolation: CPU builds a dense 256-entry LUT by lerping
//! PREMULTIPLIED sRGB stop bytes per-pixel
//! (`uzor-urx-cpu/src/gradient.rs`'s module doc: "interpolate in
//! linear-premul space"). This pipeline evaluates STRAIGHT
//! (non-premultiplied) colour at each tessellation VERTEX only and
//! lets the rasteriser's barycentric interpolation carry it across
//! each triangle, premultiplying once in the fragment shader — the
//! same "straight-in, premultiply-in-shader" convention Quad/Line
//! already use. Straight-lerp and premultiplied-lerp are
//! mathematically IDENTICAL when alpha is constant across the
//! gradient (premultiplied = straight × a constant, and lerp commutes
//! with constant scaling) — `linear_gradient_rect`'s fixture therefore
//! uses two fully-opaque stops, sidestepping this real algorithmic
//! difference cleanly rather than hoping the tolerance budget absorbs
//! it. A future gradient with varying alpha would need this
//! documented and probed more carefully (design risk 5 already
//! anticipates exactly this).

use kurbo::Shape as _;
use peniko::LinearGradientPosition;
use uzor_urx_core::math::{
    Affine, BezPath, BlendMode, Brush, Color, ColorStops, Compose, Extend, Gradient, GradientKind, Mix, Point, Rect,
    RoundedRect, RoundedRectRadii, Vec2,
};
use uzor_urx_core::metrics_keys::{KEY_RENDER_GLYPH_INSTANCES, KEY_RENDER_PRIMITIVES, KEY_RENDER_SKIPPED_NONFINITE};
use uzor_urx_core::scene::{DrawCommand, FillRule, FontId, Glyph, ImageId, LineCap, Scene, Stroke};
use uzor_urx_core::validate::{validate_command, ValidationIssue};

use std::sync::Arc;

use crate::atlas::NativeGlyphAtlas;
use crate::gradient_lut::GradientLutAtlas;
use crate::pipelines::blend_composite::BlendCompositeInstance;
use crate::pipelines::glyph::GlyphInstance;
use crate::pipelines::gradient::GradientInstance;
use crate::pipelines::image::ImageInstance;
use crate::pipelines::line::LineInstance;
use crate::pipelines::path::TriInstance;
use crate::pipelines::quad::{pack_rgba8, QuadInstance};
use crate::renderer::Viewport;
use crate::tessellate::{TessCache, TessMesh};

/// Which of the two stencil mask-write pipelines (`stencil_mask.rs`) a
/// `BatchKind::StencilMask` batch replays through — the ONLY
/// difference between the two is `pass_op` (design §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskOp {
    Increment,
    Decrement,
}

/// Which pipeline a [`Batch`] replays through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchKind {
    Quad,
    Line,
    Triangle,
    Glyph,
    /// Wave 3 Commit 2 (design §2/§4) — mask-write geometry, indexes
    /// into `EncodedFrame::stencil_masks`, NOT `triangles` (a separate
    /// buffer — mask writes and content triangles never share one
    /// vertex buffer, since they bind different pipelines).
    StencilMask(MaskOp),
    /// Wave 4 Commit 3 (design §2) — Radial/Sweep gradient-mesh
    /// triangles, indexes into `EncodedFrame::gradients` (a SEPARATE
    /// buffer from `triangles`/`GradientInstance` is a different, wider
    /// layout — see that struct's own doc comment).
    Gradient,
    /// Wave 4 Commit 3 (design §4.3) — `DrawCommand::Image`, indexes
    /// into `EncodedFrame::images`. Carries the `ImageId` IN the batch
    /// key itself (unlike every other `BatchKind`, which needs no
    /// per-batch identity beyond its own variant) — a DIFFERENT
    /// `ImageId` never coalesces with another (each needs its OWN bind
    /// group bound at replay time), but CONSECUTIVE draws of the SAME
    /// `ImageId` still coalesce via `bump_batch`'s existing equality
    /// check (`BatchKind` derives `PartialEq`, so two `Image(id)`
    /// values compare equal exactly when `id` matches) — no new
    /// batching mechanism needed, mirrors Wave 3's blend-composite
    /// precedent ("composites are never coalesced with anything... each
    /// one binds a DIFFERENT layer's own resolve texture") for the
    /// cross-image case, while still batching same-image runs for free.
    Image(ImageId),
}

/// A contiguous run of same-kind, same-`stencil_ref` instances in
/// painter's-order scan order — `start`/`count` index into
/// `EncodedFrame::{quads,lines,triangles,glyphs,stencil_masks}`
/// depending on `kind`. Copied algorithm from legacy
/// `InstancedRenderer::render`'s Phase-1 scan
/// (`uzor-render-wgpu-instanced/src/renderer.rs:766-848`, design §5):
/// a new batch opens only when `kind` OR `stencil_ref` differs from the
/// previous one (Wave 3 Commit 2 extends the original kind-only rule —
/// `set_stencil_reference` is genuine per-draw-call pass state, unlike
/// the free per-instance `clip_rect` field, design risk 6), so
/// consecutive same-kind-AND-same-depth commands still coalesce into
/// one draw call while painter's order is preserved.
///
/// `stencil_ref` meaning (design item 4's wgpu-pass-compatibility
/// finding — see `encode_scene`'s doc comment for the full resolution):
/// for `Quad`/`Line`/`Triangle`/`Glyph` batches, `None` means "depth 0"
/// and `Some(d)` means "nested `d` rounded clips deep" — BOTH are
/// replayed with the `_test` pipeline variant, gated `Equal(0)`/
/// `Equal(d)`, whenever `EncodedFrame::has_rounded_clip` is true for
/// the WHOLE frame (the renderer's `_off`-vs-`_test` choice is a
/// frame-wide decision, never per-batch — mixing `_off` and `_test`
/// pipelines within ONE pass that has a stencil attachment is a wgpu
/// validation error, `RenderPassCompatibilityError::IncompatibleDepthStencilAttachment`,
/// confirmed by reading `wgpu-core-29.0.3/src/device/mod.rs`'s
/// `RenderPassContext::check_compatible`). For `StencilMask` batches,
/// `stencil_ref` is ALWAYS `Some(gate)` (the explicit gate value the
/// mask write itself tests against before mutating stencil) — never
/// `None`, since a mask write never has a "no stencil" state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Batch {
    pub(crate) kind: BatchKind,
    pub(crate) start: u32,
    pub(crate) count: u32,
    pub(crate) stencil_ref: Option<u32>,
}

/// Replaces `EncodedFrame::batches: Vec<Batch>` (Wave 1/2 shape) —
/// Wave 3 Commit 3, design §3.2. A `Batch` is now wrapped in
/// `FrameOp::Draw` alongside two new markers that bracket a blend-
/// layer scope exactly where `PushBlendLayer`/`PopBlendLayer` occurred
/// in the scan — `bump_batch`'s coalescing check (`last_mut()` must be
/// a `FrameOp::Draw` with a matching `(kind, stencil_ref)`) naturally
/// refuses to merge across a `PushLayer`/`PopLayer` marker, since the
/// "last op" is a marker, not a `Draw`, whenever one sits between two
/// otherwise-identical `Draw`s.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FrameOp {
    Draw(Batch),
    /// Emitted at a `PushBlendLayer` that didn't hit the depth cap.
    PushLayer { depth: u32 },
    /// Emitted at the matching `PopBlendLayer` (real or synthetic,
    /// end-of-scene force-close) — tells the executor "composite layer
    /// `depth`'s resolved content onto whatever is now the current
    /// target (parent layer or root), gated by `stencil_ref`." The
    /// composite's `alpha`/`clip_rect` (design §3.6: "baked from the
    /// clip active at the `PopBlendLayer` point") and `mode`'s already-
    /// applied degrade (`degrade_blend_mode`, design §0.3) do NOT need a
    /// slot here too — they are pushed, in the SAME order, into
    /// `EncodedFrame::composites` (a `BlendCompositeInstance` the GPU
    /// pipeline reads directly), and reading them back off THIS marker
    /// as well would just be a second, unused copy (`depth`/
    /// `stencil_ref` are the only two values the executor itself needs
    /// to pick a target and a stencil test — the vertex/fragment shader
    /// gets `alpha`/`clip_rect` from the instance buffer, not from this
    /// marker).
    PopLayer { depth: u32, stencil_ref: Option<u32> },
}

/// Painter's-order encoded frame — the Scene walker's output, ready
/// for `NativeUrxRenderer` to upload + replay via `ops`.
#[derive(Debug, Default)]
pub(crate) struct EncodedFrame {
    pub(crate) quads: Vec<QuadInstance>,
    pub(crate) lines: Vec<LineInstance>,
    pub(crate) triangles: Vec<TriInstance>,
    pub(crate) glyphs: Vec<GlyphInstance>,
    /// Mask-write geometry (design §2/§4) — a SEPARATE buffer from
    /// `triangles`; `StencilMask` batches index into this one.
    pub(crate) stencil_masks: Vec<TriInstance>,
    /// Radial/Sweep gradient-mesh triangles (Wave 4 Commit 3, design
    /// §2.3) — `BatchKind::Gradient` batches index into this one.
    /// Linear gradients stay on `triangles` (unchanged, design §2.1).
    pub(crate) gradients: Vec<GradientInstance>,
    /// `DrawCommand::Image` quads (Wave 4 Commit 3, design §4.3) —
    /// `BatchKind::Image(id)` batches index into this one.
    pub(crate) images: Vec<ImageInstance>,
    /// One entry per `FrameOp::PopLayer`, in the SAME order — the
    /// renderer's executor reads these via a running counter as it
    /// processes `PopLayer` ops (design §3.6), the same "upload once,
    /// replay via index" convention every other instance type here
    /// uses; no `Batch`/`BatchKind` entry needed since a composite draw
    /// is never coalesced with anything (each one binds a DIFFERENT
    /// layer's own resolve texture).
    pub(crate) composites: Vec<BlendCompositeInstance>,
    pub(crate) ops: Vec<FrameOp>,
    /// Set (never cleared) the first time ANY `PushClipRoundedRect` is
    /// scanned (design §2.5) — read ONCE by `render_into_encoder`
    /// before opening any pass, deciding for the WHOLE frame whether a
    /// `StencilTarget` is even allocated. `false` (the overwhelming
    /// common case — no Wave 1/2 fixture uses a rounded clip) means
    /// byte-identical cost/behaviour to pre-Wave-3.
    pub(crate) has_rounded_clip: bool,
    /// Transient encode-time channel — NOT part of the frame's actual
    /// render output, just how `active_clip` tells the NEXT `push_quad`/
    /// `push_line`/`push_triangle`/`push_glyph` call which stencil
    /// depth this command's instances should test against. Never read
    /// directly by tests or the renderer — its only observable effect
    /// is `Batch::stencil_ref` on whatever gets pushed next.
    current_stencil_ref: Option<u32>,
}

impl EncodedFrame {
    fn push_quad(&mut self, instance: QuadInstance) {
        let start = self.quads.len() as u32;
        self.quads.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Quad, start, stencil_ref);
    }

    fn push_line(&mut self, instance: LineInstance) {
        let start = self.lines.len() as u32;
        self.lines.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Line, start, stencil_ref);
    }

    fn push_triangle(&mut self, instance: TriInstance) {
        let start = self.triangles.len() as u32;
        self.triangles.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Triangle, start, stencil_ref);
    }

    fn push_glyph(&mut self, instance: GlyphInstance) {
        let start = self.glyphs.len() as u32;
        self.glyphs.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Glyph, start, stencil_ref);
    }

    fn push_gradient(&mut self, instance: GradientInstance) {
        let start = self.gradients.len() as u32;
        self.gradients.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Gradient, start, stencil_ref);
    }

    fn push_image(&mut self, instance: ImageInstance, id: ImageId) {
        let start = self.images.len() as u32;
        self.images.push(instance);
        let stencil_ref = self.current_stencil_ref;
        self.bump_batch(BatchKind::Image(id), start, stencil_ref);
    }

    /// Mask-write geometry (design §2.4) — `gate_ref` is the EXPLICIT
    /// stencil-test gate this specific mask write needs (parent depth
    /// for an increment, this scope's own depth for a decrement), NOT
    /// read from `current_stencil_ref` (which tracks CONTENT depth, a
    /// different value at push/pop time — see `emit_stencil_mask_batch`).
    fn push_stencil_mask(&mut self, instance: TriInstance, op: MaskOp, gate_ref: u32) {
        let start = self.stencil_masks.len() as u32;
        self.stencil_masks.push(instance);
        self.bump_batch(BatchKind::StencilMask(op), start, Some(gate_ref));
    }

    /// Extend the current batch if BOTH `kind` and `stencil_ref` match,
    /// else open a new `FrameOp::Draw` (design §5, extended by Wave 3
    /// Commit 2's `stencil_ref` equality requirement, further extended
    /// by Commit 3's marker-refusal — see `FrameOp`'s doc comment).
    fn bump_batch(&mut self, kind: BatchKind, start: u32, stencil_ref: Option<u32>) {
        if let Some(FrameOp::Draw(last)) = self.ops.last_mut() {
            if last.kind == kind && last.stencil_ref == stencil_ref {
                last.count += 1;
                return;
            }
        }
        self.ops.push(FrameOp::Draw(Batch { kind, start, count: 1, stencil_ref }));
    }

    /// Push a `PushLayer`/`PopLayer` marker directly — bypasses
    /// `bump_batch`'s coalescing entirely (a marker is never merged
    /// with anything, design §3.2).
    fn push_marker(&mut self, op: FrameOp) {
        self.ops.push(op);
    }

    /// Test-only convenience — every `FrameOp::Draw` entry, in order,
    /// as a flat `Vec<Batch>`, ignoring blend-layer structure. NOT used
    /// internally by `bump_batch`, which must see the raw `ops` list
    /// (including markers) to correctly refuse to coalesce across one;
    /// NOT used by the renderer's executor either (`replay_ops` walks
    /// `ops` directly, since it needs the markers `draw_batches` throws
    /// away). `#[cfg(test)]`-gated rather than plain `pub(crate)` — a
    /// production-reachable method that's actually only ever called
    /// from `#[cfg(test)] mod tests` would be dead code outside test
    /// builds.
    #[cfg(test)]
    pub(crate) fn draw_batches(&self) -> Vec<Batch> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                FrameOp::Draw(b) => Some(*b),
                _ => None,
            })
            .collect()
    }
}

/// CPU-side clip stack, computed at encode time (design §5) — same
/// intersection semantics as `uzor-urx-cpu::clip::ClipStack`'s plain-
/// rect entries (`push_rect`/`current`): root = full viewport,
/// intersect-with-current-top on every push, guarded pop (root is
/// never popped).
///
/// **Wave 3 Commit 2**: gains a `Rounded` frame variant (design §4) —
/// `PushClipRoundedRect` now drives REAL stencil clipping (closes
/// `native_rounded_clip_to_rect_bbox`, removed entirely — there is no
/// approximation left to degrade about). The bbox intersection still
/// feeds the SAME plain-rect `clip_rect` mechanism every instance
/// already carries (stencil is purely ADDITIVE precision on top, never
/// a replacement, design §4) — a `Rounded` frame's `current()` value is
/// identical in shape to a `Rect` frame's, just also carrying the
/// cached tessellated mask mesh + its projection coefficients (captured
/// at push time, replayed unchanged at the matching pop — mirrors
/// `emit_solid_mesh`'s replay contract) so the eventual `PopClip` can
/// re-emit the SAME geometry as a decrement without re-tessellating.
///
/// **CPU clip-edge AA finding** (Wave 1, still true for plain-rect
/// clips): `uzor-urx-cpu`'s `ClipStack::coverage` (`clip.rs:34-39`)
/// does a HARD binary in/out test for plain `Rect` entries, but that
/// method is only reachable via `pixel_coverage()`, which every CPU
/// rasteriser only calls when a MASK (rounded-clip) entry is present.
/// For a plain-rect-only clip stack, every CPU primitive instead
/// intersects its own float bounding box against `clip.current()` and
/// feeds the result through the SAME analytic per-pixel coverage
/// function used for the shape's own edges — a SOFT, sub-pixel-accurate
/// AA transition, same as any other edge — while this native path's
/// fragment-shader clip test is a HARD binary cut (the same "two
/// different AA algorithms" class of divergence as every other Wave 1
/// finding). Wave 3's stencil+MSAA mechanism is its OWN separate,
/// coarser-but-graduated AA source (design §2.6) — a third, distinct
/// divergence source layered on top for rounded-clip content
/// specifically, not a replacement for this one.
enum ClipFrame {
    Rect([f32; 4]),
    Rounded {
        /// Bbox intersection — feeds the existing `clip_rect` mechanism
        /// unchanged (design §4).
        rect_device: [f32; 4],
        /// This frame's OWN stencil depth (1-indexed — the outermost
        /// rounded clip is depth 1).
        stencil_depth: u32,
        /// Cached tessellated mask geometry (LOCAL space) — the SAME
        /// `TessCache` Wave 1 built for gradient-routed `FillRect`, no
        /// new cache.
        mesh: Arc<TessMesh>,
        /// Full affine transform captured at PUSH time (Wave 4 Commit 4,
        /// design §5.4) — replaces the earlier translate+scale-only
        /// `sx/sy/tx/ty` quadruple now that `project_local`/
        /// `emit_stencil_mask_batch` reproject through the full 6-coeff
        /// affine. The eventual `PopClip`/blend-layer-replay re-projects
        /// the SAME mesh through this SAME `Affine`, not whatever
        /// transform happens to be active later (there isn't one —
        /// `PopClip` carries no transform of its own — but documented
        /// for the same "replay contract" reason `emit_solid_mesh`
        /// documents its own coefficients).
        transform: Affine,
    },
}

struct ClipStack {
    stack: Vec<ClipFrame>,
    /// Current rounded-clip nesting depth — 0 means "no rounded clip
    /// active right now" (either before the first push, or after the
    /// last matching pop). Tracked separately from `stack.len()` since
    /// plain `Rect` frames can interleave with `Rounded` ones (design
    /// §2.4's nested-rounded-plus-rect fixture is exactly this case).
    rounded_depth: u32,
}

impl ClipStack {
    fn new(viewport: Viewport) -> Self {
        Self {
            stack: vec![ClipFrame::Rect([0.0, 0.0, viewport.width as f32, viewport.height as f32])],
            rounded_depth: 0,
        }
    }

    /// Current active clip rect (bbox, top of stack) — identical shape
    /// regardless of frame kind (design §4: the plain-rect mechanism is
    /// unchanged by `Rounded` frames existing). The stack is seeded
    /// with the root entry in `new` and that entry is never popped, so
    /// this never panics.
    fn current(&self) -> [f32; 4] {
        match self.stack.last().expect("ClipStack is seeded with a root entry that is never popped") {
            ClipFrame::Rect(r) => *r,
            ClipFrame::Rounded { rect_device, .. } => *rect_device,
        }
    }

    /// Current rounded-clip nesting depth (design item 4 / §2.4) — `0`
    /// when no rounded clip is active right now.
    fn rounded_depth(&self) -> u32 {
        self.rounded_depth
    }

    /// Intersect `r` (already in device space) with the current top
    /// and push a plain `Rect` frame — mirrors
    /// `uzor-urx-cpu::clip::ClipStack::push_rect`'s intersection
    /// (`clip.rs:73-82`), just carrying `[x,y,w,h]` instead of a
    /// `kurbo::Rect`.
    fn push_rect_device(&mut self, r: [f32; 4]) {
        let cur = self.current();
        let x0 = cur[0].max(r[0]);
        let y0 = cur[1].max(r[1]);
        let x1 = (cur[0] + cur[2]).min(r[0] + r[2]);
        let y1 = (cur[1] + cur[3]).min(r[1] + r[3]);
        self.stack.push(ClipFrame::Rect([x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]));
    }

    /// Intersect `r` with the current top (SAME bbox math as
    /// `push_rect_device`) and push a `Rounded` frame carrying the mask
    /// mesh + its projection coefficients. Returns the NEW stencil
    /// depth (post-increment) — the caller (`encode_scene`) uses
    /// `new_depth - 1` as the mask-write's gate reference (design §2.4)
    /// and emits the matching `MaskOp::Increment` batch itself (kept OUT
    /// of `ClipStack` to keep this type free of any `EncodedFrame`
    /// dependency — pure bookkeeping, same separation of concerns as
    /// Wave 1's `ClipStack`).
    fn push_rounded_rect_device(&mut self, r: [f32; 4], mesh: Arc<TessMesh>, transform: Affine) -> u32 {
        let cur = self.current();
        let x0 = cur[0].max(r[0]);
        let y0 = cur[1].max(r[1]);
        let x1 = (cur[0] + cur[2]).min(r[0] + r[2]);
        let y1 = (cur[1] + cur[3]).min(r[1] + r[3]);
        let rect_device = [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)];
        self.rounded_depth += 1;
        let depth = self.rounded_depth;
        self.stack.push(ClipFrame::Rounded { rect_device, stencil_depth: depth, mesh, transform });
        depth
    }

    /// Guarded pop — the root entry (index 0) is never removed, so an
    /// unbalanced (extra) `PopClip` is a defensive no-op rather than a
    /// panic, same policy as the legacy adapter's `clip_depth` guard
    /// (`adapter.rs:233-238`). Returns `Some((mesh, transform,
    /// stencil_depth))` when the popped frame was `Rounded` — the
    /// caller emits the matching `MaskOp::Decrement` batch from it
    /// (gated on `stencil_depth`, the scope's OWN depth, design §2.4);
    /// `None` for a `Rect` pop (nothing stencil-related to undo) or an
    /// underflow no-op.
    fn pop(&mut self) -> Option<(Arc<TessMesh>, Affine, u32)> {
        if self.stack.len() <= 1 {
            return None;
        }
        match self.stack.pop().expect("length > 1 checked above") {
            ClipFrame::Rounded { mesh, transform, stencil_depth, rect_device: _ } => {
                self.rounded_depth -= 1;
                Some((mesh, transform, stencil_depth))
            }
            ClipFrame::Rect(_) => None,
        }
    }

    /// End-of-scene unbalanced-scene guard (design §2.4's "unbalanced-
    /// scene guard", mirrored from the CPU-side blend-layer
    /// `force_close_all` audit fix) — force-pops every still-open
    /// `Rounded` frame (LIFO) so no buggy/unbalanced scene ever leaves
    /// the stencil buffer in a stale nonzero state for the rest of that
    /// (already-malformed) frame. Returns each popped frame's data (same
    /// shape as `pop()`'s `Some` case) so the caller emits one decrement
    /// batch per entry, counted `native_rounded_clip_force_closed_at_scene_end`
    /// (never silent — same doctrine as the blend-layer force-close).
    fn force_close_all_rounded(&mut self) -> Vec<(Arc<TessMesh>, Affine, u32)> {
        let mut closed = Vec::new();
        while let Some(entry) = self.pop() {
            closed.push(entry);
        }
        closed
    }

    /// Snapshot every currently-active `Rounded` frame, root-to-
    /// innermost nesting order (design §3.4 Risk 4) — for replay into a
    /// freshly-opened blend layer's OWN stencil texture. A new layer's
    /// stencil starts `Clear`ed to 0, but content inside it must behave
    /// as if it inherited the enclosing rounded-clip scope; the
    /// encode-side resolution (chosen over an executor-side `ClipStack`
    /// snapshot — see `encode_scene`'s `PushBlendLayer` arm) is to bake
    /// the SAME sequence of increments that built up the current stack
    /// directly into the op stream, right after the `PushLayer` marker,
    /// so the executor never needs to know anything about `ClipStack`
    /// at all — it just replays `Draw(StencilMask)` batches like any
    /// other, which happen to land in the newly-opened layer's pass
    /// because that's whichever pass is current when they're processed.
    /// Returns `(mesh, transform, stencil_depth, parent_bbox)` per
    /// active frame — `parent_bbox` is frame `i`'s OWN parent's bbox
    /// (`self.stack[i-1]`'s `current()`-equivalent value), exactly what
    /// `emit_stencil_mask_batch`'s original increment used as
    /// `clip_rect` (`stack[0]` is always the root `Rect` frame, never
    /// `Rounded`, so `i - 1` is always in bounds for any `Rounded` frame
    /// found).
    fn active_rounded_frames_for_replay(&self) -> Vec<(Arc<TessMesh>, Affine, u32, [f32; 4])> {
        let mut out = Vec::new();
        for i in 0..self.stack.len() {
            if let ClipFrame::Rounded { mesh, transform, stencil_depth, .. } = &self.stack[i] {
                let parent_bbox = match &self.stack[i - 1] {
                    ClipFrame::Rect(r) => *r,
                    ClipFrame::Rounded { rect_device, .. } => *rect_device,
                };
                out.push((mesh.clone(), *transform, *stencil_depth, parent_bbox));
            }
        }
        out
    }
}

/// `None` when the current clip has zero area — callers should elide
/// the primitive entirely (design commit 4 item 3: a zero-area clip
/// makes anything under it invisible by construction, so encoding an
/// instance for it would be pure waste, not a degrade).
///
/// **Also stashes the CURRENT stencil-test depth** into
/// `frame.current_stencil_ref` (design item 4's wgpu-pass-compatibility
/// finding) for whichever `push_quad`/`push_line`/`push_triangle`/
/// `push_glyph` call the caller makes next — `None` when no rounded
/// clip is active right now (depth 0), `Some(depth)` otherwise. This is
/// NOT a per-batch "_off vs _test" decision: mixing pipeline variants
/// within a SINGLE render pass that has a stencil attachment is a wgpu
/// validation error (`RenderPassCompatibilityError::IncompatibleDepthStencilAttachment`
/// — confirmed by reading `wgpu-core-29.0.3/src/device/mod.rs`'s
/// `RenderPassContext::check_compatible`, which compares
/// `attachments.depth_stencil: Option<TextureFormat>` between the pass
/// and every pipeline used in it: a pipeline built with `depth_stencil:
/// None` cannot be bound inside a pass whose `depth_stencil_attachment`
/// is `Some(_)`). The RENDERER alone decides `_off` vs `_test` for the
/// WHOLE frame from `EncodedFrame::has_rounded_clip`; `None` here is
/// defaulted to `0` at replay time when the frame turns out to be
/// armed, and is otherwise never read at all (an unarmed frame's `_off`
/// variant never touches stencil regardless).
#[inline]
fn active_clip(clip: &ClipStack, frame: &mut EncodedFrame) -> Option<[f32; 4]> {
    let r = clip.current();
    if r[2] <= 0.0 || r[3] <= 0.0 {
        return None;
    }
    let depth = clip.rounded_depth();
    frame.current_stencil_ref = if depth > 0 { Some(depth) } else { None };
    Some(r)
}

/// Encode-time blend-layer nesting tracker (design §3.3) — parallel to,
/// and completely independent of, `ClipStack`: `PushBlendLayer`/
/// `PopBlendLayer` never reset or nest the clip stack, content inside a
/// layer sees whatever clip/stencil state was already active, unchanged
/// mechanism (design §0.2).
struct LayerStack {
    /// One entry per currently-open layer: `(mode, alpha)` remembered
    /// from its `PushBlendLayer`, needed at the matching Pop to emit
    /// `FrameOp::PopLayer`.
    open: Vec<(BlendMode, f32)>,
    /// Incremented instead of pushing onto `open` once `open.len() ==
    /// max_depth` — the matching Pop decrements this FIRST (never
    /// touches `open`) so suppressed pushes/pops always balance without
    /// emitting any `FrameOp` at all (content just keeps drawing into
    /// whatever target was already active, unchanged).
    suppressed: u32,
    max_depth: usize,
}

/// Outcome of [`LayerStack::pop`].
enum LayerPopOutcome {
    /// A real layer closed — `depth` is the depth just closed (1-indexed).
    Popped { depth: u32, mode: BlendMode, alpha: f32 },
    /// This `Pop` matched a SUPPRESSED `Push` (the depth cap was hit,
    /// so no layer was ever actually opened for it) — a defensive
    /// no-op, not an error; nothing further to count (the matching
    /// `Push` already counted `native_blend_layer_depth_exceeded` once).
    Suppressed,
    /// No open layer AND nothing suppressed to balance — an unbalanced
    /// scene's extra `Pop`. Defensive no-op, mirrors `ClipStack::pop`'s
    /// existing guard.
    Underflow,
}

impl LayerStack {
    fn new(max_depth: usize) -> Self {
        Self { open: Vec::new(), suppressed: 0, max_depth }
    }

    /// Returns `Some(new_depth)` (1-indexed) when a real layer opened;
    /// `None` when the depth cap was hit (caller counts
    /// `native_blend_layer_depth_exceeded`; content keeps drawing into
    /// whatever target was already active, unchanged, no new
    /// `FrameOp::PushLayer` emitted).
    fn push(&mut self, mode: BlendMode, alpha: f32) -> Option<u32> {
        if self.open.len() >= self.max_depth {
            self.suppressed += 1;
            return None;
        }
        self.open.push((mode, alpha));
        Some(self.open.len() as u32)
    }

    fn pop(&mut self) -> LayerPopOutcome {
        if self.suppressed > 0 {
            self.suppressed -= 1;
            return LayerPopOutcome::Suppressed;
        }
        let Some((mode, alpha)) = self.open.pop() else {
            return LayerPopOutcome::Underflow;
        };
        let depth = self.open.len() as u32 + 1; // the depth just closed
        LayerPopOutcome::Popped { depth, mode, alpha }
    }

    /// End-of-scene unbalanced-scene guard (design §3.3, mirrored from
    /// the CPU-side `LayerStack::force_close_all` audit fix, Wave 3
    /// Commit 1) — drains every still-open REAL layer (LIFO), ignoring
    /// `suppressed` entirely (a leftover suppressed count with no
    /// matching real layer has no resource to close and no FrameOp was
    /// ever emitted for it — nothing to do). Returns each closed
    /// layer's `(depth, mode, alpha)` so the caller emits one synthetic
    /// `FrameOp::PopLayer` per entry, counted
    /// `native_blend_layer_force_closed_at_scene_end`.
    fn force_close_all(&mut self) -> Vec<(u32, BlendMode, f32)> {
        let mut closed = Vec::new();
        while let Some((mode, alpha)) = self.open.pop() {
            let depth = self.open.len() as u32 + 1;
            closed.push((depth, mode, alpha));
        }
        closed
    }
}

/// Count (never silently drop) when a blend layer's requested `mode`
/// differs from the one combination this wave actually implements
/// (`{Mix::Normal, Compose::SrcOver}`, design §0.3) — the GPU-side twin
/// of `uzor-urx-cpu::blend::degrade_for_mode` (Wave 3 Commit 1), same
/// two independent counters.
fn degrade_blend_mode(mode: &BlendMode) {
    if mode.mix != Mix::Normal {
        degrade("native_blend_layer_mix_to_normal");
    }
    if mode.compose != Compose::SrcOver {
        degrade("native_blend_layer_compose_to_srcover");
    }
}

/// Translate + uniform-scale decomposition (design §5) — the native
/// path drops shear/rotation (`c[1]`/`c[2]`) for Wave 1, matching the
/// legacy adapter's own known Wave-4 gap (`adapter.rs:311-323`). A
/// sheared `FillRect`/`Line` would need the (not-yet-built) path
/// pipeline to render correctly — Wave 4's documented "detect +
/// reroute" extension, not invented here.
fn decompose_translate_scale(t: &Affine) -> (f64, f64, f64, f64) {
    let c = t.as_coeffs();
    (c[0], c[3], c[4], c[5])
}

/// Transform `rect` by the translate+scale decomposition of `t`,
/// normalizing for a possible negative scale. Returns
/// `(x0, y0, w, h, |sx|, |sy|)` in device space.
fn transform_rect(rect: Rect, t: &Affine) -> (f64, f64, f64, f64, f64, f64) {
    let (sx, sy, tx, ty) = decompose_translate_scale(t);
    let x0 = rect.x0 * sx + tx;
    let y0 = rect.y0 * sy + ty;
    let x1 = rect.x1 * sx + tx;
    let y1 = rect.y1 * sy + ty;
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    (x0, y0, x1 - x0, y1 - y0, sx.abs(), sy.abs())
}

/// Project a LOCAL-space point through the FULL 6-coefficient affine
/// `t` — Wave 4 Commit 4 upgrade (design §5.4) from the earlier
/// translate+scale-only decomposition. `kurbo::Affine`'s own
/// `Mul<Point>` impl computes exactly `(a*x+c*y+e, b*x+d*y+f)`, the
/// SAME full formula `transform_point_full` manually replicates for
/// gradient PARAMETERS — this closes that function's own documented
/// "shape doesn't rotate, params do" asymmetry (see
/// `transform_gradient_params`'s doc comment, updated below) by making
/// mesh reprojection use the identical math. Every call site that used
/// to pre-decompose via `decompose_translate_scale` now passes
/// `transform` straight through — a strict generalization (translate+
/// scale is the special case `b == c == 0`), so every existing
/// identity/translate/uniform-scale fixture is byte-identical.
#[inline]
fn project_local(p: [f32; 2], t: &Affine) -> [f32; 2] {
    let mapped = *t * Point::new(p[0] as f64, p[1] as f64);
    [mapped.x as f32, mapped.y as f32]
}

/// Apply the FULL 2x3 affine `t` to a single point — mirrors
/// `uzor-urx-cpu::clip::transform_point_full`'s formula exactly (Wave 4
/// Commit 1), reimplemented here rather than shared across crates:
/// this backend does not (and should not) depend on its CPU sibling as
/// a library — they're peer backends, not shared infra (design §0.4's
/// own architectural principle, the reason `uzor-urx-image`/
/// `uzor-urx-glyph` exist as SEPARATE shared crates rather than one
/// backend depending on the other directly). Used ONLY for gradient
/// PARAMETERS (§0.1b/§2.4's "commit 1 parity" — Radial's `end_center`,
/// Sweep's `center`) — NOT for the tessellated MESH's own
/// reprojection, which stays on `project_local`'s translate+scale-only
/// decomposition until Commit 4's §5.4 upgrade (see
/// `transform_gradient_params`'s own doc comment for the resulting,
/// disclosed, one-commit-lived shape/gradient rotation asymmetry).
fn transform_point_full(t: &Affine, p: Point) -> Point {
    let c = t.as_coeffs();
    let (a, b, e, d, tx, ty) = (c[0], c[1], c[2], c[3], c[4], c[5]);
    Point::new(a * p.x + e * p.y + tx, b * p.x + d * p.y + ty)
}

/// `(hypot(a,b), hypot(c,d))` — the affine's own per-axis scale
/// magnitude from its linear part, ignoring rotation/shear. Mirrors
/// `uzor-urx-cpu::gradient::affine_scale_factors` (Wave 4 Commit 1)
/// formula-for-formula.
fn affine_scale_factors(t: &Affine) -> (f64, f64) {
    let c = t.as_coeffs();
    (c[0].hypot(c[1]), c[2].hypot(c[3]))
}

/// `atan2(b, a)` — the affine's rotation component. Mirrors
/// `uzor-urx-cpu::gradient::affine_rotation_angle` (Wave 4 Commit 1).
fn affine_rotation_angle(t: &Affine) -> f64 {
    let c = t.as_coeffs();
    c[1].atan2(c[0])
}

/// Unconditional `(sx, sy, angle, tx, ty)` via hypot/atan2 — no
/// shear/reflection validity check. `decompose_similarity` (below)
/// wraps this with a validity gate for the "is this a CLEAN rotation +
/// scale, no shear" question; `encode_image`'s "shear degrades to a
/// rotation-ONLY approximation" fallback (design §4.3) calls this
/// directly instead, since it explicitly WANTS the (honestly counted)
/// approximation even when the gate below would reject the transform.
fn decompose_similarity_raw(t: &Affine) -> (f64, f64, f64, f64, f64) {
    let c = t.as_coeffs();
    let (a, b, cc, d, e, f) = (c[0], c[1], c[2], c[3], c[4], c[5]);
    (a.hypot(b), cc.hypot(d), b.atan2(a), e, f)
}

/// `Some((sx, sy, angle, tx, ty))` when `t`'s linear part is a pure
/// rotation + non-uniform-axis-scale (columns orthogonal, i.e. no
/// shear, AND determinant > 0, i.e. no reflection — a reflection can't
/// be represented by rotating an axis-aligned quad, since `size` is an
/// unsigned magnitude). `None` when genuine shear or a reflection is
/// present (design §5.1's `decompose_similarity` sketch — introduced
/// here, this commit, for `encode_image`'s exclusive use; Commit 4
/// wires FillRect/StrokeRect's OWN Quad-SDF-vs-Triangle routing
/// decision through this SAME function rather than a duplicate, since
/// it's pure geometry math with no Image-specific assumption baked in
/// — `quad.rs`/`tessellate.rs`, the files that routing decision
/// touches, are this commit's own explicit off-limits list).
fn decompose_similarity(t: &Affine) -> Option<(f64, f64, f64, f64, f64)> {
    let c = t.as_coeffs();
    let (a, b, cc, d, _, _) = (c[0], c[1], c[2], c[3], c[4], c[5]);
    const EPS: f64 = 1e-6;
    let dot = a * cc + b * d;
    if dot.abs() > EPS * (a.hypot(b) * cc.hypot(d)).max(1e-12) {
        return None; // columns not orthogonal -> genuine shear
    }
    let det = a * d - b * cc;
    if det <= 0.0 {
        return None; // reflection present -> not representable as a quad rotation
    }
    Some(decompose_similarity_raw(t))
}

#[inline]
fn degrade(kind: &'static str) {
    metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => kind).increment(1);
}

/// Which case `resolve_brush_color` hit — drives which `native_*`
/// counter (if any) the caller emits. Kept distinct from the color
/// itself so callers can label gradient-vs-image degrades honestly
/// instead of collapsing both into one "brush wasn't solid" counter.
enum BrushKind {
    Solid,
    Gradient,
    Image,
}

/// The gradient's own first stop colour, straight (non-premultiplied)
/// sRGB — the fallback representative colour whenever a gradient
/// can't render its real per-fragment/per-vertex form (Radial/Sweep's
/// `native_gradient_lut_full_this_frame` degrade path in
/// `emit_gradient_mesh` reuses this SAME resolution, not a re-derived
/// copy).
#[inline]
fn first_stop_color(gradient: &Gradient) -> Color {
    gradient
        .stops
        .first()
        .map(|s| s.color.to_alpha_color::<peniko::color::Srgb>())
        .unwrap_or(Color::from_rgba8(0, 0, 0, 0))
}

/// Resolve any `Brush` to a single representative colour: the brush's
/// own colour for `Solid`, the first gradient stop for `Gradient`
/// (matches the legacy adapter's `brush_to_solid_color`,
/// `adapter.rs:290-298`), fully transparent for `Image` (no image
/// atlas wired yet).
fn resolve_brush_color(brush: &Brush) -> (Color, BrushKind) {
    match brush {
        Brush::Solid(c) => (*c, BrushKind::Solid),
        Brush::Gradient(g) => (first_stop_color(g), BrushKind::Gradient),
        Brush::Image(_) => (Color::from_rgba8(0, 0, 0, 0), BrushKind::Image),
    }
}

#[inline]
fn packed_color(c: Color) -> u32 {
    let p = c.to_rgba8();
    pack_rgba8([p.r, p.g, p.b, p.a])
}

/// `Some(r)` when `radii` is absent (implicit `0.0`) or all four
/// corners share one value — the fast Quad SDF path (design §6) stays
/// eligible. `None` when radii are genuinely non-uniform — the caller
/// routes to the Triangle pipeline instead (§6.2's `rect_bez_path`
/// true per-corner tessellation), closing
/// `native_per_corner_radii_uniform_approx` ENTIRELY (Wave 4 Commit 4
/// — there is no longer an approximation left to degrade about, so
/// this function no longer emits any counter itself; the caller's
/// shear-routing branch already owns the one telemetry counter this
/// whole routing decision needs, `native_rect_shear_to_triangle_pipeline`).
fn uniform_radius(radii: &Option<[f32; 4]>) -> Option<f32> {
    match radii {
        None => Some(0.0),
        Some(r) => {
            if r.iter().any(|v| (*v - r[0]).abs() > 0.01) {
                None
            } else {
                Some(r[0].max(0.0))
            }
        }
    }
}

/// The rect's own center point — normalization-independent (kurbo's
/// `Rect::center` is a plain midpoint of `x0/x1`/`y0/y1`, correct
/// regardless of whether the rect is stored with `x0 <= x1`). Shared by
/// the Quad-SDF FillRect/StrokeRect placement below and
/// `encode_image`'s identical recipe (Wave 4 Commit 3) — both need "the
/// TRUE device-space center under the full affine," not an axis-aligned
/// bbox center.
#[inline]
fn rect_center(rect: Rect) -> Point {
    rect.center()
}

/// Local (pre-tessellation) stroke width to feed the tessellator so
/// that, after the mesh is reprojected through the FULL transform at
/// replay (`emit_solid_mesh`/`emit_gradient_mesh`), the resulting
/// on-screen stroke width comes out DEVICE-CONSTANT — matching CPU's
/// own device-space-constant stroke-width semantic (design §0.2: CPU
/// applies `stroke.width` AFTER transforming points, never scaling the
/// width itself). `affine_scale_factors` gives a valid magnitude for
/// ANY affine, including a genuine shear — used here unconditionally
/// (every Triangle-routed stroke, gradient or solid, non-uniform-radii
/// or sheared, needs the same unification, not just the Quad-SDF
/// similarity case).
fn local_stroke_width(stroke_width: f32, transform: &Affine) -> f32 {
    let (sx, sy) = affine_scale_factors(transform);
    let avg_scale = ((sx.abs() + sy.abs()) * 0.5).max(1e-6);
    (stroke_width as f64 / avg_scale) as f32
}

/// Tessellate `path` for `stroke`, pre-dividing `stroke.width` by the
/// transform's own average scale magnitude (design §0.2) before handing
/// it to `tess_cache.get_or_insert_stroke` — the ADJUSTED width is what
/// `TessKey::for_stroke` hashes (not the raw `stroke.width`), so the
/// cache naturally re-keys per distinct effective scale (the disclosed
/// cost design §0.2 accepts). ONE shared call site for every
/// Triangle-routed stroke (`StrokePath`, gradient-brushed `StrokeRect`,
/// non-uniform-radii/sheared solid `StrokeRect`) so the unification
/// can't accidentally apply to only some of them.
fn tess_stroke_scaled(tess_cache: &mut TessCache, path: &BezPath, stroke: &Stroke, transform: &Affine) -> Arc<TessMesh> {
    let adjusted = Stroke { width: local_stroke_width(stroke.width, transform), ..*stroke };
    tess_cache.get_or_insert_stroke(path, &adjusted)
}

/// `atlas` is `Option<&mut NativeGlyphAtlas>` rather than the design's
/// literal `&mut NativeGlyphAtlas` (design §6) — a deliberate,
/// disclosed deviation. `NativeGlyphAtlas` only exists device-backed
/// (its texture/bind-group-layout/bind-group are real GPU resources,
/// `atlas.rs::NativeGlyphAtlas::new`); a hard `&mut` parameter would
/// force EVERY one of this module's ~30 pre-existing Wave-1 unit tests
/// (`FillRect`/`Line`/`FillPath`/clip-stack — none of which ever touch
/// a glyph) to also acquire a headless GPU device just to call
/// `encode_scene`, turning them all `#[ignore]`-gated. `Option` keeps
/// every one of those tests exactly as fast/device-free as before
/// (they pass `None`, which is provably safe: none of them ever emit
/// `DrawCommand::GlyphRun`). `NativeUrxRenderer::render_into_encoder`
/// (Commit 2) always has a live `self.glyph_atlas` and always passes
/// `Some(&mut self.glyph_atlas)` — `None` is a test-only state, never
/// reachable from production. See `encode_glyph_run`'s doc comment for
/// how a `None` atlas is handled if a glyph ever DOES successfully
/// rasterise under it (defensive-only; unreachable in practice and in
/// every test this commit adds).
/// Emit one `MaskOp` batch's worth of `TriInstance`s from an
/// already-tessellated (LOCAL-space) mask mesh, re-projected through
/// `sx/sy/tx/ty` (design §2.3/§2.4). `clip_rect` is the PARENT scope's
/// plain-rect bbox — the ALREADY-active clip before this push (or
/// after this pop) — never the rounded rect's OWN newly-computed bbox,
/// since the mask write must respect whatever plain-rect clip was
/// already active, not the shape it's about to help refine (design §2.3
/// "a mask write additionally respects whatever PLAIN-RECT clip is
/// active"). `gate_ref` is `depth - 1` for an increment (§2.4: gated on
/// the PARENT scope, so the write only lands where that scope was
/// already active) or `depth` for a decrement (gated on THIS scope's
/// own depth, so it only clears pixels CURRENTLY at that depth). Color
/// fields carry a fixed opaque dummy — ignored by
/// `STENCIL_MASK_SHADER_NATIVE`, `ColorWrites::empty()` blocks the
/// pipeline from ever writing color regardless (design §2.3).
fn emit_stencil_mask_batch(
    frame: &mut EncodedFrame,
    mesh: &TessMesh,
    transform: &Affine,
    clip_rect: [f32; 4],
    op: MaskOp,
    gate_ref: u32,
) {
    const DUMMY_COLOR: u32 = 0xFFFF_FFFF;
    for tri in &mesh.triangles {
        frame.push_stencil_mask(
            TriInstance {
                v0: project_local(tri[0], transform),
                v1: project_local(tri[1], transform),
                v2: project_local(tri[2], transform),
                color0: DUMMY_COLOR,
                color1: DUMMY_COLOR,
                color2: DUMMY_COLOR,
                _pad0: 0.0,
                clip_rect,
            },
            op,
            gate_ref,
        );
    }
}

/// `blend_layer_max_depth` — `UrxConfig::blend_layer_max_depth`, read
/// once by `NativeUrxRenderer::with_config` and threaded through as a
/// plain resolved value (design §3.3), consistent with `tess_cache`/
/// `atlas` above rather than accepting the whole config struct.
///
/// `lut: Option<&mut GradientLutAtlas>` (Wave 4 Commit 3) mirrors
/// `atlas`'s own `Option` shape exactly, for the identical reason (see
/// `encode_glyph_run`'s doc comment) — `GradientLutAtlas` only exists
/// device-backed; a hard `&mut` would force every one of this module's
/// device-free unit tests that never touch a Radial/Sweep gradient to
/// also acquire a headless GPU device. `NativeUrxRenderer::render_into_encoder`
/// always supplies `Some`; `None` is a test-only state.
///
/// `text_gamma_enabled` — `UrxConfig::text_gamma_enabled` (URX
/// text-gamma design, 2026-07-26), read once at `NativeUrxRenderer`
/// construction and threaded through the same "plain resolved value"
/// way as `blend_layer_max_depth` above. Consumed only by
/// `encode_glyph_run` (every other primitive ignores it).
pub(crate) fn encode_scene(
    scene: &Scene,
    viewport: Viewport,
    tess_cache: &mut TessCache,
    mut atlas: Option<&mut NativeGlyphAtlas>,
    mut lut: Option<&mut GradientLutAtlas>,
    blend_layer_max_depth: usize,
    text_gamma_enabled: bool,
) -> EncodedFrame {
    let mut frame = EncodedFrame::default();
    let mut clip = ClipStack::new(viewport);
    let mut layer_stack = LayerStack::new(blend_layer_max_depth);

    for cmd in &scene.commands {
        // Same "silent skip + counter" policy as `uzor-urx-cpu`
        // (`uzor-urx-cpu/src/backend.rs:137-140`) — degenerate geometry
        // passes through to the per-primitive code below, which already
        // rejects zero-area rects via its own `w <= 0.0 || h <= 0.0` guard.
        if let Err(ValidationIssue::NonFinite) = validate_command(cmd) {
            metrics::counter!(KEY_RENDER_SKIPPED_NONFINITE).increment(1);
            continue;
        }

        match cmd {
            DrawCommand::PushClipRect { rect, transform } => {
                let (x0, y0, w, h, _, _) = transform_rect(*rect, transform);
                clip.push_rect_device([x0 as f32, y0 as f32, w as f32, h as f32]);
            }
            DrawCommand::PushClipRoundedRect { rect, transform } => {
                // Real stencil clip (Wave 3 Commit 2, design §2.4) —
                // closes `native_rounded_clip_to_rect_bbox` entirely
                // (no approximation left to degrade about). The bbox
                // intersection still feeds the SAME plain-rect
                // `clip_rect` mechanism as before (design §4).
                let (x0, y0, w, h, _, _) = transform_rect(rect.rect(), transform);
                let parent_clip_rect = clip.current(); // gate for the mask write below
                let path = (*rect).into_path(crate::tessellate::TESS_TOLERANCE_PX);
                let mesh = tess_cache.get_or_insert_fill(&path, FillRule::NonZero);
                let parent_depth = clip.rounded_depth();
                let new_depth =
                    clip.push_rounded_rect_device([x0 as f32, y0 as f32, w as f32, h as f32], mesh.clone(), *transform);
                debug_assert_eq!(new_depth, parent_depth + 1, "push must increment depth by exactly 1");
                emit_stencil_mask_batch(&mut frame, &mesh, transform, parent_clip_rect, MaskOp::Increment, parent_depth);
                frame.has_rounded_clip = true;
            }
            DrawCommand::PopClip => {
                if let Some((mesh, transform, depth)) = clip.pop() {
                    // The parent scope's bbox is whatever `clip.current()`
                    // is NOW, after the pop already happened above.
                    let parent_clip_rect = clip.current();
                    emit_stencil_mask_batch(&mut frame, &mesh, &transform, parent_clip_rect, MaskOp::Decrement, depth);
                }
            }
            DrawCommand::GlyphRun { glyphs, font, font_size, brush, transform, text: _ } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_glyph_run(
                    &mut frame,
                    atlas.as_deref_mut(),
                    *font,
                    *font_size,
                    glyphs,
                    brush,
                    transform,
                    clip_rect,
                    text_gamma_enabled,
                );
            }
            DrawCommand::Image { src, src_rect, dest, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_image(&mut frame, *src, *src_rect, *dest, transform, clip_rect);
            }
            DrawCommand::PushBlendLayer { mode, alpha, transform } => {
                // `Affine` derives `PartialEq` in the pinned kurbo 0.13
                // (same finding as `uzor-urx-cpu`'s Commit 1 CPU-side
                // arm) — a plain `!=` works directly.
                if *transform != Affine::IDENTITY {
                    degrade("native_blend_layer_transform_ignored");
                }
                match layer_stack.push(*mode, *alpha) {
                    Some(depth) => {
                        frame.push_marker(FrameOp::PushLayer { depth });
                        // Risk-4 resolution (design §3.4), encode-side
                        // (chosen over an executor-side `ClipStack`
                        // snapshot — see `ClipStack::active_rounded_frames_for_replay`'s
                        // doc comment): replay every currently-active
                        // `Rounded` frame's increment into the op stream
                        // RIGHT AFTER the `PushLayer` marker, so whichever
                        // pass the executor has open when it processes
                        // these `Draw(StencilMask)` batches — the new
                        // layer's, since it opens on `PushLayer` before
                        // the next op is processed — ends up with the
                        // SAME nesting depth the enclosing scope already
                        // established, without the executor needing to
                        // know anything about `ClipStack` at all.
                        for (mesh, transform, stencil_depth, parent_bbox) in clip.active_rounded_frames_for_replay() {
                            emit_stencil_mask_batch(
                                &mut frame,
                                &mesh,
                                &transform,
                                parent_bbox,
                                MaskOp::Increment,
                                stencil_depth - 1,
                            );
                        }
                    }
                    None => degrade("native_blend_layer_depth_exceeded"),
                }
            }
            DrawCommand::PopBlendLayer => match layer_stack.pop() {
                LayerPopOutcome::Popped { depth, mode, alpha } => {
                    degrade_blend_mode(&mode);
                    // Same values `active_clip` would produce for a
                    // content draw AT this exact point (design §3.6) —
                    // computed directly rather than via `active_clip`
                    // itself, since a degenerate (zero-area) clip must
                    // NOT skip emitting the `PopLayer` marker the way it
                    // would skip a content primitive (the layer still
                    // needs to structurally close + composite, even if
                    // the composite ends up clipped to nothing).
                    let clip_rect = clip.current();
                    let stencil_ref = if clip.rounded_depth() > 0 { Some(clip.rounded_depth()) } else { None };
                    frame.push_marker(FrameOp::PopLayer { depth, stencil_ref });
                    frame.composites.push(BlendCompositeInstance { alpha, _pad: [0.0; 3], clip_rect });
                }
                LayerPopOutcome::Suppressed => {}
                LayerPopOutcome::Underflow => degrade("native_blend_layer_pop_underflow"),
            },
            DrawCommand::FillRect { rect, radii, brush, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_fill_rect(&mut frame, tess_cache, lut.as_deref_mut(), *rect, radii, brush, transform, clip_rect);
            }
            DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_stroke_rect(&mut frame, tess_cache, lut.as_deref_mut(), *rect, radii, stroke, brush, transform, clip_rect);
            }
            DrawCommand::Line { from, to, stroke, brush, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_line(&mut frame, *from, *to, stroke, brush, transform, clip_rect);
            }
            DrawCommand::FillPath { path, rule, brush, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_fill_path(&mut frame, tess_cache, lut.as_deref_mut(), path, *rule, brush, transform, clip_rect);
            }
            DrawCommand::StrokePath { path, stroke, brush, transform } => {
                let Some(clip_rect) = active_clip(&clip, &mut frame) else { continue };
                encode_stroke_path(&mut frame, tess_cache, lut.as_deref_mut(), path, stroke, brush, transform, clip_rect);
            }
        }
    }

    // Unbalanced-scene guards, run in this FIXED order (layers, then
    // clips) — this is a deliberate, documented simplification for the
    // genuinely pathological case of a scene with BOTH an open blend
    // layer AND an open rounded clip left unclosed at scene end. The
    // two force-close mechanisms are fully independent stacks (design
    // §0.2/§3.3), so this code has no way to know which was ACTUALLY
    // opened first without unifying their timelines — closing layers
    // first means each force-closed layer's composite still sees the
    // clip stack exactly as it stood mid-scene (a still-active rounded
    // clip's `clip_rect`/`stencil_ref` are captured before that clip is
    // ITSELF force-closed below), which is the safer of the two
    // possible orderings for a malformed scene. Never panics either way.
    //
    // Blend layers first (design §3.3, mirrored from the CPU-side
    // `LayerStack::force_close_all` audit fix, Wave 3 Commit 1) — any
    // `PushBlendLayer` with no matching `PopBlendLayer` still gets
    // composited here, counted `native_blend_layer_force_closed_at_scene_end`.
    for (depth, mode, alpha) in layer_stack.force_close_all() {
        degrade_blend_mode(&mode);
        let clip_rect = clip.current();
        let stencil_ref = if clip.rounded_depth() > 0 { Some(clip.rounded_depth()) } else { None };
        frame.push_marker(FrameOp::PopLayer { depth, stencil_ref });
        frame.composites.push(BlendCompositeInstance { alpha, _pad: [0.0; 3], clip_rect });
        degrade("native_blend_layer_force_closed_at_scene_end");
    }

    // Then rounded clips (design §2.4, mirrored from the same CPU-side
    // audit fix) — any `PushClipRoundedRect` with no matching `PopClip`
    // gets a synthetic decrement here, so the stencil buffer never
    // carries a stale nonzero value into whatever comes after this
    // (already-malformed) frame. Never silent.
    for (mesh, transform, depth) in clip.force_close_all_rounded() {
        let parent_clip_rect = clip.current();
        emit_stencil_mask_batch(&mut frame, &mesh, &transform, parent_clip_rect, MaskOp::Decrement, depth);
        degrade("native_rounded_clip_force_closed_at_scene_end");
    }

    frame
}

/// Routing (design §2.5): `Solid` stays on the fast Quad SDF path
/// unchanged. `Gradient` (ANY kind — Linear/Radial/Sweep all 3 now
/// full, Wave 4 Commit 3) always tessellates `rect_bez_path` and
/// dispatches through `emit_gradient_mesh`, the SAME mesh the
/// solid-fill path would have needed anyway for a rounded rect — zero
/// marginal pipeline cost for the radii case. `Image` (the peniko
/// inline-image BRUSH, NOT `DrawCommand::Image`) stays dropped
/// entirely — out of scope, design §4.1, unchanged.
fn encode_fill_rect(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    lut: Option<&mut GradientLutAtlas>,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    match brush {
        Brush::Solid(c) => encode_fill_rect_solid(frame, tess_cache, rect, radii, *c, transform, clip_rect),
        Brush::Gradient(g) => {
            let path = rect_bez_path(rect, radii);
            let mesh = tess_cache.get_or_insert_fill(&path, FillRule::NonZero);
            emit_gradient_mesh(frame, lut, &mesh, g, transform, clip_rect);
        }
        Brush::Image(_) => {
            // No image atlas wired yet — dropped entirely, matching the
            // legacy adapter's own `wgpu_fill_rect_image_dropped` policy
            // (`adapter.rs:109-115`). Unchanged, design §4.1 — this is
            // `Brush::Image`, NOT `DrawCommand::Image` (which now
            // renders for real, see `encode_image`).
            degrade("native_fill_rect_image_dropped");
        }
    }
}

/// Solid-brush `FillRect` routing (design §5.2/§6.2): uniform radii (or
/// none) AND a similarity transform (no shear/reflection) stays on the
/// fast Quad SDF path — now rotation-capable (§5.3, rotation angle
/// baked into `_pad0[0]`). Anything else (non-uniform radii, OR a
/// shear/reflection transform) routes through the Triangle pipeline via
/// `rect_bez_path`'s true per-corner tessellation — the SAME mesh the
/// gradient-routed case already builds for a rounded rect, zero new
/// pipeline machinery. `native_rect_shear_to_triangle_pipeline` is
/// telemetry, not a degrade (design §8: "a scene-content-derived
/// routing signal") — it fires ONLY for the shear branch, never for the
/// non-uniform-radii branch (that one is a closed, zero-cost capability
/// upgrade with nothing to report).
fn encode_fill_rect_solid(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    color: Color,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    if let Some(uniform_r) = uniform_radius(radii) {
        if let Some((sx, sy, angle, _, _)) = decompose_similarity(transform) {
            let normalized = rect.abs();
            let w = normalized.width() * sx;
            let h = normalized.height() * sy;
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let scale = ((sx + sy) * 0.5) as f32;
            let center = transform_point_full(transform, rect_center(normalized));
            let pos = [(center.x - w * 0.5) as f32, (center.y - h * 0.5) as f32];

            frame.push_quad(QuadInstance {
                pos,
                size: [w as f32, h as f32],
                color: packed_color(color),
                border_color: 0,
                corner_radius: uniform_r * scale,
                border_width: 0.0,
                _pad0: [angle as f32, 0.0],
                clip_rect,
            });
            return;
        }
        degrade("native_rect_shear_to_triangle_pipeline");
    }

    let path = rect_bez_path(rect, radii);
    let mesh = tess_cache.get_or_insert_fill(&path, FillRule::NonZero);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

/// Build a LOCAL-space rect `BezPath` — a real `RoundedRect` when
/// `radii` carries any positive value (exact per-corner radii, no
/// uniform-radius approximation needed since this path tessellates
/// the actual shape rather than evaluating an SDF), a plain 4-corner
/// rect otherwise. Mirrors the RECIPE `uzor-urx-cpu::backend.rs`'s
/// `StrokeRect`+radii arm uses (`RoundedRect::from_rect` +
/// `.into_path(0.25)`), NOT its `FillRect`+`Gradient`+radii behaviour
/// (which, per this module's doc comment, doesn't actually consult the
/// rounded mask it pushes).
fn rect_bez_path(rect: Rect, radii: &Option<[f32; 4]>) -> BezPath {
    if let Some(r) = radii {
        if r.iter().any(|v| *v > 0.0) {
            let rr = RoundedRect::from_rect(
                rect,
                RoundedRectRadii::new(
                    r[0].max(0.0) as f64,
                    r[1].max(0.0) as f64,
                    r[2].max(0.0) as f64,
                    r[3].max(0.0) as f64,
                ),
            );
            return rr.into_path(crate::tessellate::TESS_TOLERANCE_PX);
        }
    }
    let mut path = BezPath::new();
    path.move_to((rect.x0, rect.y0));
    path.line_to((rect.x1, rect.y0));
    path.line_to((rect.x1, rect.y1));
    path.line_to((rect.x0, rect.y1));
    path.close_path();
    path
}

/// Fold a raw gradient parameter `t` into `[0, 1]` per the spread mode
/// — copied formula-for-formula from `uzor-urx-cpu::gradient::apply_spread`
/// so a future non-`Pad` fixture still matches CPU's folding exactly.
fn apply_spread(t: f32, mode: Extend) -> f32 {
    match mode {
        Extend::Pad => t.clamp(0.0, 1.0),
        Extend::Repeat => {
            let f = t - t.floor();
            if f < 0.0 {
                f + 1.0
            } else {
                f
            }
        }
        Extend::Reflect => {
            let m = (t.rem_euclid(2.0) - 1.0).abs();
            1.0 - m
        }
    }
}

/// Straight (non-premultiplied) sRGB byte quad for a stop — same
/// conversion `uzor-urx-cpu::gradient::stop_rgba8` uses, minus the
/// premultiply step (this crate premultiplies once in the fragment
/// shader; see this module's doc comment).
fn stop_rgba8(stop: &peniko::ColorStop) -> [u8; 4] {
    let p = stop.color.to_alpha_color::<peniko::color::Srgb>().to_rgba8();
    [p.r, p.g, p.b, p.a]
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let r = (a as f32) * (1.0 - t) + (b as f32) * t;
    r.round().clamp(0.0, 255.0) as u8
}

/// Sample a stop sequence at parameter `t` — same bracket-search +
/// lerp shape as `uzor-urx-cpu::gradient::sample_stops`, but returns a
/// STRAIGHT (non-premultiplied) colour (see this module's doc comment
/// for why that's the right choice for this pipeline's convention).
fn sample_gradient_straight(stops: &ColorStops, t: f32) -> Color {
    if stops.is_empty() {
        return Color::from_rgba8(0, 0, 0, 0);
    }
    let last = stops.len() - 1;
    if stops.len() == 1 || t <= stops[0].offset {
        let [r, g, b, a] = stop_rgba8(&stops[0]);
        return Color::from_rgba8(r, g, b, a);
    }
    if t >= stops[last].offset {
        let [r, g, b, a] = stop_rgba8(&stops[last]);
        return Color::from_rgba8(r, g, b, a);
    }
    for w in stops.windows(2) {
        let s0 = &w[0];
        let s1 = &w[1];
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            let local = if span.abs() < 1e-9 { 0.0 } else { (t - s0.offset) / span };
            let c0 = stop_rgba8(s0);
            let c1 = stop_rgba8(s1);
            return Color::from_rgba8(
                lerp_u8(c0[0], c1[0], local),
                lerp_u8(c0[1], c1[1], local),
                lerp_u8(c0[2], c1[2], local),
                lerp_u8(c0[3], c1[3], local),
            );
        }
    }
    let [r, g, b, a] = stop_rgba8(&stops[last]);
    Color::from_rgba8(r, g, b, a)
}

/// Evaluate the gradient colour at a LOCAL-space mesh vertex by
/// projecting it onto the gradient axis (design §5).
fn gradient_vertex_color(
    p: [f32; 2],
    pos: &LinearGradientPosition,
    axis: (f64, f64),
    axis_len_sq: f64,
    stops: &ColorStops,
    extend: Extend,
) -> Color {
    let t_raw = if axis_len_sq < 1e-9 {
        0.0
    } else {
        ((p[0] as f64 - pos.start.x) * axis.0 + (p[1] as f64 - pos.start.y) * axis.1) / axis_len_sq
    };
    let t = apply_spread(t_raw as f32, extend);
    sample_gradient_straight(stops, t)
}

/// Device-space Radial/Sweep gradient parameters, ready to bake
/// straight into a `GradientInstance` (design §2.3/§2.4). Computed
/// ONCE per draw command, not per vertex/fragment.
struct GradientDeviceParams {
    p0: [f32; 2],
    p1: f32,
    p2: f32,
    kind_extend: u32,
}

/// Transform a Radial/Sweep gradient's own parameters through the SAME
/// full-affine math CPU's Commit-1 gradient-axis fix uses
/// (`transform_point_full`/`affine_scale_factors`/`affine_rotation_angle`
/// — design §0.1b/§2.4, "commit 1 parity"). `GradientKind::Linear` is
/// unreachable here — `emit_gradient_mesh`'s own Linear arm returns
/// before this function is ever called for it.
///
/// **Commit 3→4 asymmetry, now CLOSED**: Commit 3 computed these
/// params via full affine while the TESSELLATED MESH these params
/// accompany (`emit_gradient_mesh`, below) was still reprojected via
/// `project_local`'s translate+scale-ONLY decomposition — a disclosed,
/// one-commit-lived gap (a rotating transform would rotate the colour
/// FIELD without rotating the shape's own outline). Commit 4's §5.4
/// upgrade makes `project_local` apply the SAME full affine this
/// function already used, so shape and gradient field now rotate
/// together — this function's own math didn't need to change at all,
/// exactly as anticipated.
fn transform_gradient_params(kind: &GradientKind, extend: Extend, transform: &Affine) -> GradientDeviceParams {
    let extend_bits: u32 = match extend {
        Extend::Pad => 0,
        Extend::Repeat => 1,
        Extend::Reflect => 2,
    };
    match kind {
        GradientKind::Radial(pos) => {
            let center = transform_point_full(transform, pos.end_center);
            let (sx, sy) = affine_scale_factors(transform);
            let radius = (pos.end_radius as f64 * (sx + sy) * 0.5).max(1e-3);
            GradientDeviceParams {
                p0: [center.x as f32, center.y as f32],
                p1: radius as f32,
                p2: 0.0,
                kind_extend: (extend_bits << 2), // kind bits = 0 (Radial)
            }
        }
        GradientKind::Sweep(pos) => {
            let center = transform_point_full(transform, pos.center);
            let rot = affine_rotation_angle(transform);
            GradientDeviceParams {
                p0: [center.x as f32, center.y as f32],
                p1: (pos.start_angle as f64 + rot) as f32,
                p2: (pos.end_angle as f64 + rot) as f32,
                kind_extend: 1u32 | (extend_bits << 2),
            }
        }
        GradientKind::Linear(_) => unreachable!(
            "Linear stays on the per-vertex TriInstance path — emit_gradient_mesh's own Linear arm returns \
             before transform_gradient_params is ever called for it"
        ),
    }
}

/// Shared "resolve a LUT row, then emit one `GradientInstance` per
/// mesh triangle" tail for BOTH Radial and Sweep — factored out so
/// `emit_gradient_mesh`'s per-kind arms only need their own
/// kind-specific pre-check (Radial's focal-degrade counter) before
/// falling into this common path.
///
/// **`lut: None`** mirrors `encode_glyph_run`'s established `atlas:
/// None` precedent exactly (see that function's own doc comment for
/// the full rationale) — a defensive-only, production-unreachable
/// state (`NativeUrxRenderer::render_into_encoder` always supplies
/// `Some`) that exists purely so this module's ~30+ device-free unit
/// tests (none of which exercise Radial/Sweep) don't need a headless
/// GPU device just to call `encode_scene`.
///
/// **LUT full this frame** (`get_or_insert` returns `None` — every row
/// already touched this frame, design §2.2's never-evict-this-frame
/// invariant exhausted): falls back to the SAME first-stop solid color
/// every OTHER "can't render the real thing" gradient path in this
/// crate already uses, counted `native_gradient_lut_full_this_frame`
/// (design §8) — never silent, content never simply vanishes.
fn emit_gradient_lut_triangles(
    frame: &mut EncodedFrame,
    lut: Option<&mut GradientLutAtlas>,
    mesh: &TessMesh,
    gradient: &Gradient,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let row = match lut {
        Some(lut) => lut.get_or_insert(&gradient.stops, gradient.extend),
        None => None,
    };
    let Some(row) = row else {
        degrade("native_gradient_lut_full_this_frame");
        emit_solid_mesh(frame, mesh, transform, packed_color(first_stop_color(gradient)), clip_rect);
        return;
    };

    let params = transform_gradient_params(&gradient.kind, gradient.extend, transform);
    for tri in &mesh.triangles {
        frame.push_gradient(GradientInstance {
            v0: project_local(tri[0], transform),
            v1: project_local(tri[1], transform),
            v2: project_local(tri[2], transform),
            p0: params.p0,
            p1: params.p1,
            p2: params.p2,
            kind_extend: params.kind_extend,
            lut_row: row,
            clip_rect,
        });
    }
}

/// General gradient-mesh emitter — replaces `encode_fill_rect_linear_gradient`
/// as the single entry point every gradient-brushed, tessellated draw
/// command routes through (design §2.4): `FillRect`, `FillPath`,
/// `StrokePath`, `StrokeRect` (design §2.5's routing table — all 4 now
/// get real Radial/Sweep, not just Linear-on-FillRect). Dispatches per
/// `gradient.kind`: Linear stays on the EXISTING per-vertex `TriInstance`
/// path, completely unchanged (design §2.1 — barycentric interpolation
/// of an affine function is mathematically exact, migrating it would
/// be pure churn); Radial/Sweep go through the NEW per-triangle
/// `GradientInstance` + per-fragment LUT eval (design §2.3).
fn emit_gradient_mesh(
    frame: &mut EncodedFrame,
    lut: Option<&mut GradientLutAtlas>,
    mesh: &TessMesh,
    gradient: &Gradient,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    match &gradient.kind {
        GradientKind::Linear(pos) => {
            let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
            let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;

            for tri in &mesh.triangles {
                let c0 = gradient_vertex_color(tri[0], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
                let c1 = gradient_vertex_color(tri[1], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
                let c2 = gradient_vertex_color(tri[2], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
                frame.push_triangle(TriInstance {
                    v0: project_local(tri[0], transform),
                    v1: project_local(tri[1], transform),
                    v2: project_local(tri[2], transform),
                    color0: packed_color(c0),
                    color1: packed_color(c1),
                    color2: packed_color(c2),
                    _pad0: 0.0,
                    clip_rect,
                });
            }
        }
        GradientKind::Radial(pos) => {
            // Focal-vs-concentric check at ENCODE TIME (design §2.6) —
            // the identical formula CPU's Commit-1 fix uses, so the
            // SAME counter fires under the SAME condition on both
            // backends: genuine byte-parity, not "both wrong in the
            // same direction by coincidence." UNPREFIXED — a shared,
            // backend-agnostic label (matches CPU's own
            // `gradient_radial_focal_degraded`, never
            // `native_gradient_radial_focal_degraded`).
            let dx_c = (pos.end_center.x - pos.start_center.x).abs();
            let dy_c = (pos.end_center.y - pos.start_center.y).abs();
            if dx_c > 0.5 || dy_c > 0.5 || pos.start_radius.abs() > 0.5 {
                degrade("gradient_radial_focal_degraded");
            }
            emit_gradient_lut_triangles(frame, lut, mesh, gradient, transform, clip_rect);
        }
        GradientKind::Sweep(_) => {
            emit_gradient_lut_triangles(frame, lut, mesh, gradient, transform, clip_rect);
        }
    }
}

/// Routing (design §2.5): `Gradient` (any kind) now routes through the
/// SAME stroke-tessellation-of-`rect_bez_path` mechanism
/// `encode_stroke_path` already uses for arbitrary paths, just fed a
/// rect-shaped path — `emit_gradient_mesh`, closing
/// `native_strokerect_gradient_to_solid` entirely. `Solid`/`Image` stay
/// on the fast Quad SDF path exactly as before (Image still degrades
/// to a fully-transparent border colour, unchanged, design §4.1).
/// Routing mirrors `encode_fill_rect_solid` (design §5.2/§6.2): uniform
/// radii AND a similarity transform stays on Quad SDF — now with
/// stroke-width unification (§0.2: `border_width` is fed `stroke.width`
/// directly, NEVER scaled by the transform, matching CPU's
/// device-space-constant semantic). Anything else (non-uniform radii,
/// shear/reflection, or a Gradient brush — which always tessellates
/// regardless, design §2.5) routes through the Triangle pipeline via
/// `tess_stroke_scaled`, which applies the SAME width-unification at
/// the LOCAL-space pre-scale stage instead (§0.2's Path-pipeline
/// recipe) since that mesh gets reprojected through the full transform
/// rather than carrying a device-constant scalar field.
fn encode_stroke_rect(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    lut: Option<&mut GradientLutAtlas>,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    if stroke.width <= 0.0 {
        return;
    }

    if let Brush::Gradient(g) = brush {
        let path = rect_bez_path(rect, radii);
        let mesh = tess_stroke_scaled(tess_cache, &path, stroke, transform);
        emit_gradient_mesh(frame, lut, &mesh, g, transform, clip_rect);
        return;
    }

    let (color, kind) = resolve_brush_color(brush);
    if matches!(kind, BrushKind::Image) {
        degrade("native_strokerect_image_to_solid");
    }

    if let Some(uniform_r) = uniform_radius(radii) {
        if let Some((sx, sy, angle, _, _)) = decompose_similarity(transform) {
            let normalized = rect.abs();
            let w = normalized.width() * sx;
            let h = normalized.height() * sy;
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let scale = ((sx + sy) * 0.5) as f32;
            let center = transform_point_full(transform, rect_center(normalized));
            let pos = [(center.x - w * 0.5) as f32, (center.y - h * 0.5) as f32];
            let border_width = stroke.width.max(0.0); // §0.2: device-constant, no scale multiply

            frame.push_quad(QuadInstance {
                pos,
                size: [w as f32, h as f32],
                // Fully transparent fill — a StrokeRect draws only the
                // border band; see the centered-border fragment-shader
                // formula.
                color: 0,
                border_color: packed_color(color),
                corner_radius: uniform_r * scale,
                border_width,
                _pad0: [angle as f32, 0.0],
                clip_rect,
            });
            return;
        }
        degrade("native_rect_shear_to_triangle_pipeline");
    }

    let path = rect_bez_path(rect, radii);
    let mesh = tess_stroke_scaled(tess_cache, &path, stroke, transform);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

/// `Line` → `LineInstance`. Endpoints transform through the FULL affine
/// (design §5.2 — CPU already does this, `stroke_line_aa_with_caps`'s
/// `transform_point`; a line has no "shape" to rotate independently of
/// its own endpoints, so there's no similarity-vs-shear branch here at
/// all, unlike Rect). Width is now DEVICE-CONSTANT (design §0.2: the
/// `* scale` multiplication is deleted entirely, matching CPU's own
/// stroke-width semantic — `stroke.width` is used exactly as given,
/// never scaled by the transform).
///
/// Cap mapping (design §3): `LineCap::Round` → `cap_flags 0`
/// (round-round — the Scene IR's `Stroke.cap` is one style for the
/// whole segment, not per-endpoint, so there's no finer round/butt
/// split to preserve). `LineCap::Butt` → `cap_flags 3` (butt-both).
/// `LineCap::Square` is a pre-existing gap shared with `uzor-urx-cpu`
/// (neither reference implementation renders square caps today) —
/// degrades to Round, counted `native_line_square_cap_to_round`.
fn encode_line(
    frame: &mut EncodedFrame,
    from: Vec2,
    to: Vec2,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        BrushKind::Gradient => degrade("native_line_gradient_to_solid"),
        BrushKind::Image => degrade("native_line_image_to_solid"),
        BrushKind::Solid => {}
    }

    let from_p = transform_point_full(transform, Point::new(from.x, from.y));
    let to_p = transform_point_full(transform, Point::new(to.x, to.y));
    let start = [from_p.x as f32, from_p.y as f32];
    let end = [to_p.x as f32, to_p.y as f32];
    let width = stroke.width.max(0.0); // §0.2: device-constant, no scale multiply
    if width <= 0.0 {
        return;
    }

    let cap_flags = match stroke.cap {
        LineCap::Round => 0.0,
        LineCap::Butt => 3.0,
        LineCap::Square => {
            degrade("native_line_square_cap_to_round");
            0.0
        }
    };

    frame.push_line(LineInstance {
        start,
        end,
        color: packed_color(color),
        width,
        cap_flags,
        _pad0: 0.0,
        _pad1: [0.0; 2],
        clip_rect,
    });
}

/// Emit every triangle of a (already tessellated, LOCAL-space) mesh as
/// a flat-shaded `TriInstance` (`color0 == color1 == color2`),
/// re-projected through the frame's FULL affine transform (design
/// §5.4, Wave 4 Commit 4 — was translate+scale-only through Commit 3).
fn emit_solid_mesh(
    frame: &mut EncodedFrame,
    mesh: &crate::tessellate::TessMesh,
    transform: &Affine,
    packed: u32,
    clip_rect: [f32; 4],
) {
    for tri in &mesh.triangles {
        frame.push_triangle(TriInstance {
            v0: project_local(tri[0], transform),
            v1: project_local(tri[1], transform),
            v2: project_local(tri[2], transform),
            color0: packed,
            color1: packed,
            color2: packed,
            _pad0: 0.0,
            clip_rect,
        });
    }
}

/// Routing (design §2.5): `Gradient` (any kind) routes through
/// `emit_gradient_mesh`, fed the SAME mesh `get_or_insert_fill` already
/// builds for the solid-fill case — zero marginal pipeline cost,
/// closing `native_fillpath_gradient_to_solid` entirely.
fn encode_fill_path(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    lut: Option<&mut GradientLutAtlas>,
    path: &BezPath,
    rule: FillRule,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    if let Brush::Gradient(g) = brush {
        let mesh = tess_cache.get_or_insert_fill(path, rule);
        emit_gradient_mesh(frame, lut, &mesh, g, transform, clip_rect);
        return;
    }
    let (color, kind) = resolve_brush_color(brush);
    if matches!(kind, BrushKind::Image) {
        degrade("native_fillpath_image_to_solid");
    }
    let mesh = tess_cache.get_or_insert_fill(path, rule);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

/// Routing (design §2.5): `Gradient` (any kind) routes through
/// `emit_gradient_mesh`, fed the SAME mesh `get_or_insert_stroke`
/// already builds for the solid-stroke case, closing
/// `native_strokepath_gradient_to_solid` entirely. Always tessellated
/// (no similarity-vs-shear branch — a path has no bbox-based fast path
/// to begin with), so it always goes through `tess_stroke_scaled`'s
/// width unification (design §0.2).
fn encode_stroke_path(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    lut: Option<&mut GradientLutAtlas>,
    path: &BezPath,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    if stroke.width <= 0.0 {
        return;
    }
    if let Brush::Gradient(g) = brush {
        let mesh = tess_stroke_scaled(tess_cache, path, stroke, transform);
        emit_gradient_mesh(frame, lut, &mesh, g, transform, clip_rect);
        return;
    }
    let (color, kind) = resolve_brush_color(brush);
    if matches!(kind, BrushKind::Image) {
        degrade("native_strokepath_image_to_solid");
    }
    let mesh = tess_stroke_scaled(tess_cache, path, stroke, transform);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

/// `GlyphRun` → one `GlyphInstance` per successfully-placed glyph
/// (design §6). Rasterises through `uzor_urx_glyph::rasterise_glyph` —
/// the SAME function `uzor-urx-cpu`'s `GlyphRun` arm uses via
/// `draw_glyph_run` — so both backends composite byte-identical swash
/// bitmaps; they diverge only in what happens to that bitmap
/// afterward (bilinear GPU sampling + MSAA here vs. a direct 1:1 texel
/// copy on CPU).
///
/// **Translate-ONLY transform** — deliberately matches
/// `uzor-urx-cpu`'s own `GlyphRun` arm (`uzor-urx-cpu/src/backend.rs:
/// 209-210`), which extracts ONLY `coeffs[4]/[5]` and never reads
/// scale at all for text (unlike Quad/Line/Path's translate+SCALE
/// decomposition elsewhere in this file). Matched here on purpose so
/// Wave 2 does not introduce a NEW cross-backend divergence by being
/// "more correct" on just one backend — a future wave that gives CPU
/// real transform-aware text scaling must update this arm too, or
/// reintroduce the gap.
///
/// **`atlas: None` + a glyph that successfully rasterises**: counted
/// under the same `native_glyph_atlas_full_this_frame` label as a
/// genuine full-atlas miss (both mean "no atlas slot backs this
/// glyph, for whatever reason") rather than panicking — this state is
/// unreachable from `NativeUrxRenderer::render_into_encoder` (always
/// supplies `Some`) and never exercised by this crate's own tests
/// either (every test that reaches a successful rasterise supplies a
/// real, device-backed atlas), but "never happens in practice" is not
/// a licence to panic on scene content, matching this file's
/// established policy for every other primitive.
fn encode_glyph_run(
    frame: &mut EncodedFrame,
    atlas: Option<&mut NativeGlyphAtlas>,
    font: FontId,
    font_size: f32,
    glyphs: &[Glyph],
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
    text_gamma_enabled: bool,
) {
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        // Same sibling pattern as Line/StrokeRect/FillPath/StrokePath
        // (Wave 1 Commit 4) — a GlyphRun's `brush: Brush` is the same
        // general type, so it MAY carry a gradient/image brush even
        // though no real producer does yet; never a silent gap.
        BrushKind::Gradient => degrade("native_glyphrun_gradient_to_solid"),
        BrushKind::Image => degrade("native_glyphrun_image_to_solid"),
        BrushKind::Solid => {}
    }
    let packed = packed_color(color); // STRAIGHT, non-premultiplied — the shader premultiplies (design §5)

    // URX text-gamma design, 2026-07-26, §2.5a — bin computed ONCE per
    // run (loop-invariant across every glyph, exactly like the CPU
    // side's own `gamma_bin` in `draw_glyph_run`), via the SAME pure
    // `luma_bin` function both backends call independently — this is
    // what makes CPU/GPU bin selection identical BY CONSTRUCTION, not
    // just by coincidence of matching formulas. Bin 0 (identity row)
    // when the flag is off.
    let gamma_bin: f32 = if text_gamma_enabled {
        let rgba = color.to_rgba8();
        uzor_urx_core::text_gamma::luma_bin([rgba.r, rgba.g, rgba.b, rgba.a]) as f32
    } else {
        0.0
    };

    let coeffs = transform.as_coeffs();
    let (tx, ty) = (coeffs[4] as f32, coeffs[5] as f32);

    let mut atlas = atlas;
    for g in glyphs {
        let px = tx + g.x;
        let py = ty + g.y;
        let subpx = uzor_urx_glyph::subpixel_bin_for_x(px);
        let key = uzor_urx_glyph::GlyphKey::new(font, g.glyph_id, font_size, subpx);

        let bitmap = match uzor_urx_glyph::rasterise_glyph(font, g.glyph_id as u16, font_size, subpx) {
            Ok(bm) => bm,
            Err(_) => {
                // Per-glyph skip, run continues for the rest — an
                // intentional, documented improvement over CPU's own
                // `draw_glyph_run`, which returns on the FIRST failing
                // glyph via `?` and silently abandons the rest of the
                // run (design §8).
                degrade("native_glyph_rasterise_failed");
                continue;
            }
        };
        if bitmap.width == 0 || bitmap.height == 0 {
            continue; // whitespace glyph — nothing to draw, not a degrade
        }

        let Some(uv_rect) = atlas.as_deref_mut().and_then(|a| a.get_or_insert(key, &bitmap)) else {
            degrade("native_glyph_atlas_full_this_frame");
            continue;
        };

        // dst_x0/dst_y0: copied line-for-line from
        // `uzor_urx_glyph::draw_glyph_run` (`lib.rs:254,257`) — x
        // FLOORS, y ROUNDS (swash's `top` is glyph-height-above-baseline;
        // bitmap top-left = baseline_y - top). Getting this asymmetry
        // backwards would shift every glyph, uniformly, by up to 1px
        // (design risk 4).
        let dst_x0 = px.floor() + bitmap.left as f32;
        let dst_y0 = py.round() - bitmap.top as f32;

        frame.push_glyph(GlyphInstance {
            pos: [dst_x0, dst_y0],
            size: [bitmap.width as f32, bitmap.height as f32],
            uv_pos: [uv_rect[0], uv_rect[1]],
            uv_size: [uv_rect[2], uv_rect[3]],
            color: packed,
            _pad0: gamma_bin,
            clip_rect,
        });
    }
    // Same counter + semantic ("glyph instances composited")
    // `uzor_urx_glyph::draw_glyph_run` already increments (design §6) —
    // reused rather than inventing a second key. Counts the whole
    // run's glyph count, not just the successfully-placed ones (same
    // shape as the CPU-side call this mirrors).
    metrics::counter!(KEY_RENDER_GLYPH_INSTANCES).increment(glyphs.len() as u64);
}

/// `DrawCommand::Image` → `ImageInstance` (design §4.3). Resolves the
/// registered image via the SHARED `uzor_urx_image` registry (no
/// per-backend fork of `ImageId` allocation, design §0.4) — an
/// unregistered (or since-unregistered) id is a clean, honest miss,
/// mirroring `uzor-urx-cpu::image_draw`'s own policy: no `ImageInstance`
/// is ever emitted for it, and the SAME UNPREFIXED `image_id_unknown`
/// counter CPU already uses fires here too (design §4.4's naming-
/// consistency recommendation, same precedent as
/// `gradient_radial_focal_degraded`).
///
/// ## Placement math — why `pos`/`size` are NOT simply "scale each
/// corner, drop rotation"
///
/// A naive port of `transform_rect`'s recipe (scale+translate each
/// CORNER independently, attach `rotation` as an afterthought) is
/// subtly WRONG for a non-trivial `dest` under a genuine rotation: it
/// places the quad's CENTER at `S*dest_center + t` (scale+translate
/// only) rather than at the TRUE, fully-transformed center
/// `R*S*dest_center + t` — these differ by `(I - R) * S * dest_center`,
/// zero only when there's no rotation or `dest` is centered at the
/// local origin. The CORRECT construction (verified by direct
/// substitution against the full affine `T(p) = R*S*p + t`):
/// 1. `size = [dest.width()*sx, dest.height()*sy]` — scale-magnitude
///    only, no rotation (matches `ImageInstance.size`'s own "pre-
///    rotation local extents" contract).
/// 2. `center = transform_point_full(transform, dest's own center)` —
///    the TRUE center, via the FULL affine (this is where the
///    rotation's effect on the CENTER'S POSITION is captured exactly).
/// 3. `pos = center - size * 0.5`.
/// 4. The shader then rotates `size`-scaled LOCAL offsets around
///    `center` (this module's `IMAGE_SHADER_NATIVE`, same technique as
///    Quad SDF's own extension, §5.3) — captures the rotation's effect
///    on the shape's OWN extent exactly.
/// Substituting confirms `pos + rotate(uv_local*size - size*0.5) ==
/// R*S*corner + t` for every corner — an EXACT match to the full
/// affine, not an approximation, for the genuine-similarity case.
fn encode_image(
    frame: &mut EncodedFrame,
    src: ImageId,
    src_rect: Option<Rect>,
    dest: Rect,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let Some(data) = uzor_urx_image::lookup_image(src) else {
        degrade("image_id_unknown");
        return;
    };

    let img_w = data.width.max(1) as f64;
    let img_h = data.height.max(1) as f64;
    let src_box = src_rect.unwrap_or(Rect::new(0.0, 0.0, img_w, img_h));
    if src_box.width() <= 0.0 || src_box.height() <= 0.0 {
        return;
    }
    let uv_pos = [(src_box.x0 / img_w) as f32, (src_box.y0 / img_h) as f32];
    let uv_size = [(src_box.width() / img_w) as f32, (src_box.height() / img_h) as f32];

    // Similarity-vs-shear routing (design §4.3/§5.2) — introduced here
    // for Image's exclusive use this commit; `decompose_similarity`
    // itself is pure geometry with no Image-specific assumption baked
    // in, so Commit 4 can wire FillRect/StrokeRect's OWN routing
    // through this SAME function rather than a duplicate.
    let (sx, sy, angle) = match decompose_similarity(transform) {
        Some((sx, sy, angle, _, _)) => (sx, sy, angle),
        None => {
            // Shear or reflection — approximate as rotation-only,
            // dropping the shear component (design §4.3), rather than
            // building a whole parallel textured-triangle pipeline for
            // a shape with zero current producers (§0.4's grep).
            degrade("native_image_shear_to_rotation_approx");
            let (sx, sy, angle, _, _) = decompose_similarity_raw(transform);
            (sx, sy, angle)
        }
    };

    let w = dest.width() * sx;
    let h = dest.height() * sy;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let dest_center = Point::new(dest.x0 + dest.width() * 0.5, dest.y0 + dest.height() * 0.5);
    let center = transform_point_full(transform, dest_center);
    let pos = [(center.x - w * 0.5) as f32, (center.y - h * 0.5) as f32];

    frame.push_image(
        ImageInstance {
            pos,
            size: [w as f32, h as f32],
            uv_pos,
            uv_size,
            rotation: angle as f32,
            // Opaque white, straight — a no-op multiply (design §4.3:
            // "reserved... no producer sets a tint yet").
            tint: 0xFFFF_FFFF,
            clip_rect,
        },
        src,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::scene::Stroke as SceneStroke;

    fn viewport() -> Viewport {
        Viewport { width: 100, height: 100 }
    }

    fn cache() -> TessCache {
        TessCache::new()
    }

    /// `UrxConfig::default().blend_layer_max_depth` (design §3.3) — every
    /// pre-Wave-3-Commit-3 test in this module encodes zero blend layers,
    /// so the exact cap value is inert; kept as a named helper (mirroring
    /// `viewport()`/`cache()`) rather than a bare repeated literal so the
    /// handful of NEW depth-cap tests below can call it with a different
    /// value without hunting down every other call site.
    fn max_depth() -> usize {
        8
    }

    #[test]
    fn fill_rect_solid_emits_one_quad() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(10.0, 10.0, 50.0, 50.0), Color::from_rgba8(255, 0, 0, 255));
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 1);
        assert_eq!(frame.quads[0].pos, [10.0, 10.0]);
        assert_eq!(frame.quads[0].size, [40.0, 40.0]);
        assert_eq!(frame.quads[0].border_width, 0.0);
    }

    #[test]
    fn fill_rect_with_uniform_radii_sets_corner_radius_without_degrade() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: Some([8.0, 8.0, 8.0, 8.0]),
            brush: Brush::Solid(Color::from_rgba8(0, 255, 0, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 1);
        assert!((frame.quads[0].corner_radius - 8.0).abs() < 0.01);
    }

    // ── Wave 4 Commit 4: full affine + stroke-width unification + ────
    // ── per-corner radii routing ──────────────────────────────────────

    /// Design §0.2's central regression proof: a `StrokeRect`'s
    /// `border_width` must be IDENTICAL at 1x and 2x scale (device-
    /// space-constant), even though the rect's own `size` legitimately
    /// scales — matching CPU's semantic (width applied AFTER
    /// transforming points, never itself scaled).
    #[test]
    fn stroke_rect_border_width_stays_device_constant_at_2x_scale() {
        let stroke_rect = |transform: Affine| DrawCommand::StrokeRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            stroke: SceneStroke { width: 3.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 255, 255)),
            transform,
        };

        let mut scene_1x = Scene::new();
        scene_1x.push(stroke_rect(Affine::IDENTITY));
        let frame_1x = encode_scene(&scene_1x, viewport(), &mut cache(), None, None, max_depth(), false);

        let mut scene_2x = Scene::new();
        scene_2x.push(stroke_rect(Affine::scale(2.0)));
        let frame_2x = encode_scene(&scene_2x, viewport(), &mut cache(), None, None, max_depth(), false);

        assert_eq!(frame_1x.quads.len(), 1);
        assert_eq!(frame_2x.quads.len(), 1);
        assert!((frame_1x.quads[0].border_width - 3.0).abs() < 0.01);
        assert!(
            (frame_1x.quads[0].border_width - frame_2x.quads[0].border_width).abs() < 0.01,
            "border_width must be DEVICE-CONSTANT regardless of transform scale: 1x={} 2x={}",
            frame_1x.quads[0].border_width,
            frame_2x.quads[0].border_width
        );
        // The rect ITSELF still scales — only the stroke width doesn't.
        assert_eq!(frame_2x.quads[0].size, [80.0, 80.0]);
    }

    /// Design §5.2/§5.3: a similarity transform (pure rotation, no
    /// shear/reflection) with UNIFORM radii stays on the fast Quad SDF
    /// path — batch inspection proves it never falls through to the
    /// Triangle pipeline, and the rotation angle lands in `_pad0[0]`.
    #[test]
    fn rotated_uniform_radius_rect_routes_through_quad_sdf_not_triangle() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 30.0, 30.0),
            radii: Some([4.0, 4.0, 4.0, 4.0]),
            brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
            transform: Affine::rotate(std::f64::consts::FRAC_PI_4),
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 1, "a similarity transform with UNIFORM radii must stay on Quad SDF");
        assert!(frame.triangles.is_empty(), "must NOT route through the Triangle pipeline");
        assert!(
            (frame.quads[0]._pad0[0] - std::f64::consts::FRAC_PI_4 as f32).abs() < 0.001,
            "rotation angle must be baked into _pad0[0]: got {}",
            frame.quads[0]._pad0[0]
        );
    }

    #[test]
    fn stroke_rect_emits_transparent_fill_with_border() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokeRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            stroke: SceneStroke { width: 3.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 1);
        assert_eq!(frame.quads[0].color, 0);
        assert!((frame.quads[0].border_width - 3.0).abs() < 0.01);
        assert_ne!(frame.quads[0].border_color, 0);
    }

    #[test]
    fn zero_width_stroke_emits_nothing() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokeRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            stroke: SceneStroke { width: 0.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.quads.is_empty());
    }

    #[test]
    fn degenerate_rect_is_skipped_not_panicked() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.quads.is_empty());
    }

    #[test]
    fn line_solid_emits_one_line_not_a_quad() {
        let mut scene = Scene::new();
        scene.line_solid(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 10.0 },
            2.0,
            Color::from_rgba8(255, 255, 255, 255),
        );
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.quads.is_empty());
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.lines[0].start, [0.0, 0.0]);
        assert_eq!(frame.lines[0].end, [10.0, 10.0]);
        assert!((frame.lines[0].width - 2.0).abs() < 0.01);
    }

    /// Wave 4 Commit 4 (design §0.2): stroke width is now DEVICE-
    /// CONSTANT under scale, matching CPU's own semantic — this test
    /// used to assert the OPPOSITE (width scaling 2x with the
    /// transform), which was the pre-Commit-4, GPU-only-consistent-with-
    /// itself behavior the design doc explicitly requires changing.
    /// Endpoints still transform through the full affine, unaffected.
    #[test]
    fn line_width_stays_device_constant_under_scale() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 4.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::scale(2.0),
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.lines[0].end, [20.0, 0.0], "endpoints still transform through the full affine");
        assert!(
            (frame.lines[0].width - 4.0).abs() < 0.01,
            "stroke width must stay DEVICE-CONSTANT under scale (design §0.2), not scale to 8.0"
        );
    }

    #[test]
    fn zero_width_line_emits_nothing() {
        let mut scene = Scene::new();
        scene.line_solid(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 10.0 },
            0.0,
            Color::from_rgba8(255, 255, 255, 255),
        );
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.lines.is_empty());
    }

    #[test]
    fn cap_round_maps_to_flag_zero() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Round, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.lines[0].cap_flags, 0.0);
    }

    #[test]
    fn cap_butt_maps_to_flag_three() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Butt, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.lines[0].cap_flags, 3.0);
    }

    #[test]
    fn cap_square_degrades_to_round_flag_zero() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Square, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        // Degrades to round (flag 0) — counted via `native_line_square_cap_to_round`
        // (asserting on the global metrics recorder isn't wired in this
        // crate's unit tests; the geometry-level effect is what we can verify).
        assert_eq!(frame.lines[0].cap_flags, 0.0);
    }

    #[test]
    fn batches_coalesce_by_kind_and_preserve_painters_order() {
        // rect, rect, line, rect -> 3 batches: [Quad x2, Line x1, Quad x1].
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.fill_rect_solid(Rect::new(20.0, 0.0, 30.0, 10.0), Color::from_rgba8(0, 255, 0, 255));
        scene.line_solid(
            Vec2 { x: 0.0, y: 20.0 },
            Vec2 { x: 30.0, y: 20.0 },
            2.0,
            Color::from_rgba8(0, 0, 255, 255),
        );
        scene.fill_rect_solid(Rect::new(0.0, 30.0, 10.0, 40.0), Color::from_rgba8(255, 255, 0, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 3);
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.draw_batches().len(), 3, "batches: {:?}", frame.draw_batches());

        assert_eq!(frame.draw_batches()[0].kind, BatchKind::Quad);
        assert_eq!(frame.draw_batches()[0].start, 0);
        assert_eq!(frame.draw_batches()[0].count, 2);

        assert_eq!(frame.draw_batches()[1].kind, BatchKind::Line);
        assert_eq!(frame.draw_batches()[1].start, 0);
        assert_eq!(frame.draw_batches()[1].count, 1);

        assert_eq!(frame.draw_batches()[2].kind, BatchKind::Quad);
        assert_eq!(frame.draw_batches()[2].start, 2, "third batch must resume at quad index 2, not restart at 0");
        assert_eq!(frame.draw_batches()[2].count, 1);
    }

    #[test]
    fn alternating_kinds_never_coalesce_across_a_gap() {
        // rect, line, rect, line -> 4 distinct batches, never merged.
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 }, 2.0, Color::from_rgba8(0, 0, 255, 255));
        scene.fill_rect_solid(Rect::new(0.0, 20.0, 10.0, 30.0), Color::from_rgba8(0, 255, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 40.0 }, Vec2 { x: 10.0, y: 40.0 }, 2.0, Color::from_rgba8(255, 255, 0, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.draw_batches().len(), 4);
        let kinds: Vec<BatchKind> = frame.draw_batches().iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Line, BatchKind::Quad, BatchKind::Line]);
    }

    #[test]
    fn fill_path_triangle_emits_triangles_and_batches() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));
        path.line_to((10.0, 20.0));
        path.close_path();

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(!frame.triangles.is_empty());
        assert_eq!(frame.draw_batches().len(), 1);
        assert_eq!(frame.draw_batches()[0].kind, BatchKind::Triangle);
        for tri in &frame.triangles {
            assert_eq!(tri.color0, tri.color1);
            assert_eq!(tri.color1, tri.color2);
        }
    }

    #[test]
    fn stroke_path_zero_width_emits_nothing() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));

        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokePath {
            path,
            stroke: SceneStroke { width: 0.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.triangles.is_empty());
    }

    #[test]
    fn quad_line_triangle_batches_interleave_correctly() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));
        path.line_to((10.0, 20.0));
        path.close_path();

        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 }, 2.0, Color::from_rgba8(0, 0, 255, 255));
        scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.draw_batches().len(), 3);
        let kinds: Vec<BatchKind> = frame.draw_batches().iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Line, BatchKind::Triangle]);
    }

    #[test]
    fn gradient_vertex_projection_matches_analytic_t() {
        // Axis from (0,0) to (10,0); a vertex at x=5 should project to t=0.5.
        let pos = LinearGradientPosition { start: kurbo::Point::new(0.0, 0.0), end: kurbo::Point::new(10.0, 0.0) };
        let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
        let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;
        let stops = ColorStops::from(
            &[
                peniko::ColorStop::from((0.0f32, Color::from_rgba8(0, 0, 0, 255))),
                peniko::ColorStop::from((1.0f32, Color::from_rgba8(255, 255, 255, 255))),
            ][..],
        );
        let mid = gradient_vertex_color([5.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        let bytes = mid.to_rgba8();
        assert!((bytes.r as i32 - 128).abs() <= 2, "midpoint should be ~50% gray, got r={}", bytes.r);

        let start_color = gradient_vertex_color([0.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(start_color.to_rgba8().r, 0);

        let end_color = gradient_vertex_color([10.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(end_color.to_rgba8().r, 255);
    }

    #[test]
    fn gradient_pad_clamps_outside_axis() {
        let pos = LinearGradientPosition { start: kurbo::Point::new(0.0, 0.0), end: kurbo::Point::new(10.0, 0.0) };
        let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
        let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;
        let stops = ColorStops::from(
            &[
                peniko::ColorStop::from((0.0f32, Color::from_rgba8(0, 0, 0, 255))),
                peniko::ColorStop::from((1.0f32, Color::from_rgba8(255, 255, 255, 255))),
            ][..],
        );
        // Past the end of the axis — Pad should clamp to the last stop.
        let beyond = gradient_vertex_color([50.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(beyond.to_rgba8().r, 255);
        // Before the start — Pad should clamp to the first stop.
        let before = gradient_vertex_color([-50.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(before.to_rgba8().r, 0);
    }

    // ── Wave 4 Commit 3: Radial/Sweep gradients ──────────────────────

    fn two_stop_gradient_stops() -> ColorStops {
        ColorStops::from(
            &[
                peniko::ColorStop::from((0.0f32, Color::from_rgba8(255, 0, 0, 255))),
                peniko::ColorStop::from((1.0f32, Color::from_rgba8(0, 0, 255, 255))),
            ][..],
        )
    }

    fn radial_gradient_brush() -> Brush {
        let mut g = Gradient::new_radial(kurbo::Point::new(20.0, 20.0), 15.0);
        g.stops = two_stop_gradient_stops();
        g.extend = Extend::Pad;
        Brush::Gradient(g)
    }

    fn sweep_gradient_brush() -> Brush {
        let mut g = Gradient::new_sweep(kurbo::Point::new(20.0, 20.0), -std::f32::consts::PI, std::f32::consts::PI);
        g.stops = two_stop_gradient_stops();
        g.extend = Extend::Pad;
        Brush::Gradient(g)
    }

    /// Without a real `GradientLutAtlas` (`lut: None`, the shape every
    /// pre-Wave-4-Commit-3 test in this module already uses), a
    /// Radial/Sweep `FillRect` must NOT silently vanish — it falls back
    /// to the SAME first-stop-solid `emit_solid_mesh` path every other
    /// "can't render the real thing" gradient case in this crate uses,
    /// counted `native_gradient_lut_full_this_frame`. Device-free —
    /// this is the `lut: None` defensive branch
    /// (`emit_gradient_lut_triangles`'s own doc comment), not the
    /// real-hardware path (see the `#[ignore]`-gated test below for
    /// that).
    #[test]
    fn radial_gradient_fillrect_falls_back_to_solid_without_a_lut() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            brush: radial_gradient_brush(),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.gradients.is_empty(), "no GradientInstance without a real LUT atlas");
        assert!(!frame.triangles.is_empty(), "must still render SOMETHING — the solid fallback mesh");
    }

    /// Radial/Sweep `FillRect` emits a `GradientInstance` with plausible
    /// device-space params, given a REAL `GradientLutAtlas` (design §10
    /// Commit 3's own test scope). `#[ignore]`-gated — needs a headless
    /// GPU adapter to construct the atlas at all.
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn radial_and_sweep_fillrect_emit_gradient_instance_with_plausible_device_params() {
        let Some((device, _queue)) = test_device() else { return };
        let mut lut = GradientLutAtlas::new(&device, 8);
        lut.begin_frame();

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            brush: radial_gradient_brush(),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(50.0, 0.0, 90.0, 40.0),
            radii: None,
            brush: sweep_gradient_brush(),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, Some(&mut lut), max_depth(), false);

        assert!(frame.triangles.is_empty(), "Radial/Sweep must NOT land on the flat-shaded triangle path");
        assert!(!frame.gradients.is_empty(), "a real LUT atlas must produce real GradientInstances");

        // Radial instance: kind bits == 0, center near (20,20), radius
        // near 15 (identity transform — device space == local space).
        let radial = frame.gradients[0];
        assert_eq!(radial.kind_extend & 3, 0, "first FillRect's gradient is Radial (kind bits 0)");
        assert!((radial.p0[0] - 20.0).abs() < 0.5 && (radial.p0[1] - 20.0).abs() < 0.5, "center ~= (20,20), got {:?}", radial.p0);
        assert!((radial.p1 - 15.0).abs() < 0.5, "radius ~= 15, got {}", radial.p1);

        // Sweep instance: kind bits == 1, center near (20,20) (LOCAL to
        // the second rect, but the gradient's own center coordinate is
        // (20,20) regardless of which rect it's drawn under), angles
        // near -PI/PI.
        let sweep = frame.gradients.iter().find(|g| g.kind_extend & 3 == 1).expect("a Sweep instance must exist");
        assert!((sweep.p0[0] - 20.0).abs() < 0.5 && (sweep.p0[1] - 20.0).abs() < 0.5, "center ~= (20,20), got {:?}", sweep.p0);
        assert!((sweep.p1 + std::f32::consts::PI).abs() < 0.01, "start_angle ~= -PI, got {}", sweep.p1);
        assert!((sweep.p2 - std::f32::consts::PI).abs() < 0.01, "end_angle ~= PI, got {}", sweep.p2);
    }

    /// Gradient on `FillPath`/`StrokePath`/`StrokeRect` all emit
    /// `GradientInstance`s through the SAME `emit_gradient_mesh` code
    /// path as `FillRect` — a regression-proof that the generalization
    /// (design §2.4/§2.5) didn't quietly fork per-command (design §10
    /// Commit 3's own test scope).
    ///
    /// Each command is encoded into its OWN scene (rather than one
    /// combined scene) so a per-command emptiness check independently
    /// proves that command's own route works — three back-to-back
    /// `Gradient` batches at the same clip depth would legitimately
    /// coalesce into one under `bump_batch`'s equality rule (same as
    /// any other batch kind), so asserting on batch COUNT would be
    /// asserting against `bump_batch`'s own, correct, coalescing
    /// behavior rather than against a real regression.
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn gradient_on_fillpath_strokepath_strokerect_all_emit_gradient_instances() {
        let Some((device, _queue)) = test_device() else { return };

        let mut triangle = BezPath::new();
        triangle.move_to((0.0, 0.0));
        triangle.line_to((20.0, 0.0));
        triangle.line_to((10.0, 20.0));
        triangle.close_path();

        let fill_scene = {
            let mut scene = Scene::new();
            scene.push(DrawCommand::FillPath {
                path: triangle.clone(),
                rule: FillRule::NonZero,
                brush: radial_gradient_brush(),
                transform: Affine::IDENTITY,
            });
            scene
        };
        let stroke_path_scene = {
            let mut scene = Scene::new();
            scene.push(DrawCommand::StrokePath {
                path: triangle,
                stroke: SceneStroke { width: 2.0, ..SceneStroke::default() },
                brush: radial_gradient_brush(),
                transform: Affine::IDENTITY,
            });
            scene
        };
        let stroke_rect_scene = {
            let mut scene = Scene::new();
            scene.push(DrawCommand::StrokeRect {
                rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                radii: None,
                stroke: SceneStroke { width: 2.0, ..SceneStroke::default() },
                brush: radial_gradient_brush(),
                transform: Affine::IDENTITY,
            });
            scene
        };

        for (label, scene) in [
            ("FillPath", &fill_scene),
            ("StrokePath", &stroke_path_scene),
            ("StrokeRect", &stroke_rect_scene),
        ] {
            let mut lut = GradientLutAtlas::new(&device, 8);
            lut.begin_frame();
            let frame = encode_scene(scene, viewport(), &mut cache(), None, Some(&mut lut), max_depth(), false);
            assert!(frame.triangles.is_empty(), "{label} must route through the Gradient path, not the solid one");
            assert!(!frame.gradients.is_empty(), "{label} must emit real GradientInstances");
            assert_eq!(
                frame.draw_batches().iter().map(|b| b.kind).collect::<Vec<_>>(),
                vec![BatchKind::Gradient],
                "{label} alone must produce exactly one Gradient batch"
            );
        }

        // Combined scene: back-to-back gradient draws at the same clip
        // depth legitimately coalesce into ONE batch (proves
        // `bump_batch`'s coalescing applies to `Gradient` exactly like
        // every other batch kind), while still carrying all 3 shapes'
        // worth of instances.
        let mut combined = Scene::new();
        for scene in [&fill_scene, &stroke_path_scene, &stroke_rect_scene] {
            combined.commands.extend(scene.commands.iter().cloned());
        }
        let mut lut = GradientLutAtlas::new(&device, 8);
        lut.begin_frame();
        let frame = encode_scene(&combined, viewport(), &mut cache(), None, Some(&mut lut), max_depth(), false);
        assert!(frame.triangles.is_empty(), "combined scene must route through the Gradient path, not the solid one");
        let batches = frame.draw_batches();
        assert_eq!(batches.len(), 1, "3 back-to-back Gradient draws at the same clip depth must coalesce into 1 batch");
        assert_eq!(batches[0].kind, BatchKind::Gradient);
        assert_eq!(batches[0].count, frame.gradients.len() as u32, "the coalesced batch must cover every emitted instance");
    }

    // ── Wave 4 Commit 3: DrawCommand::Image ───────────────────────────

    fn register_test_image(w: u32, h: u32, fill: [u8; 4]) -> ImageId {
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        for px in bytes.chunks_exact_mut(4) {
            px.copy_from_slice(&fill);
        }
        let data = uzor_urx_image::ImageData::from_raw_premul(w, h, bytes).expect("size matches by construction");
        uzor_urx_image::register_image(data)
    }

    /// `BatchKind::Image`'s batch key includes the `ImageId` (design
    /// §4.3): two DIFFERENT ids never coalesce into one batch (each
    /// needs its own bind group), but two draws of the SAME id DO
    /// coalesce, via the exact same `bump_batch` equality check every
    /// other batch kind already uses.
    #[test]
    fn image_batch_key_includes_id_distinct_ids_split_same_id_coalesces() {
        let id_a = register_test_image(2, 2, [255, 0, 0, 255]);
        let id_b = register_test_image(2, 2, [0, 255, 0, 255]);

        let mut scene = Scene::new();
        scene.push(DrawCommand::Image {
            src: id_a,
            src_rect: None,
            dest: Rect::new(0.0, 0.0, 10.0, 10.0),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::Image {
            src: id_a,
            src_rect: None,
            dest: Rect::new(10.0, 0.0, 20.0, 10.0),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::Image {
            src: id_b,
            src_rect: None,
            dest: Rect::new(20.0, 0.0, 30.0, 10.0),
            transform: Affine::IDENTITY,
        });

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.images.len(), 3, "all 3 draws must still emit their own ImageInstance");
        let batches = frame.draw_batches();
        assert_eq!(
            batches.len(),
            2,
            "the two SAME-id (id_a) draws must coalesce into ONE batch; the id_b draw must be a SEPARATE batch"
        );
        assert_eq!(batches[0].kind, BatchKind::Image(id_a));
        assert_eq!(batches[0].count, 2, "both id_a draws coalesced");
        assert_eq!(batches[1].kind, BatchKind::Image(id_b));
        assert_eq!(batches[1].count, 1);

        uzor_urx_image::unregister_image(id_a);
        uzor_urx_image::unregister_image(id_b);
    }

    // ── Commit 4: ClipStack ─────────────────────────────────────────

    #[test]
    fn clip_stack_root_is_full_viewport_and_never_pops() {
        let mut clip = ClipStack::new(viewport());
        assert_eq!(clip.current(), [0.0, 0.0, 100.0, 100.0]);
        clip.pop(); // no matching push — must be a no-op, not a panic.
        assert_eq!(clip.current(), [0.0, 0.0, 100.0, 100.0]);
    }

    #[test]
    fn clip_stack_push_intersects_with_current_top() {
        let mut clip = ClipStack::new(viewport());
        clip.push_rect_device([10.0, 10.0, 60.0, 60.0]); // -> [10,10,60,60] (root is [0,0,100,100])
        assert_eq!(clip.current(), [10.0, 10.0, 60.0, 60.0]);
        // Nested push partially outside the current top — must clip to
        // the INTERSECTION, not the pushed rect verbatim.
        clip.push_rect_device([40.0, 40.0, 60.0, 60.0]); // pushed = [40,40]..[100,100]; current = [10,10]..[70,70]
        assert_eq!(clip.current(), [40.0, 40.0, 30.0, 30.0]); // intersection = [40,40]..[70,70]
        clip.pop();
        assert_eq!(clip.current(), [10.0, 10.0, 60.0, 60.0], "pop must restore the PREVIOUS top exactly");
    }

    #[test]
    fn clip_stack_disjoint_push_yields_zero_area() {
        let mut clip = ClipStack::new(viewport());
        clip.push_rect_device([0.0, 0.0, 10.0, 10.0]);
        clip.push_rect_device([50.0, 50.0, 10.0, 10.0]); // no overlap with [0,0,10,10]
        let cur = clip.current();
        assert!(cur[2] <= 0.0 || cur[3] <= 0.0, "disjoint push must intersect to zero area: {cur:?}");
    }

    #[test]
    fn push_clip_rect_feeds_every_subsequent_instance() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::PushClipRect {
            rect: Rect::new(10.0, 10.0, 30.0, 30.0),
            transform: Affine::IDENTITY,
        });
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 100.0, 100.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 50.0, y: 50.0 }, 2.0, Color::from_rgba8(0, 255, 0, 255));
        scene.push(DrawCommand::PopClip);
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(0, 0, 255, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads[0].clip_rect, [10.0, 10.0, 20.0, 20.0], "rect under the pushed clip");
        assert_eq!(frame.lines[0].clip_rect, [10.0, 10.0, 20.0, 20.0], "line under the pushed clip");
        assert_eq!(
            frame.quads[1].clip_rect,
            [0.0, 0.0, 100.0, 100.0],
            "rect encoded AFTER PopClip must see the full viewport again"
        );
    }

    /// Wave 3 Commit 2 (design §4): the bbox intersection still feeds
    /// the SAME plain-rect `clip_rect` mechanism every instance already
    /// carries — stencil is purely ADDITIVE precision on top, never a
    /// replacement (the `[10,10,20,20]` value is identical to what a
    /// plain `PushClipRect` at the same rect would give).
    #[test]
    fn push_clip_rounded_rect_still_feeds_bbox_into_clip_rect() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::PushClipRoundedRect {
            rect: uzor_urx_core::math::RoundedRect::from_rect(
                Rect::new(10.0, 10.0, 30.0, 30.0),
                uzor_urx_core::math::RoundedRectRadii::new(5.0, 5.0, 5.0, 5.0),
            ),
            transform: Affine::IDENTITY,
        });
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 100.0, 100.0), Color::from_rgba8(255, 0, 0, 255));
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads[0].clip_rect, [10.0, 10.0, 20.0, 20.0]);
        assert!(frame.has_rounded_clip);
    }

    /// The core stencil-ref sequencing contract (design §2.4, §9 commit
    /// 2 gate) — pure `encode_scene` output inspection, no GPU needed:
    /// `push` emits a `MaskOp::Increment` batch gated on the PARENT
    /// depth (`0`, outermost), content between push/pop is tagged
    /// `Some(1)`, `pop` emits a `MaskOp::Decrement` batch gated on THIS
    /// scope's own depth (`1`). Also proves the coalescing rule splits
    /// on `stencil_ref` change even though `Quad` batches sandwich the
    /// rounded-clip scope on both sides.
    #[test]
    fn rounded_clip_stencil_ref_sequencing_push_content_pop() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255)); // depth 0, BEFORE
        scene.push(DrawCommand::PushClipRoundedRect {
            rect: uzor_urx_core::math::RoundedRect::from_rect(
                Rect::new(10.0, 10.0, 30.0, 30.0),
                uzor_urx_core::math::RoundedRectRadii::new(5.0, 5.0, 5.0, 5.0),
            ),
            transform: Affine::IDENTITY,
        });
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 100.0, 100.0), Color::from_rgba8(255, 0, 0, 255)); // depth 1, INSIDE
        scene.push(DrawCommand::PopClip);
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(2, 2, 2, 255)); // depth 0, AFTER

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.has_rounded_clip);
        assert!(!frame.stencil_masks.is_empty(), "the mask geometry itself must have been emitted");

        let kinds: Vec<(BatchKind, Option<u32>)> = frame.draw_batches().iter().map(|b| (b.kind, b.stencil_ref)).collect();
        assert_eq!(
            kinds,
            vec![
                (BatchKind::Quad, None),                             // depth 0, before
                (BatchKind::StencilMask(MaskOp::Increment), Some(0)), // gate = parent depth (0)
                (BatchKind::Quad, Some(1)),                          // depth 1, inside
                (BatchKind::StencilMask(MaskOp::Decrement), Some(1)), // gate = this scope's own depth (1)
                (BatchKind::Quad, None),                             // depth 0, after — did NOT coalesce with the first Quad batch
            ],
            "push -> increment(gate=parent), content -> Some(depth), pop -> decrement(gate=own depth)"
        );
    }

    // The unbalanced-scene force-close test lives in
    // `metrics_recorder_proof` below (it needs `TestRecorder`, private
    // to that submodule).

    /// Port of the legacy adapter's `adapt_balances_clip_stack`
    /// (`adapter.rs:436-446`) shape, adapted to the native encoder's
    /// chosen semantics: an unbalanced `PushClipRect` with no matching
    /// `PopClip` does NOT panic, and — unlike the legacy adapter, which
    /// force-restores at scene end so nothing downstream is affected —
    /// this encoder has no "end of scene" cleanup step, so the pushed
    /// clip stays ACTIVE for every instance encoded after it, for the
    /// rest of the frame. Documented here as the chosen behaviour, not
    /// an oversight: a scene is a fixed, one-shot command list: there
    /// is no "downstream" beyond its own end to protect.
    #[test]
    fn unbalanced_push_clip_rect_stays_active_for_the_rest_of_the_frame_no_panic() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::PushClipRect {
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            transform: Affine::IDENTITY,
        });
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 100.0, 100.0), Color::from_rgba8(255, 0, 0, 255));
        // No matching PopClip.
        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(frame.quads.len(), 1);
        assert_eq!(frame.quads[0].clip_rect, [10.0, 10.0, 10.0, 10.0]);
    }

    #[test]
    fn zero_area_clip_elides_every_instance_type_without_a_degrade() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));
        path.line_to((10.0, 20.0));
        path.close_path();

        let mut scene = Scene::new();
        scene.push(DrawCommand::PushClipRect { rect: Rect::new(0.0, 0.0, 0.0, 0.0), transform: Affine::IDENTITY });
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 }, 2.0, Color::from_rgba8(0, 0, 255, 255));
        scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert!(frame.quads.is_empty());
        assert!(frame.lines.is_empty());
        assert!(frame.triangles.is_empty());
        assert!(frame.draw_batches().is_empty());
    }

    // ── Wave 3 Commit 3: FrameOp sequencing (blend layers) ───────────
    //
    // `FrameOp` deliberately does not derive `PartialEq` (its `Batch`/
    // `BlendMode` payloads carry more than these tests need to compare)
    // — fold every op into this small comparable projection instead of
    // pattern-matching by hand at every assertion site.
    #[derive(Debug, PartialEq, Eq)]
    enum OpShape {
        Draw(BatchKind),
        Push(u32),
        Pop(u32),
    }

    fn op_shapes(frame: &EncodedFrame) -> Vec<OpShape> {
        frame
            .ops
            .iter()
            .map(|op| match op {
                FrameOp::Draw(b) => OpShape::Draw(b.kind),
                FrameOp::PushLayer { depth } => OpShape::Push(*depth),
                FrameOp::PopLayer { depth, .. } => OpShape::Pop(*depth),
            })
            .collect()
    }

    fn push_blend_layer(alpha: f32) -> DrawCommand {
        DrawCommand::PushBlendLayer { mode: BlendMode::default(), alpha, transform: Affine::IDENTITY }
    }

    /// Design §3.2: `PushLayer`/`PopLayer` markers bracket a blend-layer
    /// scope exactly where the Push/Pop occurred in the scan, with
    /// content before/inside/after emitting ordinary `Draw` ops around
    /// them — and the composite instance carries the layer's own alpha.
    #[test]
    fn push_pop_blend_layer_emits_markers_bracketing_content_in_scan_order() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255)); // before
        scene.push(push_blend_layer(0.5));
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(2, 2, 2, 255)); // inside
        scene.push(DrawCommand::PopBlendLayer);
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(3, 3, 3, 255)); // after

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(
            op_shapes(&frame),
            vec![
                OpShape::Draw(BatchKind::Quad),
                OpShape::Push(1),
                OpShape::Draw(BatchKind::Quad),
                OpShape::Pop(1),
                OpShape::Draw(BatchKind::Quad),
            ],
            "before/inside/after content must NOT coalesce across the layer markers"
        );
        assert_eq!(frame.composites.len(), 1);
        assert_eq!(frame.composites[0].alpha, 0.5);
    }

    /// Design §3.2's coalescing-refusal claim, isolated: two content-free
    /// `Push`/`Pop` markers alone (no draw between them) are still enough
    /// to keep two otherwise-identical adjacent `Quad` batches from
    /// merging — `bump_batch`'s `last_mut()` check sees a marker, not a
    /// `Draw`, as the immediately-preceding op.
    #[test]
    fn coalescing_refuses_across_empty_push_pop_blend_layer_markers() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));
        scene.push(push_blend_layer(1.0));
        scene.push(DrawCommand::PopBlendLayer);
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(
            op_shapes(&frame),
            vec![OpShape::Draw(BatchKind::Quad), OpShape::Push(1), OpShape::Pop(1), OpShape::Draw(BatchKind::Quad)],
        );
        let batches = frame.draw_batches();
        assert_eq!(batches.len(), 2, "the two quad draws must NOT merge across the push/pop layer markers");
        assert_eq!(batches[0].count, 1);
        assert_eq!(batches[1].count, 1);
    }

    /// Design §3.3: pushes beyond `blend_layer_max_depth` are suppressed
    /// (no `PushLayer` emitted, content keeps drawing into whatever
    /// target was already current) and their matching pops emit no
    /// `PopLayer`/composite either — the suppressed counter alone tracks
    /// the imbalance, never `open`.
    #[test]
    fn blend_layer_depth_cap_suppresses_pushes_beyond_max_and_matching_pops_emit_nothing() {
        let mut scene = Scene::new();
        scene.push(push_blend_layer(1.0)); // depth 1 — real
        scene.push(push_blend_layer(1.0)); // depth 2 — real (cap == 2)
        scene.push(push_blend_layer(1.0)); // suppressed — cap already hit
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));
        scene.push(DrawCommand::PopBlendLayer); // matches the suppressed push — no-op
        scene.push(DrawCommand::PopBlendLayer); // matches depth 2
        scene.push(DrawCommand::PopBlendLayer); // matches depth 1

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, 2, false);
        assert_eq!(
            op_shapes(&frame),
            vec![
                OpShape::Push(1),
                OpShape::Push(2),
                OpShape::Draw(BatchKind::Quad),
                OpShape::Pop(2),
                OpShape::Pop(1),
            ],
            "the 3rd push (cap = 2) must emit no PushLayer, and its matching pop must emit no PopLayer either"
        );
        assert_eq!(frame.composites.len(), 2, "only the 2 real layers get a composite instance, not the suppressed one");
    }

    /// Design §3.3's unbalanced-scene guard, checked purely via the op
    /// stream (the degrade-counter proof for the same scenario lives in
    /// `metrics_recorder_proof` below, alongside its rounded-clip twin).
    #[test]
    fn unbalanced_push_blend_layer_force_closes_at_scene_end_with_synthetic_pop() {
        let mut scene = Scene::new();
        scene.push(push_blend_layer(0.75));
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(9, 9, 9, 255));
        // NO PopBlendLayer — deliberately unbalanced.

        let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
        assert_eq!(
            op_shapes(&frame),
            vec![OpShape::Push(1), OpShape::Draw(BatchKind::Quad), OpShape::Pop(1)],
            "a synthetic PopLayer must close the still-open layer at scene end"
        );
        assert_eq!(frame.composites.len(), 1);
        assert_eq!(frame.composites[0].alpha, 0.75);
    }

    // ── Wave 2 Commit 2: GlyphRun ─────────────────────────────────────
    //
    // The two tests below construct a real, device-backed
    // `NativeGlyphAtlas` — `encode_scene`'s `atlas` parameter only
    // does something observable (places a `GlyphInstance`, populates
    // `frame.glyphs`) once a glyph actually rasterises AND finds atlas
    // room, both of which require a real font + a real atlas. They are
    // `#[ignore]`-gated per this crate's established GPU-test
    // convention (`atlas.rs`'s own tests). The unregistered-font and
    // gradient-brush-degrade tests deliberately do NOT need any of
    // this — see `metrics_recorder_proof` below.

    /// Headless wgpu device — mirrors `atlas.rs::tests::test_device`
    /// (same wgpu-29 shape, same graceful-skip-if-no-adapter policy).
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-urx-wgpu-encode-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    /// Register a real font (DejaVuSans, same asset + glyph ids the
    /// parity fixture uses) — CPU-only, no device needed. Each call
    /// mints a fresh `FontId`; harmless to call once per test.
    fn registered_font() -> FontId {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../uzor-fonts/fonts/DejaVuSans.ttf"))
            .expect("uzor-fonts ships DejaVuSans.ttf for exactly this kind of test-only registration");
        uzor_urx_glyph::register_font(bytes).expect("DejaVuSans.ttf is a valid font")
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn glyphrun_with_registered_font_emits_plausible_instances() {
        let Some((device, queue)) = test_device() else { return };
        let mut atlas = NativeGlyphAtlas::new(&device, &queue, 256, 256);
        atlas.begin_frame();

        let font = registered_font();
        let mut scene = Scene::new();
        scene.push(DrawCommand::GlyphRun {
            glyphs: vec![Glyph { glyph_id: 36, x: 0.0, y: 0.0 }, Glyph { glyph_id: 37, x: 20.0, y: 0.0 }],
            font,
            font_size: 32.0,
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::translate((10.0, 10.0)),
            text: None,
        });

        let frame = encode_scene(&scene, viewport(), &mut cache(), Some(&mut atlas), None, max_depth(), false);
        assert_eq!(frame.glyphs.len(), 2, "both glyphs should have rasterised + placed successfully");
        for g in &frame.glyphs {
            assert!(g.size[0] > 0.0 && g.size[1] > 0.0, "a real glyph bitmap must have a positive size");
            assert!(g.uv_size[0] > 0.0 && g.uv_size[1] > 0.0, "a placed glyph must have a non-zero atlas UV rect");
        }
        assert_eq!(frame.draw_batches().len(), 1);
        assert_eq!(frame.draw_batches()[0].kind, BatchKind::Glyph);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn glyph_batches_coalesce_and_interleave_with_quad_batches() {
        let Some((device, queue)) = test_device() else { return };
        let mut atlas = NativeGlyphAtlas::new(&device, &queue, 256, 256);
        atlas.begin_frame();

        let font = registered_font();
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.push(DrawCommand::GlyphRun {
            glyphs: vec![Glyph { glyph_id: 36, x: 0.0, y: 0.0 }, Glyph { glyph_id: 37, x: 20.0, y: 0.0 }],
            font,
            font_size: 32.0,
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::translate((0.0, 40.0)),
            text: None,
        });
        scene.fill_rect_solid(Rect::new(50.0, 50.0, 60.0, 60.0), Color::from_rgba8(0, 255, 0, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache(), Some(&mut atlas), None, max_depth(), false);
        assert_eq!(frame.glyphs.len(), 2, "both glyphs must have placed for this to be a meaningful coalescing test");
        assert_eq!(frame.draw_batches().len(), 3, "Quad, then Glyph (both glyphs coalesced into ONE batch), then Quad");
        let kinds: Vec<BatchKind> = frame.draw_batches().iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Glyph, BatchKind::Quad]);
        assert_eq!(frame.draw_batches()[1].count, 2, "both glyphs from the one GlyphRun must coalesce into a single batch");
    }

    // ── Commit 4: metrics-recorder-backed degrade-counter proof ──────
    //
    // `metrics-util`'s `DebuggingRecorder` is NOT used anywhere in this
    // workspace (grepped — no hits), so there is no existing pattern to
    // mirror. Rather than pull in a new dev-dependency for one test,
    // this uses `metrics::with_local_recorder` — already part of the
    // crate's EXISTING `metrics = "0.24"` dependency (no new dep at
    // all) — with a small hand-rolled `Recorder` that records counter
    // values into a `Mutex<HashMap<Key, Arc<AtomicU64>>>`.
    mod metrics_recorder_proof {
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::{Arc, Mutex};

        use metrics::{Counter, CounterFn, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit};

        struct RecordedCounter(AtomicU64);
        impl CounterFn for RecordedCounter {
            fn increment(&self, value: u64) {
                self.0.fetch_add(value, Ordering::SeqCst);
            }
            fn absolute(&self, value: u64) {
                self.0.store(value, Ordering::SeqCst);
            }
        }

        #[derive(Default)]
        struct TestRecorder {
            counters: Mutex<HashMap<Key, Arc<RecordedCounter>>>,
        }

        impl TestRecorder {
            /// Sum of every counter registered under `metric_name` whose
            /// labels include `kind == label` — matches how
            /// `metrics::counter!(KEY, "kind" => label)` registers.
            fn value_for(&self, metric_name: &str, label: &str) -> u64 {
                let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
                map.iter()
                    .filter(|(key, _)| {
                        key.name() == metric_name && key.labels().any(|l| l.key() == "kind" && l.value() == label)
                    })
                    .map(|(_, counter)| counter.0.load(Ordering::SeqCst))
                    .sum()
            }

            /// Sum of every counter registered under `metric_name`,
            /// regardless of labels — for LABEL-LESS counters like
            /// `KEY_RENDER_SKIPPED_NONFINITE` (`encode_scene`'s own
            /// `metrics::counter!(KEY_RENDER_SKIPPED_NONFINITE)` call
            /// carries no `"kind"` label, unlike every `degrade()` call
            /// `value_for` above is built for).
            fn total_for(&self, metric_name: &str) -> u64 {
                let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
                map.iter()
                    .filter(|(key, _)| key.name() == metric_name)
                    .map(|(_, counter)| counter.0.load(Ordering::SeqCst))
                    .sum()
            }
        }

        impl Recorder for TestRecorder {
            fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
            fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
            fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

            fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
                let mut map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
                let handle = map.entry(key.clone()).or_insert_with(|| Arc::new(RecordedCounter(AtomicU64::new(0))));
                Counter::from_arc(handle.clone())
            }
            fn register_gauge(&self, _key: &Key, _metadata: &Metadata<'_>) -> Gauge {
                Gauge::noop()
            }
            fn register_histogram(&self, _key: &Key, _metadata: &Metadata<'_>) -> Histogram {
                Histogram::noop()
            }
        }

        use super::*;

        /// Unbalanced-scene guard (design §2.4, mirrored from the
        /// CPU-side blend-layer `force_close_all` audit fix, Wave 3
        /// Commit 1): a `PushClipRoundedRect` with NO matching
        /// `PopClip` still gets a decrement batch emitted at
        /// end-of-scene, and the force-close is counted
        /// (`native_rounded_clip_force_closed_at_scene_end`) rather
        /// than silently leaving the stencil buffer in a stale nonzero
        /// state for whatever (already-malformed) frame comes next.
        /// Replaces the Wave 1 `rounded_clip_bbox_degrade_counter_actually_increments`
        /// test — its subject (`native_rounded_clip_to_rect_bbox`) was
        /// REMOVED this commit, replaced by real stencil clipping.
        #[test]
        fn unbalanced_push_clip_rounded_rect_force_closes_at_scene_end() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::PushClipRoundedRect {
                    rect: uzor_urx_core::math::RoundedRect::from_rect(
                        Rect::new(10.0, 10.0, 30.0, 30.0),
                        uzor_urx_core::math::RoundedRectRadii::new(5.0, 5.0, 5.0, 5.0),
                    ),
                    transform: Affine::IDENTITY,
                });
                scene.fill_rect_solid(Rect::new(0.0, 0.0, 100.0, 100.0), Color::from_rgba8(255, 0, 0, 255));
                // NO PopClip — deliberately unbalanced.
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            let batches = frame.draw_batches();
            let last = batches.last().expect("at least the synthetic decrement batch must exist");
            assert_eq!(last.kind, BatchKind::StencilMask(MaskOp::Decrement));
            assert_eq!(last.stencil_ref, Some(1), "force-close gates on the still-open scope's own depth");

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_rounded_clip_force_closed_at_scene_end");
            assert_eq!(value, 1, "the force-close counter must have actually incremented, not just avoided a panic");
        }

        // ── Wave 3 Commit 3: blend-layer degrade-counter proofs ──────

        fn push_blend_layer(mode: BlendMode, alpha: f32, transform: Affine) -> DrawCommand {
            DrawCommand::PushBlendLayer { mode, alpha, transform }
        }

        /// Symmetric twin of `unbalanced_push_clip_rounded_rect_force_closes_at_scene_end`
        /// above — an unbalanced `PushBlendLayer` still gets composited
        /// (the op-stream shape is proven separately in the plain `tests`
        /// module's `unbalanced_push_blend_layer_force_closes_at_scene_end_with_synthetic_pop`),
        /// and the force-close is counted exactly once, never silent.
        #[test]
        fn blend_layer_force_closed_at_scene_end_counter_increments_once() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(push_blend_layer(BlendMode::default(), 1.0, Affine::IDENTITY));
                scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));
                // NO PopBlendLayer — deliberately unbalanced.
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_force_closed_at_scene_end");
            assert_eq!(value, 1, "the force-close counter must have actually incremented, not just avoided a panic");
        }

        /// Design §3.3: each push beyond the depth cap counts
        /// `native_blend_layer_depth_exceeded` once — never silently
        /// dropped.
        #[test]
        fn blend_layer_depth_exceeded_counts_once_per_suppressed_push() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(push_blend_layer(BlendMode::default(), 1.0, Affine::IDENTITY)); // depth 1 — real
                scene.push(push_blend_layer(BlendMode::default(), 1.0, Affine::IDENTITY)); // suppressed
                scene.push(push_blend_layer(BlendMode::default(), 1.0, Affine::IDENTITY)); // suppressed
                scene.push(DrawCommand::PopBlendLayer);
                scene.push(DrawCommand::PopBlendLayer);
                scene.push(DrawCommand::PopBlendLayer);
                encode_scene(&scene, viewport(), &mut cache(), None, None, 1, false)
            });

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_depth_exceeded");
            assert_eq!(value, 2, "one count per suppressed push, cap = 1 with 3 total pushes");
        }

        /// Mirrors `ClipStack::pop`'s existing underflow guard — an extra
        /// `PopBlendLayer` with no open layer (and nothing suppressed to
        /// balance) is a defensive no-op, counted rather than panicking.
        #[test]
        fn blend_layer_pop_underflow_is_a_defensive_noop_and_counts() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::PopBlendLayer); // no matching push at all
                scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            assert_eq!(frame.quads.len(), 1, "content after the stray pop must still render normally, no panic");
            assert!(frame.composites.is_empty(), "an underflowing pop has no layer to composite");

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_pop_underflow");
            assert_eq!(value, 1);
        }

        /// Design §0.3: a non-`{Normal, SrcOver}` `BlendMode` degrades to
        /// that combination, counting `mix`- and `compose`-mismatch
        /// independently (same two-counter shape as
        /// `uzor-urx-cpu::blend::degrade_for_mode`, Wave 3 Commit 1) —
        /// proven here for BOTH fields mismatching in the SAME pop.
        #[test]
        fn blend_layer_non_default_mode_counts_mix_and_compose_degrades_independently() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                let mode = BlendMode::new(uzor_urx_core::math::Mix::Multiply, uzor_urx_core::math::Compose::SrcIn);
                scene.push(push_blend_layer(mode, 1.0, Affine::IDENTITY));
                scene.fill_rect_solid(Rect::new(0.0, 0.0, 5.0, 5.0), Color::from_rgba8(1, 1, 1, 255));
                scene.push(DrawCommand::PopBlendLayer);
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_mix_to_normal"), 1);
            assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_compose_to_srcover"), 1);
        }

        /// `PushBlendLayer::transform` still drops ANY non-identity value
        /// entirely (Wave 3 design §0.2 — unaffected by Wave 4's §5
        /// full-affine work, which only extends `FillRect`/`StrokeRect`/
        /// `Line`/`Image`, not blend-layer transforms) — a non-identity
        /// value is counted (never silently applied OR silently
        /// ignored), `native_blend_layer_transform_ignored`.
        #[test]
        fn blend_layer_non_identity_transform_is_counted() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(push_blend_layer(BlendMode::default(), 1.0, Affine::translate((5.0, 5.0))));
                scene.push(DrawCommand::PopBlendLayer);
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_blend_layer_transform_ignored");
            assert_eq!(value, 1);
        }

        // ── Wave 4 Commit 3: gradient/image degrade-counter proofs ───

        /// Without a real `GradientLutAtlas`, a Radial/Sweep `FillRect`
        /// counts `native_gradient_lut_full_this_frame` (the SAME
        /// degrade the real "every row already touched this frame"
        /// case uses — from the caller's perspective both are "no LUT
        /// row available," design §8).
        #[test]
        fn radial_gradient_without_a_lut_counts_the_degrade() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                    radii: None,
                    brush: radial_gradient_brush(),
                    transform: Affine::IDENTITY,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_gradient_lut_full_this_frame");
            assert_eq!(value, 1);
        }

        /// Focal (non-concentric) Radial gradient counts the SAME
        /// UNPREFIXED `gradient_radial_focal_degraded` label CPU emits
        /// (design §2.6) — checked at ENCODE time, independent of
        /// whether a real LUT atlas is available.
        #[test]
        fn focal_radial_gradient_counts_the_shared_unprefixed_label() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut g = Gradient::new_radial(kurbo::Point::new(20.0, 20.0), 15.0);
                if let GradientKind::Radial(pos) = &mut g.kind {
                    pos.start_center = kurbo::Point::new(5.0, 5.0); // far from end_center — genuinely focal
                }
                g.stops = two_stop_gradient_stops();
                let mut scene = Scene::new();
                scene.push(DrawCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                    radii: None,
                    brush: Brush::Gradient(g),
                    transform: Affine::IDENTITY,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "gradient_radial_focal_degraded"), 1);
        }

        // ── Wave 4 Commit 4: routing telemetry proofs ─────────────────

        /// A genuine shear (columns not orthogonal) routes a Solid
        /// `FillRect` through the Triangle pipeline instead of Quad SDF
        /// (design §5.2), counting the telemetry-only (never a real
        /// degrade — the shape renders CORRECTLY either way)
        /// `native_rect_shear_to_triangle_pipeline` label exactly once.
        #[test]
        fn sheared_rect_routes_through_triangle_pipeline_and_counts_telemetry() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::FillRect {
                    rect: Rect::new(10.0, 10.0, 30.0, 30.0),
                    radii: None,
                    brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
                    transform: Affine::new([1.0, 0.0, 0.5, 1.0, 0.0, 0.0]), // genuine shear
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            assert!(frame.quads.is_empty(), "a sheared transform must NOT route through Quad SDF");
            assert!(!frame.triangles.is_empty(), "must route through the Triangle pipeline instead");
            assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "native_rect_shear_to_triangle_pipeline"), 1);
        }

        /// Design §6.2's closure proof: non-uniform per-corner radii
        /// route through the Triangle pipeline and — unlike Wave 1-3 —
        /// NO degrade counter fires at all. `native_per_corner_radii_uniform_approx`
        /// is fully CLOSED (design §8): there's no longer an
        /// approximation to report. The shear telemetry must ALSO stay
        /// silent here since the transform is `IDENTITY` — this proves
        /// the two routing reasons (non-uniform radii vs. shear) are
        /// independently gated, not conflated into one counter.
        #[test]
        fn non_uniform_radii_rect_routes_through_triangle_pipeline_without_any_degrade() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                    radii: Some([4.0, 40.0, 4.0, 40.0]), // deliberately non-uniform
                    brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
                    transform: Affine::IDENTITY,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            assert!(frame.quads.is_empty(), "non-uniform radii must NOT route through Quad SDF");
            assert!(!frame.triangles.is_empty(), "must route through the Triangle pipeline instead");
            assert_eq!(
                recorder.value_for(KEY_RENDER_PRIMITIVES, "native_rect_shear_to_triangle_pipeline"),
                0,
                "non-uniform radii under an IDENTITY (non-shear) transform must not count the shear telemetry"
            );
            assert_eq!(
                recorder.value_for(KEY_RENDER_PRIMITIVES, "native_per_corner_radii_uniform_approx"),
                0,
                "the approximation counter is fully CLOSED — must never fire again"
            );
        }

        /// An unregistered `ImageId` is a clean, honest miss — no
        /// `ImageInstance` is emitted, and the shared, UNPREFIXED
        /// `image_id_unknown` counter fires (design §4.4). Device-free
        /// — `encode_image` only ever touches the shared
        /// `uzor_urx_image` registry, never a GPU resource, at encode
        /// time.
        #[test]
        fn unregistered_image_counts_miss_and_emits_nothing() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::Image {
                    src: ImageId(u64::MAX),
                    src_rect: None,
                    dest: Rect::new(0.0, 0.0, 20.0, 20.0),
                    transform: Affine::IDENTITY,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            assert!(frame.images.is_empty(), "no ImageInstance for an unregistered id");
            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "image_id_unknown");
            assert_eq!(value, 1);
        }

        /// A sheared `DrawCommand::Image` degrades to a rotation-only
        /// approximation (design §4.3) — still emits an `ImageInstance`
        /// (never silently dropped), and counts
        /// `native_image_shear_to_rotation_approx`.
        #[test]
        fn sheared_image_degrades_to_rotation_only_and_still_renders() {
            let id = register_test_image(2, 2, [10, 20, 30, 255]);
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                // Non-uniform off-diagonal terms — genuine shear, not a
                // pure rotation+scale.
                let shear = Affine::new([1.0, 0.0, 0.5, 1.0, 0.0, 0.0]);
                scene.push(DrawCommand::Image {
                    src: id,
                    src_rect: None,
                    dest: Rect::new(0.0, 0.0, 10.0, 10.0),
                    transform: shear,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });
            assert_eq!(frame.images.len(), 1, "a sheared image must still render, approximated");
            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_image_shear_to_rotation_approx");
            assert_eq!(value, 1);
            uzor_urx_image::unregister_image(id);
        }

        // ── Wave 2 Commit 2: GlyphRun degrade proofs ─────────────────
        //
        // Both tests below use an UNREGISTERED `FontId` deliberately —
        // `rasterise_glyph` fails for every glyph before
        // `encode_glyph_run` ever calls `atlas.get_or_insert`, so these
        // stay 100% device-free (`atlas: None` is provably safe here,
        // see `encode_scene`'s doc comment) while still exercising the
        // REAL brush-kind-degrade + per-glyph-rasterise-failure code
        // paths, not a mock of them.

        #[test]
        fn glyphrun_with_unregistered_font_counts_rasterise_failed_and_emits_nothing() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::GlyphRun {
                    glyphs: vec![
                        uzor_urx_core::scene::Glyph { glyph_id: 1, x: 0.0, y: 0.0 },
                        uzor_urx_core::scene::Glyph { glyph_id: 2, x: 10.0, y: 0.0 },
                    ],
                    font: uzor_urx_core::scene::FontId(u64::MAX), // never registered
                    font_size: 32.0,
                    brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
                    transform: Affine::IDENTITY,
                    text: None,
                });
                let frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
                assert_eq!(frame.glyphs.len(), 0, "no glyph should have been placed — the font was never registered");
            });

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_glyph_rasterise_failed");
            assert_eq!(value, 2, "one native_glyph_rasterise_failed per glyph in the run");
        }

        #[test]
        fn glyphrun_gradient_brush_degrades_to_solid_without_panicking() {
            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let gradient = Gradient::new_linear((0.0, 0.0), (10.0, 0.0))
                    .with_stops([(0.0f32, Color::from_rgba8(0, 0, 0, 255)), (1.0f32, Color::from_rgba8(255, 255, 255, 255))]);
                let mut scene = Scene::new();
                scene.push(DrawCommand::GlyphRun {
                    glyphs: vec![uzor_urx_core::scene::Glyph { glyph_id: 1, x: 0.0, y: 0.0 }],
                    font: uzor_urx_core::scene::FontId(u64::MAX - 1), // never registered — rasterise fails harmlessly
                    font_size: 32.0,
                    brush: Brush::Gradient(gradient),
                    transform: Affine::IDENTITY,
                    text: None,
                });
                // Must not panic — that's the primary assertion here.
                let _frame = encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false);
            });

            let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_glyphrun_gradient_to_solid");
            assert_eq!(value, 1, "the gradient-brush degrade counter must fire exactly once per GlyphRun");
        }

        /// Wave 2 Commit 3 (design §9): force `native_glyph_atlas_full_this_frame`
        /// via a tiny (64x64) atlas — sized off a REAL
        /// `UrxConfigBuilder::wgpu_glyph_atlas_w/h(64)` config (proving the
        /// config knob's VALUE, not just a literal `64`, drives the tiny
        /// size) — plus a single `GlyphRun` carrying more unique glyph
        /// identities than a 64x64 atlas can hold at once. All glyphs are
        /// in ONE run, encoded in ONE `encode_scene` call (one tick), so
        /// none of the ones that DO get placed are eviction-eligible
        /// (never-evict-this-frame invariant) — once physical space runs
        /// out mid-run, every remaining glyph must hit the degrade.
        ///
        /// The `metrics::with_local_recorder` pattern (same technique as
        /// this submodule's other proofs) composes fine with a real
        /// device-backed atlas — no awkwardness — so this asserts via
        /// BOTH the degrade counter AND `NativeGlyphAtlas::stats()`
        /// (the same data `NativeUrxRenderer::glyph_atlas_stats()` reads)
        /// for a belt-and-suspenders proof.
        #[test]
        #[ignore = "needs a headless GPU adapter"]
        fn tiny_configured_atlas_forces_full_this_frame_degrade() {
            let Some((device, queue)) = test_device() else { return };
            let cfg = uzor_urx_core::config::UrxConfig::builder()
                .wgpu_glyph_atlas_w(64)
                .wgpu_glyph_atlas_h(64)
                .build()
                .expect("64x64 is a valid atlas dim");
            let mut atlas = NativeGlyphAtlas::new(&device, &queue, cfg.wgpu_glyph_atlas_w, cfg.wgpu_glyph_atlas_h);
            atlas.begin_frame();

            let font = registered_font();
            // 80 distinct glyph identities at a modest size (24px) — a
            // 64x64 atlas (4096 px²) cannot possibly hold 80 padded
            // glyph rects of any plausible non-whitespace size at once
            // (even a generous over-estimate of ~20x20px padded puts a
            // single glyph at ~400px², so ~10 is already the practical
            // ceiling) — comfortably forces overflow regardless of which
            // specific ids happen to be whitespace/.notdef/out-of-range
            // (those fail via `rasterise_glyph` or the zero-size skip,
            // both harmless to this test — plenty of the 80 will be real
            // ink).
            let glyphs: Vec<Glyph> = (3u32..83).map(|id| Glyph { glyph_id: id, x: 0.0, y: 0.0 }).collect();

            let recorder = TestRecorder::default();
            metrics::with_local_recorder(&recorder, || {
                let mut scene = Scene::new();
                scene.push(DrawCommand::GlyphRun {
                    glyphs,
                    font,
                    font_size: 24.0,
                    brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
                    transform: Affine::IDENTITY,
                    text: None,
                });
                let _frame = encode_scene(&scene, viewport(), &mut cache(), Some(&mut atlas), None, max_depth(), false);
            });

            let full_this_frame = recorder.value_for(KEY_RENDER_PRIMITIVES, "native_glyph_atlas_full_this_frame");
            assert!(full_this_frame > 0, "a 64x64 atlas fed 80 unique glyph identities in ONE frame must overflow");

            let stats = atlas.stats();
            assert!(
                stats.entries < 80,
                "a 64x64 atlas cannot hold 80 unique glyph rects — entries ({}) must be well short of 80",
                stats.entries
            );
        }

        // ── Bug fix (2026-07-25): FillPath/StrokePath non-finite path ──
        // ── points used to reach lyon's tessellator and hard-panic ──────
        //
        // `uzor_urx_core::validate::validate_command`'s `FillPath`/
        // `StrokePath` arms used to check ONLY `transform`, completely
        // ignoring the path's own points — a NaN/Inf coordinate INSIDE
        // the path slipped past this loop's `if let Err(NonFinite) =
        // validate_command(cmd) { .. continue }` guard (this file, top
        // of `encode_scene`) and reached `tessellate::build_lyon_path`,
        // which feeds points straight into lyon's path builder — lyon
        // hard-`assert!`s every point is finite and panics.
        // `uzor-urx-cpu`'s scanline rasteriser has no such assertion, so
        // the identical scene degraded silently there instead — this
        // backend is the one that actually panicked. Fixed at the
        // `validate_command` level (`uzor-urx-core::validate`'s own new
        // `is_finite_bezpath`), which this file's pre-existing skip-and-
        // count wiring picks up with NO change needed here.

        #[test]
        fn nonfinite_fill_path_point_is_skipped_not_panicked_and_counts_the_metric() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut path = BezPath::new();
                path.move_to(Point::new(0.0, 0.0));
                path.line_to(Point::new(f64::NAN, 10.0));
                path.line_to(Point::new(10.0, 10.0));
                path.close_path();

                let mut scene = Scene::new();
                scene.push(DrawCommand::FillPath {
                    path,
                    rule: FillRule::NonZero,
                    brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
                    transform: Affine::IDENTITY,
                });
                // Must not panic — the pre-fix code path panics inside
                // `tessellate::build_lyon_path`'s call into lyon before
                // this call even returns.
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            assert!(frame.triangles.is_empty(), "the non-finite path must contribute NO geometry");
            assert_eq!(
                recorder.total_for(KEY_RENDER_SKIPPED_NONFINITE),
                1,
                "the shared skip-and-count policy must fire exactly once"
            );
        }

        #[test]
        fn nonfinite_stroke_path_point_is_skipped_not_panicked_and_counts_the_metric() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut path = BezPath::new();
                path.move_to(Point::new(0.0, 0.0));
                path.line_to(Point::new(10.0, f64::INFINITY));

                let mut scene = Scene::new();
                scene.push(DrawCommand::StrokePath {
                    path,
                    stroke: SceneStroke { width: 2.0, ..SceneStroke::default() },
                    brush: Brush::Solid(Color::from_rgba8(0, 255, 0, 255)),
                    transform: Affine::IDENTITY,
                });
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            assert!(frame.triangles.is_empty(), "the non-finite path must contribute NO geometry");
            assert_eq!(recorder.total_for(KEY_RENDER_SKIPPED_NONFINITE), 1);
        }

        /// A NaN/Inf cmd must not derail an otherwise-valid scene — the
        /// SAME "skip just the bad one, keep going" contract
        /// `uzor-urx-cpu`'s own adversarial suite pins
        /// (`valid_cmds_still_paint_after_bad_cmd`).
        #[test]
        fn nonfinite_fill_path_does_not_block_a_later_valid_command() {
            let recorder = TestRecorder::default();
            let frame = metrics::with_local_recorder(&recorder, || {
                let mut bad_path = BezPath::new();
                bad_path.move_to(Point::new(0.0, 0.0));
                bad_path.line_to(Point::new(f64::NAN, 10.0));

                let mut scene = Scene::new();
                scene.push(DrawCommand::FillPath {
                    path: bad_path,
                    rule: FillRule::NonZero,
                    brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
                    transform: Affine::IDENTITY,
                });
                scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(0, 255, 0, 255));
                encode_scene(&scene, viewport(), &mut cache(), None, None, max_depth(), false)
            });

            assert_eq!(frame.quads.len(), 1, "the valid FillRect after the bad path must still render");
            assert_eq!(recorder.total_for(KEY_RENDER_SKIPPED_NONFINITE), 1);
        }
    }
}
