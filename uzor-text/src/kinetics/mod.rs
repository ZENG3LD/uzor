//! Kinetics: two-[`crate::layout::ParagraphLayout`] position/opacity
//! morph (Arc 2 Phase 3 of
//! `nemo/docs/uzor-viz/uzor_text_arc2_design.md`).
//!
//! Scope, exactly as fixed by the design doc's §4 Phase 3: the
//! **resize-morph** case — the same paragraph laid out twice at two
//! different `max_width`s (or, more generally, two layouts whose runs
//! carry the same underlying text) — never the general "different text
//! before/after" PowerPoint-Morph case (deferred to whenever
//! `uzor-deck`'s build-steps need it; see [`morph`]'s own doc comment for
//! what this phase does when handed mismatched text anyway, without
//! panicking).

mod morph;

pub use morph::{build_morph, sample, sample_layout, MorphTransition};

use uzor::ui::animation::math::timeline::Animatable;

/// One glyph's animatable kinetic state — position + opacity/scale/
/// rotation, per the design doc's own `kinetics/mod.rs` sketch (§3.2).
///
/// Phase 3 only ever drives `pos` and `opacity` (see [`morph`] for why);
/// `scale`/`rotation` exist because the doc's own sketch declares them on
/// this struct — they stay pinned at `1.0`/`0.0` here and are load-bearing
/// for whichever later phase adds a scaling/rotating kinetics effect. Per
/// the doc's own caution: `Animatable::lerp` on `rotation` is a naive
/// linear interpolation with no shortest-angle wraparound — fine while
/// nothing in this crate ever sets it non-zero.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphState {
    /// The glyph cluster's source text. Never varies under `lerp` — a
    /// matched pair's `from`/`to` clusters are identical by construction
    /// (see [`morph`]'s identity-match rule); an unmatched glyph mirrors
    /// its own cluster onto the side it's missing from, so `ch` is always
    /// the same string on both ends of any one `GlyphState`'s morph.
    pub ch: String,
    /// Absolute `(x, y baseline)` in the paragraph box.
    pub pos: (f64, f64),
    pub opacity: f32,
    pub scale: f32,
    pub rotation: f32,
}

impl Animatable for GlyphState {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        GlyphState {
            ch: self.ch.clone(),
            pos: self.pos.lerp(&target.pos, t),
            opacity: self.opacity.lerp(&target.opacity, t),
            scale: self.scale.lerp(&target.scale, t),
            rotation: self.rotation.lerp(&target.rotation, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_at_zero_and_one_returns_the_endpoints_exactly() {
        let a = GlyphState { ch: "x".to_owned(), pos: (0.0, 0.0), opacity: 1.0, scale: 1.0, rotation: 0.0 };
        let b = GlyphState { ch: "x".to_owned(), pos: (10.0, 20.0), opacity: 0.0, scale: 2.0, rotation: 0.5 };

        let at0 = a.lerp(&b, 0.0);
        assert_eq!(at0.pos, a.pos);
        assert_eq!(at0.opacity, a.opacity);
        assert_eq!(at0.scale, a.scale);

        let at1 = a.lerp(&b, 1.0);
        assert_eq!(at1.pos, b.pos);
        assert_eq!(at1.opacity, b.opacity);
        assert_eq!(at1.scale, b.scale);
    }

    #[test]
    fn lerp_at_half_is_the_midpoint() {
        let a = GlyphState { ch: "x".to_owned(), pos: (0.0, 0.0), opacity: 0.0, scale: 1.0, rotation: 0.0 };
        let b = GlyphState { ch: "x".to_owned(), pos: (10.0, 20.0), opacity: 1.0, scale: 1.0, rotation: 0.0 };

        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.pos, (5.0, 10.0));
        assert!((mid.opacity - 0.5).abs() < 1e-6);
    }
}
