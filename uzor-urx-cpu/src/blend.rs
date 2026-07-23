//! CPU blend-layer stack — real offscreen-pixmap group isolation for
//! `DrawCommand::PushBlendLayer`/`PopBlendLayer` (URX Wave 3 Commit 1,
//! `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
//! §5). Closes `cpu_blend_layer_dropped` — before this, the two
//! commands were no-ops and content drew straight onto the current
//! target in painter's order, exactly as if they didn't exist (no
//! isolation, `alpha` silently discarded, design §0.4).
//!
//! Sequential-`render()`-only feature — `parallel.rs`/`tile.rs` already
//! bail on any `PushBlendLayer`/`PopBlendLayer` before reaching this
//! module (verified by direct read, see this crate's own doc trail):
//! `parallel.rs`'s pre-scan loop returns `RenderError::ParallelUnsupported`
//! for both variants BEFORE the per-strip loop runs (`parallel.rs:46-55`);
//! `backend.rs`'s `tile_eligible` only matches `FillRect`/`PushClipRect`/
//! `PopClip` and catches everything else, including blend layers, via
//! its `_ => return false` arm (`backend.rs:15-46`) — `tile.rs`'s own
//! `render_tiled_with_config` therefore never sees a blend-layer command
//! at all. Both constraints are pre-existing and UNCHANGED by this wave.
//!
//! ## `pop()`'s parent-resolution — a deliberate deviation from the
//! design doc's sketch (§5.4)
//!
//! The design sketch calls `layer_stack.pop(layer_stack.current_target_parent(pixmap))`
//! from `backend.rs` — this double-borrows `layer_stack` (the method
//! receiver for `.pop(..)` needs `&mut layer_stack` for the whole call;
//! the argument expression `layer_stack.current_target_parent(pixmap)`
//! needs ANOTHER borrow of the same value to construct that argument,
//! and `current_target_parent` must return `&mut Pixmap`, so it's an
//! exclusive borrow — genuinely uncompilable, not a two-phase-borrow
//! edge case). Resolution chosen here: [`LayerStack::pop`] takes the
//! TRUE ROOT `Pixmap` (never a caller-computed "current parent") and
//! resolves its own parent internally, AFTER popping the top frame off
//! `open` (which releases any borrow of `open` before the next borrow
//! of it is taken) — no simultaneous borrow ever exists. The call site
//! in `backend.rs` is simply `layer_stack.pop(pixmap)`, where `pixmap`
//! is the SAME root binding `render()` was originally handed, never
//! `layer_stack.current_target(pixmap)` (which would be the frame ABOUT
//! to be popped, not its parent).
//!
//! ## End-of-scene force-close — the CPU-side symmetric guard the
//! design doc only spelled out for GPU (§3.3)
//!
//! An unbalanced scene (`PushBlendLayer` with no matching
//! `PopBlendLayer`) would otherwise leave its `LayerFrame` — and
//! whatever content was drawn into it — dropped when `LayerStack`
//! itself goes out of scope at the end of `render()`, SILENTLY LOSING
//! that content (a real regression vs. the pre-Wave-3 no-op behaviour,
//! where unmatched content simply drew straight to root and stayed
//! visible — and a never-silent-doctrine violation). `backend.rs`
//! calls [`LayerStack::force_close_all`] once, after the whole command
//! loop, mirroring the GPU design's `native_blend_layer_force_closed_at_scene_end`
//! guard (§3.3) with the CPU-side counterpart
//! `cpu_blend_layer_force_closed_at_scene_end`.

use uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES;

use crate::pixmap::Pixmap;

/// One currently-open blend layer — an isolated, full-canvas-sized
/// (§0.2: layers cover the full viewport, matching every other native
/// target's sizing convention) offscreen accumulation buffer, plus the
/// `mode`/`alpha` its matching `PushBlendLayer` requested (remembered
/// here so the eventual `Pop` can degrade-check `mode` and scale by
/// `alpha` without the IR command itself needing to carry either).
struct LayerFrame {
    pixmap: Pixmap,
    mode: peniko::BlendMode,
    alpha: f32,
}

/// Outcome of [`LayerStack::pop`] — what `backend.rs`'s `PopBlendLayer`
/// arm should do next.
pub(crate) enum PopOutcome {
    /// A real layer was popped and composited onto its parent target.
    Composited,
    /// This `Pop` matched a SUPPRESSED `Push` (the depth cap was hit,
    /// so no layer was ever actually opened for it) — a defensive
    /// no-op, not an error. The corresponding `Push` already counted
    /// `cpu_blend_layer_depth_exceeded` once; nothing further to count
    /// here.
    Suppressed,
    /// No open layer AND nothing suppressed to balance — an unbalanced
    /// scene's extra `Pop`. Defensive no-op, mirrors `ClipStack::pop`'s
    /// existing guard (`clip.rs:100-108`) — never panics.
    Underflow,
}

