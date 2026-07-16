//! [`Card`]/[`Slide`] — slide slicing (design doc §4.2): card mode (fluid,
//! unbounded per-card height) vs. fixed mode (bounded, PowerPoint-style
//! viewport), "conversion first-class" per the doc's own claim: both
//! modes compose the exact SAME `flow` through the exact same
//! [`crate::compose::compose`] — only the region bound (and, for fixed
//! mode, what happens on overflow) differs, never a second, lossy code
//! path.
//!
//! ## Grouping — `split_groups`, not a `RegionSequence` cross-region walk
//!
//! The design doc's own §4.2 prose ("explicit Block-level card breaks or
//! heading-driven splits") could in principle be built by feeding the
//! WHOLE `flow` through one `compose()` call over an infinite region
//! sequence and letting `BreakControl::ForceBefore`/`ForceAfter` (already
//! built, §3.4) end/start frames across region pulls — but that machinery
//! was designed for `PageRegionSequence`'s own "shrinking WITHIN one
//! region, fresh on the next" page semantics (`AvoidAfter`'s lookahead in
//! particular assumes a SHARED, still-being-filled region). Slide/card
//! grouping is a strictly simpler question — "does an explicit
//! break marker sit between these two blocks" — so `split_groups` answers
//! it directly, once, before any region/compose call happens at all. Each
//! resulting group is then composed independently and totally, `Card` and
//! `Slide` alike (design law 3: pure function of its own group + region).
//!
//! ## Fixed-mode overflow (§7 Q1 — open; owner has not chosen a default)
//!
//! The design doc names `OverflowPolicy::{AutoShrink, HardError}` without
//! picking a default; this task's own brief proposes `SlideOverflow::
//! {Report, Shrink}` when the owner hasn't answered — followed literally
//! here (`Report` ≈ doc's `HardError` — "returned as data," not a panic;
//! `Shrink` ≈ doc's `AutoShrink`). **`Report` is this crate's own default**
//! (`SlideOverflow::default()`), per this crate's repeated "overflow
//! returned as data, never silent" convention — flagged for the owner to
//! override, not silently resolved.
//!
//! Detecting overflow always composes a group/placeholder TWICE in the
//! overflow case: once bounded (`FixedRegionSequence`, `crate::region`'s
//! own real P3 consumer for this type — a bounded pass IS the common-case,
//! zero-extra-work path when content simply fits), and — ONLY when that
//! bounded pass reports it didn't fit — once more, unbounded
//! (`CardRegionSequence`), to learn the FULL, untruncated natural height.
//! This second pass is not optional: a truncating bounded pass alone can
//! only prove THAT something overflowed, never supply the complete
//! overflowing-block list `Report` must return or the complete content
//! `Shrink` must preserve without dropping anything past the first
//! region's own truncation point.

use uzor::types::Rect;
use uzor_text::LineShaper;

use crate::compose::{compose, BreakControl, ComposeStyle};
use crate::master::{SlideInstance, SlideLayout};
use crate::region::{CardRegionSequence, FixedRegionSequence, Frame, PlacedBlock, Region};
use crate::scene::{resolve_block_ids, BlockId, BlockNode};

/// Numeric slack for "does this placed block's bottom edge sit at or
/// below the target height" comparisons — guards against false-positive
/// overflow from ordinary floating-point summation error, never used to
/// paper over a REAL overflow.
const OVERFLOW_EPSILON: f64 = 1e-6;

/// One fluid card (design doc §4.2's card mode) — never overflows (an
/// unbounded region has no bound to exceed), so `natural_height` is
/// simply "however tall this group's content turned out to be."
pub struct Card<'a> {
    pub index: u32,
    pub width: f64,
    /// This card's own content height — the tallest placed block's bottom
    /// edge. Never a caller-fixed number; a card is fluid by definition
    /// (design doc §4.2/§7 P3 gate: "card-mode always produces exactly 1
    /// `Slide` at natural height regardless of content length" — here,
    /// exactly 1 `Card` per author-marked GROUP, each at its own natural
    /// height).
    pub natural_height: f64,
    pub frame: Frame<'a>,
}

