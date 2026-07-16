//! [`FrameMorph`] — block-level identity-matched morph between two
//! ADJACENT [`crate::slice::frames::ComposedFrame`]s (design doc §4.3):
//! matched blocks tween `(x,y,width,height)` via the core `Animatable`
//! impl for `(f64,f64,f64,f64)` (`uzor::ui::animation::math::timeline`,
//! already shipped — no new geometry-interpolation code); unmatched blocks
//! fade in/out at their own fixed position; a matched PARAGRAPH pair whose
//! two sides carry byte-identical text delegates WHOLESALE to
//! `uzor_text::kinetics::{build_morph, sample_layout}` for glyph-level
//! interpolation — this crate never reaches into a matched paragraph's own
//! glyphs itself (design doc §7 P4 risk note: the two nested identity
//! systems, block-level `BlockId` here and glyph-level word-ordinal keys
//! inside `uzor-text`, stay strictly layered).

use uzor::types::Rect;
use uzor::ui::animation::math::timeline::Animatable;
use uzor_text::{build_morph as build_paragraph_morph, sample_layout as sample_paragraph_layout, MorphTransition, Paragraph, ParagraphLayout};

use super::identity::match_frames;
use crate::region::{ListPlacement, PlacedBlock, TablePlacement};
use crate::scene::{Block, BlockId};
use crate::slice::frames::ComposedFrame;

/// One block's sampled, drawable state at a given `t` — [`FrameMorph::
/// sample`]'s own output shape. Mirrors [`PlacedBlock`]'s own fields plus
/// `opacity`, which [`PlacedBlock`] itself deliberately does NOT carry —
/// every P0-P3 consumer always paints at full opacity, so adding an
/// unread field to that pervasively-constructed type would be pure churn;
/// a small, additive, phase-local type is the "no field nothing reads"
/// convention this crate already uses elsewhere (see `CLAUDE.md`).
pub struct FrameBlockState<'a> {
    pub id: BlockId,
    pub rect: Rect,
    pub opacity: f32,
    pub kind: &'a Block<'a>,
    pub paragraph_layout: Option<ParagraphLayout>,
    pub table_placement: Option<TablePlacement<'a>>,
    pub list_placement: Option<ListPlacement<'a>>,
}

/// One matched pair's own precomputed transition kind — decided ONCE at
/// [`build_frame_morph`] time, never re-decided per sample.
enum PairKind {
    /// Block-level only: tween `(x,y,width,height)`; content painted
    /// verbatim from the `to` side (used for every non-`Paragraph` kind,
    /// and for a matched `Paragraph` pair whose text actually differs —
    /// see [`build_frame_morph`]'s own delegation rule).
    Geometry,
    /// Both sides are `Block::Paragraph` with byte-identical text —
    /// delegate glyph-level interpolation wholesale to `uzor-text`'s own
    /// Phase 3 kinetics.
    Paragraph(MorphTransition),
}

struct MatchedPair {
    from_idx: usize,
    to_idx: usize,
    kind: PairKind,
}

/// A prepared, sampleable transition between two [`ComposedFrame`]s (design
/// law 3: stateless — computed once by [`build_frame_morph`],
/// [`FrameMorph::sample`] reads it any number of times at any `t`).
pub struct FrameMorph<'a> {
    from: Vec<PlacedBlock<'a>>,
    to: Vec<PlacedBlock<'a>>,
    matched: Vec<MatchedPair>,
    from_only: Vec<usize>,
    to_only: Vec<usize>,
}

impl<'a> FrameMorph<'a> {
    /// Number of blocks matched (by [`crate::kinetics::identity`]) across
    /// `from`/`to`.
    pub fn matched_count(&self) -> usize {
        self.matched.len()
    }

    /// Number of `from`-side blocks with no partner in `to` (fade out
    /// toward `t = 1.0`).
    pub fn from_only_count(&self) -> usize {
        self.from_only.len()
    }