/// Tracks currently-open blend layers for one `CpuBackend::render`
/// call. Sequential-only (see this module's doc comment) — never
/// constructed by `parallel.rs`/`tile.rs`.
pub(crate) struct LayerStack {
    open: Vec<LayerFrame>,
    /// Incremented instead of pushing a real `LayerFrame` once
    /// `open.len() == max_depth` — the matching `Pop` decrements this
    /// FIRST (checked before `open` at all) so a suppressed push/pop
    /// pair always balances without ever touching `open`, exactly
    /// mirroring the GPU-side `LayerStack` design (§3.3).
    suppressed: u32,
    max_depth: usize,
}

impl LayerStack {
    /// `max_depth` — from `UrxConfig::blend_layer_max_depth`, read once
    /// at the top of `render()` and never revisited mid-frame (same
    /// "baked in at construction" policy every other config-derived
    /// cap in this crate family uses). Taken as given, no defensive
    /// floor: `UrxConfig::validate()` already rejects `0`
    /// (`ConfigError::InvalidBlendLayerDepth`) at config-build time —
    /// a caller that bypasses validation and hands `0` here gets the
    /// literal, honest consequence (every push suppressed from the
    /// first one), not a silently-substituted different number.
    pub(crate) fn new(max_depth: usize) -> Self {
        Self { open: Vec::new(), suppressed: 0, max_depth }
    }

    /// Open a new layer sized `canvas_w x canvas_h` (the ROOT pixmap's
    /// own dimensions — every layer is full-viewport, §0.2, so this is
    /// the SAME value regardless of current nesting depth). Returns
    /// `false` when the depth cap is hit (caller counts
    /// `cpu_blend_layer_depth_exceeded`); content keeps drawing into
    /// whatever target was already active, unchanged, no new pixmap
    /// allocated.
    pub(crate) fn push(&mut self, mode: peniko::BlendMode, alpha: f32, canvas_w: u32, canvas_h: u32) -> bool {
        if self.open.len() >= self.max_depth {
            self.suppressed += 1;
            return false;
        }
        self.open.push(LayerFrame { pixmap: Pixmap::new(canvas_w, canvas_h), mode, alpha });
        true
    }

    /// Composite the top-of-stack layer onto its parent — either a
    /// still-open OUTER layer's own pixmap, or `root` if this was the
    /// outermost layer. `root` must always be the TRUE root `Pixmap`
    /// (see this module's doc comment on why `pop()` resolves its own
    /// parent rather than taking a caller-computed one).
    pub(crate) fn pop(&mut self, root: &mut Pixmap) -> PopOutcome {
        if self.suppressed > 0 {
            self.suppressed -= 1;
            return PopOutcome::Suppressed;
        }
        if self.composite_top_onto_parent(root) {
            PopOutcome::Composited
        } else {
            PopOutcome::Underflow
        }
    }

    /// End-of-scene unbalanced-scene guard (symmetric with the GPU-side
    /// `native_blend_layer_force_closed_at_scene_end`, design §3.3):
    /// a `PushBlendLayer` with no matching `PopBlendLayer` would
    /// otherwise leave its `LayerFrame` dropped when `LayerStack` itself
    /// goes out of scope at the end of `render()` — silently LOSING
    /// whatever content was drawn into it (a real regression vs. the
    /// pre-Wave-3 no-op behaviour, where unmatched content simply drew
    /// straight to root and stayed visible; never-silent doctrine
    /// violation otherwise). Called once, after the whole command loop,
    /// with the TRUE root `Pixmap` — force-composites every still-open
    /// layer in LIFO order (innermost first, exactly like a normal
    /// nested Pop sequence would have) onto its parent, so an unbalanced
    /// scene's content still ends up visible, alpha-scaled, same as a
    /// well-formed scene would have produced. Returns the number of
    /// layers force-closed — caller counts
    /// `cpu_blend_layer_force_closed_at_scene_end` that many times
    /// (`0` when the scene was already balanced — the overwhelming
    /// common case — is a cheap no-op, one `Vec::is_empty` check).
    pub(crate) fn force_close_all(&mut self, root: &mut Pixmap) -> usize {
        let mut closed = 0usize;
        while self.composite_top_onto_parent(root) {
            closed += 1;
        }
        closed
    }