/// One fixed-viewport slide (design doc §4.2's fixed/PowerPoint mode).
pub struct Slide<'a> {
    pub index: u32,
    pub width: f64,
    pub height: f64,
    pub frame: Frame<'a>,
    /// `Some(factor)` only when [`SlideOverflow::Shrink`] engaged a
    /// uniform scale-to-fit transform (§7 P3 risk note: "AutoShrink's
    /// scale factor must apply as ONE uniform transform across every
    /// block in the slide, never per-block independent shrink, or
    /// relative proportions break"). Applied at PAINT time via
    /// `Painter::{save,scale,restore}` (`crate::render::draw_slide`) —
    /// never a per-block rect rewrite; `None` when content already fit,
    /// or under [`SlideOverflow::Report`] (which never reaches this far —
    /// it returns `Err` instead).
    pub shrink_scale: Option<f64>,
}

/// Fixed-slide overflow policy (§7 Q1 — open; see this module's own doc
/// comment for the naming rationale and why `Report` is the default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideOverflow {
    /// Content that doesn't fit the fixed viewport is reported as data —
    /// every overflowing block's id, in flow order — never silently
    /// clipped. The caller resolves it by editing content.
    Report,
    /// Content that doesn't fit is scaled down by ONE uniform factor
    /// (never per-block) until it fits — genuinely content-preserving
    /// (every block is still fully composed/placed, just painted
    /// smaller), never a truncation.
    Shrink,
}

impl Default for SlideOverflow {
    fn default() -> Self {
        SlideOverflow::Report
    }
}

/// A fixed slide's content overflowed its own viewport under
/// [`SlideOverflow::Report`] — never a silent clip (design doc §7 P3
/// gate: "fixed-mode over-full content ... returns `SliceError::
/// Overflow` (`HardError`) — never silently clips").
#[derive(Debug, Clone, PartialEq)]
pub struct SliceError {
    /// Which slide (0-based, among the groups `flow` split into) —
    /// `0` for the single-slide [`slice_slide_instance`] entry point,
    /// which has no deck position of its own.
    pub slide_index: u32,
    /// Every block (in flow order) whose own placed content extends past
    /// the fixed viewport's own height.
    pub overflowing: Vec<BlockId>,
}