    /// Number of `to`-side blocks with no partner in `from` (fade in
    /// toward `t = 1.0`).
    pub fn to_only_count(&self) -> usize {
        self.to_only.len()
    }

    /// Sample this morph at `t` (clamped to `[0, 1]`) into every block's
    /// drawable state — [`crate::render::draw_frame_state`]'s own input.
    /// `t = 0.0`/`t = 1.0` reproduce `from`/`to`'s own placed rects and
    /// opacities exactly (matched: opacity `1.0` throughout, rect at its
    /// own endpoint; unmatched: opacity `0.0`/`1.0` at the fade's own
    /// start/end). `ink_rgb` is the fallback glyph color a delegated
    /// paragraph pair's own unmatched (word-ordinal-mismatched — see
    /// `uzor_text::kinetics::morph`'s own module docs) glyphs fade
    /// against — the SAME `base_rgb` `uzor_text::kinetics::sample_layout`
    /// itself requires, threaded through here rather than baked in at
    /// [`build_frame_morph`] time (paint color is a `Theme` decision,
    /// resolved at sample/paint time everywhere else in this crate).
    /// Determinism: `t` comes entirely from the caller — no clock/RNG
    /// anywhere in this function.
    pub fn sample(&self, t: f64, ink_rgb: u32) -> Vec<FrameBlockState<'a>> {
        let t = t.clamp(0.0, 1.0);
        let mut states = Vec::with_capacity(self.matched.len() + self.from_only.len() + self.to_only.len());

        for pair in &self.matched {
            let from_block = &self.from[pair.from_idx];
            let to_block = &self.to[pair.to_idx];
            let lerped = rect_tuple(from_block.rect).lerp(&rect_tuple(to_block.rect), t);
            let rect = tuple_rect(lerped);

            match &pair.kind {
                PairKind::Paragraph(transition) => {
                    states.push(FrameBlockState {
                        id: to_block.id,
                        rect,
                        opacity: 1.0,
                        kind: to_block.kind,
                        paragraph_layout: Some(sample_paragraph_layout(transition, t, ink_rgb)),
                        table_placement: None,
                        list_placement: None,
                    });
                }
                PairKind::Geometry => {
                    let dx = rect.x - to_block.rect.x;
                    let dy = rect.y - to_block.rect.y;
                    let mut placed = to_block.clone();
                    placed.translate(dx, dy);
                    placed.rect.width = rect.width;
                    placed.rect.height = rect.height;
                    states.push(FrameBlockState {
                        id: placed.id,
                        rect: placed.rect,
                        opacity: 1.0,
                        kind: placed.kind,
                        paragraph_layout: placed.paragraph_layout,
                        table_placement: placed.table_placement,
                        list_placement: placed.list_placement,
                    });
                }
            }
        }

        for &idx in &self.from_only {
            states.push(fade_state(&self.from[idx], (1.0_f64).lerp(&0.0, t) as f32));
        }
        for &idx in &self.to_only {
            states.push(fade_state(&self.to[idx], (0.0_f64).lerp(&1.0, t) as f32));
        }

        states
    }
}

fn fade_state<'a>(block: &PlacedBlock<'a>, opacity: f32) -> FrameBlockState<'a> {
    FrameBlockState {
        id: block.id,
        rect: block.rect,
        opacity,
        kind: block.kind,
        paragraph_layout: block.paragraph_layout.clone(),
        table_placement: block.table_placement.clone(),
        list_placement: block.list_placement.clone(),
    }
}

fn rect_tuple(rect: Rect) -> (f64, f64, f64, f64) {
    (rect.x, rect.y, rect.width, rect.height)
}

fn tuple_rect(t: (f64, f64, f64, f64)) -> Rect {
    Rect::new(t.0, t.1, t.2, t.3)
}