    /// Shared by `pop()` and `force_close_all()` — both need the
    /// identical "pop the top frame, resolve whatever is now the
    /// parent (an outer layer or `root`), composite" sequence. Returns
    /// `false` when `open` was already empty (nothing to close).
    fn composite_top_onto_parent(&mut self, root: &mut Pixmap) -> bool {
        let Some(frame) = self.open.pop() else {
            return false;
        };
        degrade_for_mode(&frame.mode);
        let parent = self.open.last_mut().map(|f| &mut f.pixmap).unwrap_or(root);
        composite_layer_srcover(parent, &frame.pixmap, frame.alpha);
        true
    }

    /// The pixmap subsequent draw commands should target — top of
    /// `open`, or `root` if no layer is currently active. Called once
    /// per draw-carrying `DrawCommand` from `backend.rs`'s main loop;
    /// `clip: &ClipStack` (computed once from `root`'s own dimensions
    /// at the top of `render()`) is completely unaffected by which
    /// target is selected here — every layer pixmap is root-sized, so
    /// the clip stack's bounds stay valid regardless of nesting depth
    /// (§0.2 / design item 4's semantics check).
    pub(crate) fn current_target<'a>(&'a mut self, root: &'a mut Pixmap) -> &'a mut Pixmap {
        self.open.last_mut().map(|f| &mut f.pixmap).unwrap_or(root)
    }
}

/// Composite `layer` onto `dst`, scaled by `alpha` — premultiplied
/// SrcOver with the layer's OWN premultiplied bytes additionally scaled
/// by the scalar `alpha` (valid because premultiplied-alpha scaling is
/// `channel *= alpha` uniformly across all 4 channels, including alpha
/// itself — no separate straight-alpha unpack/repack needed). Uses only
/// `Pixmap`'s existing public API (`get_pixel`/`blend_pixel`,
/// `pixmap.rs:172-204`) — `pixmap.rs` itself is unchanged.
pub(crate) fn composite_layer_srcover(dst: &mut Pixmap, layer: &Pixmap, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    for y in 0..dst.height() {
        for x in 0..dst.width() {
            let src = layer.get_pixel(x, y);
            let scaled = [
                ((src[0] as f32) * a).round().clamp(0.0, 255.0) as u8,
                ((src[1] as f32) * a).round().clamp(0.0, 255.0) as u8,
                ((src[2] as f32) * a).round().clamp(0.0, 255.0) as u8,
                ((src[3] as f32) * a).round().clamp(0.0, 255.0) as u8,
            ];
            dst.blend_pixel(x, y, scaled);
        }
    }
}