/// Split `flow` into card/slide GROUPS at `BreakControl::ForceBefore`/
/// `ForceAfter` boundaries (design doc §4.2: "explicit Block-level card
/// breaks") — see this module's own top doc comment for why this is a
/// direct, standalone split rather than a `RegionSequence` cross-region
/// walk. A node marked `ForceBefore` starts a fresh group (unless it's
/// already the first node of the current one — forcing a break before
/// nothing wastes an empty group, same convention `compose::flow`'s own
/// `ForceBefore` arm already uses); a node marked `ForceAfter` ends the
/// current group right after it. No markers at all → the whole `flow` is
/// ONE group (design doc §7 P3 gate: "card-mode always produces exactly 1
/// `Slide` ... regardless of content length").
fn split_groups<'a>(flow: &'a [BlockNode<'a>]) -> Vec<&'a [BlockNode<'a>]> {
    if flow.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    let mut start = 0usize;
    for i in 0..flow.len() {
        if i > start && flow[i].break_control == BreakControl::ForceBefore {
            groups.push(&flow[start..i]);
            start = i;
        }
        if flow[i].break_control == BreakControl::ForceAfter {
            groups.push(&flow[start..=i]);
            start = i + 1;
        }
    }
    if start < flow.len() {
        groups.push(&flow[start..]);
    }
    groups
}

/// This frame's own natural content height — the tallest placed block's
/// bottom edge (frame-relative; every region this module hands to
/// `compose` starts at `y = 0.0`, so no further offset math is needed
/// here, design law 1).
fn natural_height(frame: &Frame<'_>) -> f64 {
    frame.blocks.iter().map(|b| b.rect.y + b.rect.height).fold(0.0_f64, f64::max)
}

/// Compose `content` at `width` via an unbounded [`CardRegionSequence`] —
/// a region that never overflows, so this ALWAYS places every block, in
/// full, never truncating — then translate the result to `origin` (so its
/// blocks land at the SAME absolute position a caller's own bounded pass
/// would have used). The non-lossy fallback both [`slice_slides`]/
/// [`slice_slide_instance`] reach for once a bounded pass reports
/// overflow (see this module's own top doc comment for why).
fn compose_unbounded<'a>(content: &'a [BlockNode<'a>], origin: (f64, f64), width: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> (Frame<'a>, f64) {
    let mut regions = CardRegionSequence::new(width);
    let mut frame = compose(content, &mut regions, style, shaper)
        .into_iter()
        .next()
        .unwrap_or_else(|| Frame { region: Region { rect: Rect::new(0.0, 0.0, width, 0.0) }, blocks: Vec::new(), overflow: None });
    let height = natural_height(&frame);
    if origin != (0.0, 0.0) {
        for placed in &mut frame.blocks {
            placed.translate(origin.0, origin.1);
        }
    }
    (frame, height)
}

/// Slice `flow` into fluid [`Card`]s (design doc §4.2's card mode) — one
/// `Card` per group [`split_groups`] finds, each at its own natural
/// height. Never fails: an unbounded region never overflows.
pub fn slice_cards<'a>(flow: &'a [BlockNode<'a>], width: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Card<'a>> {
    split_groups(flow)
        .into_iter()
        .enumerate()
        .map(|(i, group)| {
            let (frame, natural_height) = compose_unbounded(group, (0.0, 0.0), width, style, shaper);
            Card { index: i as u32, width, natural_height, frame }
        })
        .collect()
}

/// Slice `flow` into fixed-viewport [`Slide`]s (design doc §4.2's fixed
/// mode) at `width` x `height`, one `Slide` per group [`split_groups`]
/// finds. A group whose content fits `height` composes via a bounded
/// [`FixedRegionSequence`] exactly as placed (`shrink_scale: None`); a
/// group that overflows is resolved per `overflow_policy` (see this
/// module's own top doc comment). "Conversion first-class" with
/// [`slice_cards`]: the SAME `split_groups` drives both, and a group that
/// fits produces a `Slide` whose block placement/ids/order is IDENTICAL
/// to its own `slice_cards` `Card` (region height never affects HOW
/// something is placed once it's confirmed to fit whole, only WHETHER it
/// does).
pub fn slice_slides<'a>(
    flow: &'a [BlockNode<'a>],
    width: f64,
    height: f64,
    overflow_policy: SlideOverflow,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> Result<Vec<Slide<'a>>, SliceError> {
    let mut slides = Vec::new();

    for (i, group) in split_groups(flow).into_iter().enumerate() {
        let mut fixed_regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, width, height));
        let fixed_frame = compose(group, &mut fixed_regions, style, shaper)
            .into_iter()
            .next()
            .unwrap_or_else(|| Frame { region: Region { rect: Rect::new(0.0, 0.0, width, height) }, blocks: Vec::new(), overflow: None });

        let fits = fixed_frame.overflow.is_none() && fixed_frame.blocks.iter().all(|b| b.rect.y + b.rect.height <= height + OVERFLOW_EPSILON);
        if fits {
            slides.push(Slide { index: i as u32, width, height, frame: fixed_frame, shrink_scale: None });
            continue;
        }

        let (natural_frame, content_height) = compose_unbounded(group, (0.0, 0.0), width, style, shaper);
        match overflow_policy {
            SlideOverflow::Report => {
                let overflowing: Vec<BlockId> = natural_frame
                    .blocks
                    .iter()
                    .filter(|b| b.rect.y + b.rect.height > height + OVERFLOW_EPSILON)
                    .map(|b| b.id)
                    .collect();
                return Err(SliceError { slide_index: i as u32, overflowing });
            }
            SlideOverflow::Shrink => {
                let scale = (height / content_height).clamp(0.0, 1.0);
                slides.push(Slide { index: i as u32, width, height, frame: natural_frame, shrink_scale: Some(scale) });
            }
        }
    }

    Ok(slides)
}

/// "Card → slide" conversion (design doc §4.2's own "conversion
/// first-class" claim), named to make the intent explicit at call sites:
/// re-slices the SAME `flow` with a fixed viewport instead of an unbounded
/// one. Literally [`slice_slides`] — nothing about a block's identity or
/// order changes, only the target region and (if content doesn't fit) the
/// overflow policy's own explicit report.
pub fn cards_to_slides<'a>(
    flow: &'a [BlockNode<'a>],
    width: f64,
    height: f64,
    overflow_policy: SlideOverflow,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> Result<Vec<Slide<'a>>, SliceError> {
    slice_slides(flow, width, height, overflow_policy, style, shaper)
}