/// Concatenate every run's own text — the design doc's own delegation
/// rule ("matched Paragraph pairs with EQUAL text") compares plain
/// concatenated content, independent of styling/color, since a resize/
/// reflow between two build steps may legitimately change a run's font
/// without changing what the paragraph SAYS.
fn paragraph_text(paragraph: &Paragraph<'_>) -> String {
    paragraph.runs.iter().map(|r| r.text).collect::<Vec<_>>().join("")
}

/// Build a [`FrameMorph`] between `from`/`to` — identity match delegated to
/// [`crate::kinetics::identity::match_frames`] (`force_matches` passed
/// through verbatim); a matched pair where BOTH sides are
/// `Block::Paragraph` with byte-identical text gets glyph-level
/// delegation, everything else (including a matched Paragraph pair whose
/// text actually differs) gets block-level geometry tweening only. Never
/// panics regardless of how unrelated `from`/`to` are (matches
/// `uzor_text::kinetics::build_morph`'s own "no fallible surface"
/// convention) — every block with no match on either side simply fades.
pub fn build_frame_morph<'a>(from: &ComposedFrame<'a>, to: &ComposedFrame<'a>, force_matches: &[(BlockId, BlockId)]) -> FrameMorph<'a> {
    let frame_match = match_frames(from, to, force_matches);

    let matched = frame_match
        .matched
        .into_iter()
        .map(|(from_idx, to_idx)| {
            let from_block = &from.blocks[from_idx];
            let to_block = &to.blocks[to_idx];

            let kind = match (from_block.kind, to_block.kind) {
                (Block::Paragraph(from_p), Block::Paragraph(to_p)) if paragraph_text(from_p) == paragraph_text(to_p) => {
                    match (&from_block.paragraph_layout, &to_block.paragraph_layout) {
                        (Some(from_layout), Some(to_layout)) => PairKind::Paragraph(build_paragraph_morph(from_layout, to_layout)),
                        _ => PairKind::Geometry,
                    }
                }
                _ => PairKind::Geometry,
            };

            MatchedPair { from_idx, to_idx, kind }
        })
        .collect();

    FrameMorph { from: from.blocks.clone(), to: to.blocks.clone(), matched, from_only: frame_match.from_only, to_only: frame_match.to_only }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor::types::Rect;
    use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, StyledRun};

    use crate::region::PlacedBlock;

    fn frame_of(blocks: Vec<PlacedBlock<'_>>) -> ComposedFrame<'_> {
        ComposedFrame { width: 400.0, height: 300.0, blocks }
    }

    /// `sample(0)`/`sample(1)` reproduce every block's own endpoint rect/
    /// opacity exactly; `sample(0.5)` sits strictly between for a matched
    /// pair whose rect genuinely differs; every sampled value stays
    /// finite and within `[0,1]` opacity bounds.
    #[test]
    fn sample_at_zero_and_one_reproduces_endpoints_and_half_is_strictly_between_with_no_nan() {
        let spacer = Block::Spacer(1.0);

        let matched_from_rect = Rect::new(0.0, 0.0, 100.0, 50.0);
        let matched_to_rect = Rect::new(200.0, 0.0, 50.0, 80.0);
        let from_only_rect = Rect::new(0.0, 100.0, 40.0, 40.0);
        let to_only_rect = Rect::new(0.0, 200.0, 40.0, 40.0);

        let from = frame_of(vec![
            PlacedBlock { id: BlockId(1), rect: matched_from_rect, kind: &spacer, paragraph_layout: None, table_placement: None, list_placement: None },
            PlacedBlock { id: BlockId(2), rect: from_only_rect, kind: &spacer, paragraph_layout: None, table_placement: None, list_placement: None },
        ]);
        let to = frame_of(vec![
            PlacedBlock { id: BlockId(1), rect: matched_to_rect, kind: &spacer, paragraph_layout: None, table_placement: None, list_placement: None },
            PlacedBlock { id: BlockId(3), rect: to_only_rect, kind: &spacer, paragraph_layout: None, table_placement: None, list_placement: None },
        ]);

        let morph = build_frame_morph(&from, &to, &[]);
        assert_eq!(morph.matched_count(), 1);
        assert_eq!(morph.from_only_count(), 1);
        assert_eq!(morph.to_only_count(), 1);

        let at0 = morph.sample(0.0, 0x111111);
        let at1 = morph.sample(1.0, 0x111111);
        let mid = morph.sample(0.5, 0x111111);

        let matched0 = at0.iter().find(|s| s.id == BlockId(1)).expect("matched block present at t=0");
        assert_eq!(matched0.rect, matched_from_rect);
        assert_eq!(matched0.opacity, 1.0);

        let matched1 = at1.iter().find(|s| s.id == BlockId(1)).expect("matched block present at t=1");
        assert_eq!(matched1.rect, matched_to_rect);
        assert_eq!(matched1.opacity, 1.0);

        let matched_mid = mid.iter().find(|s| s.id == BlockId(1)).expect("matched block present at t=0.5");
        assert!(
            matched_mid.rect.x > matched_from_rect.x && matched_mid.rect.x < matched_to_rect.x,
            "midpoint rect must sit strictly between the two endpoints"
        );

        let from_only_at0 = at0.iter().find(|s| s.id == BlockId(2)).expect("from-only block present at t=0");
        assert_eq!(from_only_at0.opacity, 1.0);
        assert_eq!(from_only_at0.rect, from_only_rect);
        let from_only_at1 = at1.iter().find(|s| s.id == BlockId(2)).expect("from-only block present at t=1");
        assert_eq!(from_only_at1.opacity, 0.0, "a from-only block must be fully faded out by t=1");
        let from_only_mid = mid.iter().find(|s| s.id == BlockId(2)).expect("from-only block present at t=0.5");
        assert!((from_only_mid.opacity - 0.5).abs() < 1e-6);

        let to_only_at0 = at0.iter().find(|s| s.id == BlockId(3)).expect("to-only block present at t=0");
        assert_eq!(to_only_at0.opacity, 0.0, "a to-only block must not be visible yet at t=0");
        let to_only_at1 = at1.iter().find(|s| s.id == BlockId(3)).expect("to-only block present at t=1");
        assert_eq!(to_only_at1.opacity, 1.0);

        for state in at0.iter().chain(at1.iter()).chain(mid.iter()) {
            assert!(state.rect.x.is_finite() && state.rect.y.is_finite() && state.rect.width.is_finite() && state.rect.height.is_finite());
            assert!((0.0..=1.0).contains(&state.opacity), "opacity must stay within [0,1], got {}", state.opacity);
        }
    }

    const LONG_SENTENCE: &str = "The quick brown fox jumps over the lazy dog and then keeps running further down the road without stopping";

    /// A matched `Paragraph` pair with byte-identical text delegates to
    /// glyph-level interpolation: never frame-level fades (opacity stays
    /// `1.0`), and mid-morph glyph positions genuinely differ from BOTH
    /// endpoints' own sampled layouts — real movement, not a crossfade.
    #[test]
    fn matched_paragraph_pair_with_equal_text_delegates_to_glyph_level_morph_not_a_crossfade() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let runs = [StyledRun::new(LONG_SENTENCE, font)];

        let narrow = Paragraph::new(&runs, 160.0);
        let wide = Paragraph::new(&runs, 560.0);
        let narrow_layout = layout_paragraph(&narrow, &shaper);
        let wide_layout = layout_paragraph(&wide, &shaper);
        assert!(narrow_layout.lines.len() > wide_layout.lines.len(), "fixture must actually rewrap between the two widths");

        let from_kind = Block::Paragraph(narrow);
        let to_kind = Block::Paragraph(wide);

        let from = frame_of(vec![PlacedBlock {
            id: BlockId(7),
            rect: Rect::new(0.0, 0.0, 160.0, narrow_layout.height),
            kind: &from_kind,
            paragraph_layout: Some(narrow_layout),
            table_placement: None,
            list_placement: None,
        }]);
        let to = frame_of(vec![PlacedBlock {
            id: BlockId(7),
            rect: Rect::new(0.0, 0.0, 560.0, wide_layout.height),
            kind: &to_kind,
            paragraph_layout: Some(wide_layout),
            table_placement: None,
            list_placement: None,
        }]);

        let morph = build_frame_morph(&from, &to, &[]);
        assert_eq!(morph.matched_count(), 1);

        let at0 = morph.sample(0.0, 0x111111);
        let at1 = morph.sample(1.0, 0x111111);
        let mid = morph.sample(0.5, 0x111111);
        assert_eq!(mid[0].opacity, 1.0, "a delegated paragraph pair never frame-level fades -- glyph-level morph only");

        let at0_layout = at0[0].paragraph_layout.as_ref().expect("a delegated paragraph pair must carry a sampled layout");
        let at1_layout = at1[0].paragraph_layout.as_ref().expect("a delegated paragraph pair must carry a sampled layout");
        let mid_layout = mid[0].paragraph_layout.as_ref().expect("a delegated paragraph pair must carry a sampled layout");
        assert_eq!(at0_layout.glyphs.len(), mid_layout.glyphs.len());
        assert_eq!(at1_layout.glyphs.len(), mid_layout.glyphs.len());

        let any_moved = mid_layout
            .glyphs
            .iter()
            .zip(at0_layout.glyphs.iter())
            .zip(at1_layout.glyphs.iter())
            .any(|((m, a), b)| (m.x, m.y) != (a.x, a.y) && (m.x, m.y) != (b.x, b.y));
        assert!(any_moved, "mid-morph glyph positions must differ from BOTH endpoints -- real interpolation, not a crossfade");
    }

    /// A matched `Paragraph` pair whose text actually DIFFERS never
    /// glyph-morphs (the design doc explicitly defers general fuzzy-text
    /// morph past this phase) — falls back to geometry tweening, content
    /// painted is the `to` side's own layout verbatim, never a panic.
    #[test]
    fn matched_paragraph_pair_with_different_text_falls_back_to_geometry_tween_no_panic() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let from_runs = [StyledRun::new("Step one caption.", font)];
        let to_runs = [StyledRun::new("A completely different step two caption.", font)];

        let from_p = Paragraph::new(&from_runs, 300.0);
        let to_p = Paragraph::new(&to_runs, 300.0);
        let from_layout = layout_paragraph(&from_p, &shaper);
        let to_layout = layout_paragraph(&to_p, &shaper);
        let to_glyph_count = to_layout.glyphs.len();

        let from_kind = Block::Paragraph(from_p);
        let to_kind = Block::Paragraph(to_p);

        let from = frame_of(vec![PlacedBlock {
            id: BlockId(9),
            rect: Rect::new(0.0, 0.0, 300.0, from_layout.height),
            kind: &from_kind,
            paragraph_layout: Some(from_layout),
            table_placement: None,
            list_placement: None,
        }]);
        let to = frame_of(vec![PlacedBlock {
            id: BlockId(9),
            rect: Rect::new(50.0, 0.0, 300.0, to_layout.height),
            kind: &to_kind,
            paragraph_layout: Some(to_layout),
            table_placement: None,
            list_placement: None,
        }]);

        let morph = build_frame_morph(&from, &to, &[]);
        let mid = morph.sample(0.5, 0x111111);
        assert_eq!(mid.len(), 1);
        assert_eq!(mid[0].opacity, 1.0);
        let layout = mid[0].paragraph_layout.as_ref().expect("geometry-tween paragraph still carries a layout to paint");
        assert_eq!(layout.glyphs.len(), to_glyph_count, "unequal text does not glyph-morph -- content shown is the `to` side's own layout verbatim");
    }
}