/// Count (never silently drop) when the requested `mode` differs from
/// the one combination this wave actually implements
/// (`{Mix::Normal, Compose::SrcOver}`, design §0.3) — split into two
/// distinct counters so a consumer can tell which half of `BlendMode`
/// was approximated. Called from inside [`LayerStack::pop`] (not a
/// separate pre-pop "peek" step, design §5.4's sketch) — the popped
/// frame already carries its own `mode` by the time this needs to run,
/// so there is no ordering footgun (nothing to forget to call first).
fn degrade_for_mode(mode: &peniko::BlendMode) {
    if mode.mix != peniko::Mix::Normal {
        metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => "cpu_blend_layer_mix_to_normal").increment(1);
    }
    if mode.compose != peniko::Compose::SrcOver {
        metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => "cpu_blend_layer_compose_to_srcover").increment(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque(r: u8, g: u8, b: u8) -> [u8; 4] {
        [r, g, b, 255]
    }

    #[test]
    fn push_returns_true_until_depth_cap_then_false() {
        let mut stack = LayerStack::new(2);
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 4, 4));
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 4, 4));
        assert!(!stack.push(peniko::BlendMode::default(), 1.0, 4, 4), "3rd push must be suppressed at max_depth=2");
    }

    #[test]
    fn current_target_is_root_when_no_layer_open() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(2, 2);
        root.set_pixel(0, 0, opaque(1, 2, 3));
        let target = stack.current_target(&mut root);
        assert_eq!(target.get_pixel(0, 0), opaque(1, 2, 3));
    }

    #[test]
    fn current_target_is_the_open_layer_once_pushed() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(2, 2);
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 2, 2));
        let target = stack.current_target(&mut root);
        // A fresh layer pixmap is all-transparent (`Pixmap::new` zeroes)
        // — distinct from whatever the root holds, proving the target
        // really switched.
        assert_eq!(target.get_pixel(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn pop_composites_layer_onto_root_scaled_by_alpha() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(1, 1);
        root.set_pixel(0, 0, opaque(0, 0, 0));
        assert!(stack.push(peniko::BlendMode::default(), 0.5, 1, 1));
        stack.current_target(&mut root).set_pixel(0, 0, opaque(200, 100, 50));
        let outcome = stack.pop(&mut root);
        assert!(matches!(outcome, PopOutcome::Composited));
        // Layer content (200,100,50,255) scaled by alpha=0.5, then
        // SrcOver-blended onto opaque black: 200*0.5=100 (rounds to
        // 100), etc.
        let px = root.get_pixel(0, 0);
        assert_eq!(px, [100, 50, 25, 255]);
    }

    #[test]
    fn pop_on_suppressed_push_is_a_silent_noop_never_touches_open() {
        let mut stack = LayerStack::new(1);
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 1, 1));
        assert!(!stack.push(peniko::BlendMode::default(), 1.0, 1, 1), "2nd push suppressed at max_depth=1");
        let mut root = Pixmap::new(1, 1);
        // Pop the SUPPRESSED push first (LIFO — matches the innermost
        // Push/Pop pairing) — must be a no-op, must NOT touch the
        // still-genuinely-open outer layer.
        assert!(matches!(stack.pop(&mut root), PopOutcome::Suppressed));
        assert_eq!(stack.open.len(), 1, "the suppressed pop must never touch the real open layer");
        // Now pop the REAL layer.
        assert!(matches!(stack.pop(&mut root), PopOutcome::Composited));
        assert_eq!(stack.open.len(), 0);
    }

    #[test]
    fn pop_underflow_on_empty_stack_is_a_silent_noop() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(1, 1);
        assert!(matches!(stack.pop(&mut root), PopOutcome::Underflow));
    }

    #[test]
    fn nested_pop_composites_onto_the_still_open_outer_layer_not_root() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(1, 1);
        root.set_pixel(0, 0, [0, 0, 0, 0]); // transparent root — proves the inner composite did NOT land here
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 1, 1)); // outer
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 1, 1)); // inner
        stack.current_target(&mut root).set_pixel(0, 0, opaque(10, 20, 30));
        assert!(matches!(stack.pop(&mut root), PopOutcome::Composited)); // inner -> outer

        // Root must still be untouched (inner composited onto OUTER,
        // not root) — the outer layer is still open.
        assert_eq!(root.get_pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(stack.open.len(), 1);
        // The outer layer must now carry the inner's composited color.
        assert_eq!(stack.current_target(&mut root).get_pixel(0, 0), opaque(10, 20, 30));

        assert!(matches!(stack.pop(&mut root), PopOutcome::Composited)); // outer -> root
        assert_eq!(root.get_pixel(0, 0), opaque(10, 20, 30));
    }

    #[test]
    fn force_close_all_composites_every_still_open_layer_lifo_and_returns_the_count() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(1, 1);
        root.set_pixel(0, 0, opaque(0, 0, 0));
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 1, 1)); // outer — never popped
        assert!(stack.push(peniko::BlendMode::default(), 1.0, 1, 1)); // inner — never popped either
        stack.current_target(&mut root).set_pixel(0, 0, opaque(200, 100, 50));

        let closed = stack.force_close_all(&mut root);
        assert_eq!(closed, 2, "both still-open layers must be force-closed");
        assert_eq!(stack.open.len(), 0, "force_close_all must drain every open frame");
        // Same math as a well-formed nested pop sequence would have
        // produced: inner (alpha=1.0) onto outer, then outer (alpha=1.0)
        // onto root — content survives unchanged at alpha=1.0 either way.
        assert_eq!(root.get_pixel(0, 0), opaque(200, 100, 50));
    }

    #[test]
    fn force_close_all_on_an_already_balanced_stack_is_a_free_noop() {
        let mut stack = LayerStack::new(4);
        let mut root = Pixmap::new(1, 1);
        assert_eq!(stack.force_close_all(&mut root), 0);
    }

    #[test]
    fn degrade_for_mode_counts_only_the_differing_half() {
        // Normal/SrcOver — the one fully-implemented combination —
        // must not count anything.
        degrade_for_mode(&peniko::BlendMode::default());
        // No assertion here beyond "did not panic" — the counter
        // proof (via `metrics::with_local_recorder`) lives in
        // `backend.rs`'s test module, alongside the full `PopBlendLayer`
        // wiring it actually gets exercised through.
    }
}