/// "Slide → card" conversion — the other half of "conversion
/// first-class": re-slices the SAME `flow` as fluid cards. Literally
/// [`slice_cards`]; never fails (an unbounded region never overflows).
pub fn slides_to_cards<'a>(flow: &'a [BlockNode<'a>], width: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Card<'a>> {
    slice_cards(flow, width, style, shaper)
}

/// Compose a [`SlideInstance`]'s placeholder fills into their
/// [`SlideLayout`]'s own arranged rects — ONE [`FixedRegionSequence`]
/// region PER placeholder (this task's own P3 scope: "content composed
/// INTO placeholder rects (each placeholder = one region)"), closing P2's
/// own "SlideMaster/SlideLayout/SlideInstance ... NOT wired into a
/// slicing pipeline yet" divergence note.
///
/// A SEPARATE entry point from [`slice_cards`]/[`slice_slides`] (design
/// doc §4.2's flat-flow card/fixed modes): a [`crate::master::
/// PlaceholderFill`] holds exactly ONE [`BlockNode`], a genuinely
/// different content shape from an arbitrary-length flow group, so
/// folding both into one function would force one of the two shapes
/// through a lossy adapter — kept separate and reported here rather than
/// silently forced together.
///
/// `instance` is borrowed for the SAME lifetime as its own content (`&'a
/// SlideInstance<'a>`), NOT [`SlideInstance::arrange`]'s own shorter `'s`
/// — that method's re-borrow is right for its own re-mapping-only
/// callers (rects only, content untouched), but composing placeholder
/// content into a [`Frame`] needs the content to outlive this call the
/// same way every other `&'a [BlockNode<'a>]` this crate composes already
/// does.
///
/// A layout slot with no matching fill is skipped, matching
/// `SlideInstance::arrange`'s own chosen "unfilled" rule (P2's own
/// divergence log). Overflow is resolved per `overflow_policy`
/// PER PLACEHOLDER, but a [`SlideOverflow::Shrink`] scale factor is
/// computed as the WORST case across every overflowing placeholder and
/// applied to the WHOLE returned `Slide` (§7 P3 risk note: "ONE uniform
/// transform ... never per-block independent shrink" — generalized here
/// to "never per-PLACEHOLDER independent shrink" for the same reason).
pub fn slice_slide_instance<'a>(
    instance: &'a SlideInstance<'a>,
    layout: &SlideLayout,
    slide_width: f64,
    slide_height: f64,
    overflow_policy: SlideOverflow,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> Result<Slide<'a>, SliceError> {
    let mut blocks: Vec<PlacedBlock<'a>> = Vec::new();
    let mut overflowing: Vec<BlockId> = Vec::new();
    let mut worst_scale = 1.0_f64;

    for (slot, rect) in &layout.arrangement {
        let Some(fill) = instance.fills.iter().find(|f| f.slot == *slot) else { continue };
        let content: &'a BlockNode<'a> = &fill.content;
        let one = std::slice::from_ref(content);

        let mut fixed_regions = FixedRegionSequence::new(*rect);
        let fixed_frame = compose(one, &mut fixed_regions, style, shaper)
            .into_iter()
            .next()
            .unwrap_or_else(|| Frame { region: Region { rect: *rect }, blocks: Vec::new(), overflow: None });

        let bound_bottom = rect.y + rect.height;
        let fits = fixed_frame.overflow.is_none() && fixed_frame.blocks.iter().all(|b| b.rect.y + b.rect.height <= bound_bottom + OVERFLOW_EPSILON);
        if fits {
            blocks.extend(fixed_frame.blocks);
            continue;
        }

        let (natural_frame, content_height) = compose_unbounded(one, (rect.x, rect.y), rect.width, style, shaper);
        match overflow_policy {
            SlideOverflow::Report => {
                overflowing.push(resolve_block_ids(one)[0]);
            }
            SlideOverflow::Shrink => {
                worst_scale = worst_scale.min((rect.height / content_height).clamp(0.0, 1.0));
                blocks.extend(natural_frame.blocks);
            }
        }
    }

    if !overflowing.is_empty() {
        return Err(SliceError { slide_index: 0, overflowing });
    }

    let shrink_scale = if worst_scale < 1.0 { Some(worst_scale) } else { None };
    let region = Region { rect: Rect::new(0.0, 0.0, slide_width, slide_height) };
    Ok(Slide { index: 0, width: slide_width, height: slide_height, frame: Frame { region, blocks, overflow: None }, shrink_scale })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::master::{PlaceholderFill, PlaceholderKind, PlaceholderSlot, SlideInstance};
    use crate::scene::Block;

    fn font() -> FontSpec {
        FontSpec::new(FontFamily::Roboto, 16.0)
    }

    fn style() -> ComposeStyle {
        ComposeStyle::new(6.0, font())
    }

    /// Card mode: content with two explicit `ForceBefore` breaks splits
    /// into exactly 3 cards, each at its own natural (unbounded) height —
    /// never a caller-fixed number (design doc §7 P3 gate).
    #[test]
    fn card_mode_splits_at_force_before_markers_and_each_card_is_its_own_natural_height() {
        let shaper = CosmicShaper::headless();
        let run_a = [StyledRun::new("Card A — short.", font())];
        let run_b = [StyledRun::new(
            "Card B has a longer paragraph than card A, tall enough that its own natural height must exceed card A's.",
            font(),
        )];
        let run_c = [StyledRun::new("Card C — short again.", font())];

        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_a, 300.0))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_b, 300.0))).with_break_control(BreakControl::ForceBefore),
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_c, 300.0))).with_break_control(BreakControl::ForceBefore),
        ];

        let cards = slice_cards(&flow, 300.0, &style(), &shaper);
        assert_eq!(cards.len(), 3, "two ForceBefore markers must split the flow into exactly 3 cards");
        for (i, card) in cards.iter().enumerate() {
            assert_eq!(card.index, i as u32);
            assert_eq!(card.width, 300.0);
            assert!(card.natural_height > 0.0, "every card must have real, non-zero natural content height");
        }
        assert!(cards[1].natural_height > cards[0].natural_height, "card B's own longer paragraph must yield a taller natural height");
    }

    /// No break markers at all → the WHOLE flow is exactly one card,
    /// regardless of content length (design doc §7 P3 gate, verbatim).
    #[test]
    fn card_mode_with_no_break_markers_produces_exactly_one_card_regardless_of_length() {
        let shaper = CosmicShaper::headless();
        let run = [StyledRun::new(
            "A single long repeated filler paragraph with no break markers at all, composed purely to prove that card mode never \
            splits content on its own — only an explicit author-marked break ever starts a new card, matching the design doc's own \
            P3 gate: card mode always produces exactly one Slide at natural height regardless of content length.",
            font(),
        )];
        let flow: Vec<BlockNode<'_>> = (0..12).map(|_| BlockNode::new(Block::Paragraph(Paragraph::new(&run, 300.0)))).collect();

        let cards = slice_cards(&flow, 300.0, &style(), &shaper);
        assert_eq!(cards.len(), 1, "no ForceBefore/ForceAfter markers must yield exactly one card no matter how long the content is");
        assert_eq!(cards[0].frame.blocks.len(), 12);
    }

    /// Fixed mode: content that comfortably fits the fixed viewport
    /// composes verbatim, `shrink_scale: None`, and its block placement
    /// is IDENTICAL to the same group's own `slice_cards` output —
    /// "conversion first-class" round-trip proof (also exercised
    /// end-to-end via `cards_to_slides`/`slides_to_cards`).
    #[test]
    fn fixed_mode_content_that_fits_matches_card_mode_placement_exactly_conversion_round_trip() {
        let shaper = CosmicShaper::headless();
        let run_a = [StyledRun::new("Slide A title.", font())];
        let run_b = [StyledRun::new("Slide B body copy, still short.", font())];

        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_a, 300.0))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_b, 300.0))).with_break_control(BreakControl::ForceBefore),
        ];

        let cards = slides_to_cards(&flow, 300.0, &style(), &shaper);
        let slides = cards_to_slides(&flow, 300.0, 500.0, SlideOverflow::Report, &style(), &shaper).expect("content fits, must not overflow");

        assert_eq!(cards.len(), slides.len());
        for (card, slide) in cards.iter().zip(slides.iter()) {
            assert_eq!(card.index, slide.index);
            assert!(slide.shrink_scale.is_none(), "content that fits must never engage a shrink transform");
            assert_eq!(card.frame.blocks.len(), slide.frame.blocks.len());
            for (cb, sb) in card.frame.blocks.iter().zip(slide.frame.blocks.iter()) {
                assert_eq!(cb.id, sb.id, "block ids must be preserved across the card<->slide conversion");
                assert_eq!(cb.rect, sb.rect, "block placement must be identical when content fits either mode");
            }
        }

        // Round trip: slides -> cards again must reproduce the SAME ids/order.
        let cards_again = slides_to_cards(&flow, 300.0, &style(), &shaper);
        assert_eq!(cards_again.len(), cards.len());
        for (a, b) in cards_again.iter().zip(cards.iter()) {
            let ids_a: Vec<_> = a.frame.blocks.iter().map(|p| p.id).collect();
            let ids_b: Vec<_> = b.frame.blocks.iter().map(|p| p.id).collect();
            assert_eq!(ids_a, ids_b, "re-slicing as cards again must preserve block ids and order");
        }
    }

    /// Fixed mode, `SlideOverflow::Report`: content too tall for the
    /// fixed viewport returns `SliceError::overflowing` naming the block
    /// that didn't fit — never a silent clip (design doc §7 P3 gate).
    #[test]
    fn fixed_mode_report_overflow_names_the_overflowing_block_never_silently_clips() {
        let shaper = CosmicShaper::headless();
        let run = [StyledRun::new(
            "A deliberately long paragraph, repeated wrapped across many lines at this narrow width, tall enough on its own that it \
            cannot possibly fit inside a very short fixed slide viewport, forcing the Report overflow policy to name it explicitly \
            rather than silently truncating or dropping the remainder of its own content.",
            font(),
        )];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, 300.0))).with_id(crate::scene::BlockId(99))];

        let result = slice_slides(&flow, 300.0, 40.0, SlideOverflow::Report, &style(), &shaper);
        // `Slide` deliberately does not derive `Debug` (`Frame`/`Block`
        // hold `&dyn TypesetFigure` trait objects with no blanket `Debug`
        // impl) — match manually rather than reach for
        // `Result::expect_err`, which requires the `Ok` side to be
        // `Debug` too.
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("content taller than the fixed viewport must report overflow, not silently succeed"),
        };
        assert_eq!(err.slide_index, 0);
        assert_eq!(err.overflowing, vec![crate::scene::BlockId(99)], "the overflowing paragraph's own author id must be named");
    }

    /// Fixed mode, `SlideOverflow::Shrink`: content too tall for the fixed
    /// viewport is scaled down by ONE uniform factor and every block is
    /// still fully present (never dropped) — design doc §7 P3 risk note.
    #[test]
    fn fixed_mode_shrink_computes_a_uniform_scale_and_preserves_every_block() {
        let shaper = CosmicShaper::headless();
        let run_a = [StyledRun::new("Heading that stays on the shrunk slide.", font())];
        let run_b = [StyledRun::new(
            "A long body paragraph, wrapped across several lines, that on its own already exceeds a very short fixed slide height, \
            forcing the Shrink policy to scale the whole slide down instead of reporting an error or dropping any of this text.",
            font(),
        )];
        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_a, 300.0))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_b, 300.0))),
        ];

        let (_, natural) = compose_unbounded(&flow, (0.0, 0.0), 300.0, &style(), &shaper);
        let target_height = natural / 2.0;

        let slides = slice_slides(&flow, 300.0, target_height, SlideOverflow::Shrink, &style(), &shaper).expect("Shrink must never error");
        assert_eq!(slides.len(), 1);
        let slide = &slides[0];
        let scale = slide.shrink_scale.expect("overflowing content under Shrink must engage a scale factor");
        assert!(scale > 0.0 && scale < 1.0, "scale must be a real shrink factor, got {scale}");
        assert!((scale - target_height / natural).abs() < 1e-9, "scale must be exactly target/natural, one uniform factor");

        // Every block from the natural (unbounded) placement is still
        // present, in full — Shrink never drops content, only paints it
        // smaller (design doc §7 P3 risk note).
        assert_eq!(slide.frame.blocks.len(), 2, "both blocks must survive a shrink, never truncated");
    }

    /// Placeholder-fill composition: a `SlideInstance`'s fills compose
    /// into their `SlideLayout`'s own arranged rects, one
    /// `FixedRegionSequence` region per placeholder.
    #[test]
    fn slide_instance_placeholder_fills_compose_into_their_own_arranged_rects() {
        use crate::master::{LayoutId, MasterId};

        let shaper = CosmicShaper::headless();
        let title_run = [StyledRun::new("Deck Title", font())];
        let body_run = [StyledRun::new("Deck body copy, short enough to fit comfortably.", font())];

        let title_slot = PlaceholderSlot::new(PlaceholderKind::Title, 0);
        let body_slot = PlaceholderSlot::new(PlaceholderKind::Body, 0);

        let fills = vec![
            PlaceholderFill::new(title_slot, BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, 400.0)))),
            PlaceholderFill::new(body_slot, BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, 400.0)))),
        ];
        let instance = SlideInstance::new(LayoutId(1), fills);
        let layout = SlideLayout::new(
            LayoutId(1),
            MasterId(1),
            vec![(title_slot, Rect::new(0.0, 0.0, 400.0, 60.0)), (body_slot, Rect::new(0.0, 60.0, 400.0, 200.0))],
        );

        let slide = slice_slide_instance(&instance, &layout, 400.0, 260.0, SlideOverflow::Report, &style(), &shaper)
            .expect("comfortably-fitting placeholder content must not overflow");

        assert_eq!(slide.frame.blocks.len(), 2, "both filled placeholders must contribute their own placed content");
        assert!(slide.shrink_scale.is_none());
        for placed in &slide.frame.blocks {
            // Title placeholder occupies y in [0,60), body placeholder
            // occupies y in [60,260) — every placed block must land
            // inside SOME placeholder's own declared band, never at a
            // stray (0,0) origin unrelated to its own rect (proves the
            // per-placeholder region — not a single shared one — drove
            // composition).
            assert!(placed.rect.y >= 0.0 && placed.rect.y < 260.0);
        }
        let title_placed = slide.frame.blocks.iter().find(|b| b.rect.y < 60.0).expect("title content must land in the title band");
        assert_eq!(title_placed.rect.x, 0.0);
    }

    /// Placeholder-fill overflow: content too tall for its OWN
    /// placeholder rect reports that placeholder's overflowing block id
    /// under `SlideOverflow::Report` — never silently clipped.
    #[test]
    fn slide_instance_report_overflow_names_the_overflowing_placeholder() {
        use crate::master::{LayoutId, MasterId};

        let shaper = CosmicShaper::headless();
        let body_run = [StyledRun::new(
            "A deliberately long body paragraph, wrapped across many lines at this placeholder's own width, tall enough that it \
            cannot possibly fit inside a very short placeholder rect, forcing Report to name it rather than silently truncating.",
            font(),
        )];
        let body_slot = PlaceholderSlot::new(PlaceholderKind::Body, 0);
        let fills = vec![PlaceholderFill::new(
            body_slot,
            BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, 300.0))).with_id(crate::scene::BlockId(7)),
        )];
        let instance = SlideInstance::new(LayoutId(1), fills);
        let layout = SlideLayout::new(LayoutId(1), MasterId(1), vec![(body_slot, Rect::new(0.0, 0.0, 300.0, 20.0))]);

        let result = slice_slide_instance(&instance, &layout, 300.0, 20.0, SlideOverflow::Report, &style(), &shaper);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("a placeholder too short for its own content must report overflow"),
        };
        assert_eq!(err.overflowing, vec![crate::scene::BlockId(7)]);
    }

    /// A layout slot with no matching fill is skipped — never a stub
    /// entry (matches `SlideInstance::arrange`'s own P2 convention).
    #[test]
    fn slide_instance_skips_an_unfilled_layout_slot() {
        use crate::master::{LayoutId, MasterId};

        let shaper = CosmicShaper::headless();
        let title_run = [StyledRun::new("Only the title is filled", font())];
        let title_slot = PlaceholderSlot::new(PlaceholderKind::Title, 0);
        let media_slot = PlaceholderSlot::new(PlaceholderKind::Media, 0);
        let fills = vec![PlaceholderFill::new(title_slot, BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, 400.0))))];
        let instance = SlideInstance::new(LayoutId(1), fills);
        let layout = SlideLayout::new(
            LayoutId(1),
            MasterId(1),
            vec![(title_slot, Rect::new(0.0, 0.0, 400.0, 60.0)), (media_slot, Rect::new(0.0, 60.0, 400.0, 200.0))],
        );

        let slide = slice_slide_instance(&instance, &layout, 400.0, 260.0, SlideOverflow::Report, &style(), &shaper).expect("must not overflow");
        assert_eq!(slide.frame.blocks.len(), 1, "the unfilled Media slot must contribute nothing, never a placeholder stub");
    }

    #[test]
    fn slide_overflow_default_is_report() {
        assert_eq!(SlideOverflow::default(), SlideOverflow::Report);
    }
}
