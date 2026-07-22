//! Caption auto-numbering + in-text cross-references (typography-gap WAVE
//! 3) — extends the same counter-introspection mechanism `scene::block`'s
//! [`OutlineTag`](crate::scene::OutlineTag)/`crate::toc` already implement
//! for the document outline/TOC, applied to figures/tables.
//!
//! ## Numbering — a pure, single-pass function of document order (report:
//! why NO fixpoint, unlike `crate::toc`)
//!
//! [`resolve_caption_numbers`] assigns every [`crate::scene::BlockNode::
//! caption`]-tagged node a `(kind, number)` pair purely from `flow`'s own
//! document order — a per-[`crate::scene::CaptionKind`] counter,
//! incrementing in the SAME order [`crate::scene::resolve_block_ids`]
//! already walks `flow` (design law: reuse the established ordering
//! convention, never a second one). This is DELIBERATELY not built as a
//! multi-pass fixpoint the way `crate::toc::compose_document_with_toc`
//! is: a caption's own number depends ONLY on how many figures/tables
//! precede it in the authored flow — never on anything `compose()`/
//! `slice_pages()` produce (page count, line wrap, pagination). Contrast
//! `crate::toc`'s own fixpoint, which exists because a GENERATED TOC's own
//! page count depends on ITS OWN inserted page count (genuinely circular).
//! No such circularity exists here — resolving refs/numbers is therefore
//! exactly ONE pass, run entirely BEFORE `compose()`/`slice_pages()` ever
//! see the flow, not a slicing-time concern the way `crate::slice::pages`'s
//! own [`crate::slice::OutlineEntry`] collection is (that collection
//! genuinely needs `slice_pages` because it resolves a PAGE index, which
//! this module's numbers never do).
//!
//! ## Cross-references — a typed `Ref(BlockId)`, resolved before `compose()`
//!
//! [`RefSegment::Ref`] is the typed in-text cross-reference primitive this
//! feature's own brief asks for. It is NOT spliced into a
//! `uzor_text::Paragraph`'s own run list as a NEW `uzor-text`-level type —
//! `uzor-text` owns glyph shaping, not document-level identity
//! (`BlockId`/caption numbering are `uzor-typeset` concepts, per this
//! crate's own dependency-boundary law, `CLAUDE.md`'s `Forbidden` list).
//! Instead, [`resolve_refs`] is a pure text-assembly function a caller runs
//! BEFORE constructing the `StyledRun`/`Paragraph` that will contain the
//! resolved reference text (the same "resolve first, then author real
//! runs" ordering `crate::toc::build_toc` already uses for ITS OWN
//! synthesized row text) — see this module's own showcase-wiring note in
//! `export::showcase` for the full authoring sequence.
//!
//! ## Caption placement — a pre-compose flow transform, not new compose code
//!
//! [`attach_captions`] never touches `compose()`/`render.rs`/
//! `export::pdf_adapter.rs` — it splices a synthesized `Block::Paragraph`
//! (the resolved `"Figure 1 — ..."` label) directly AFTER every captioned
//! node. The synthesized LABEL TEXT is arena-backed, via the SAME
//! [`crate::toc::TocArena`] `crate::toc::build_toc` uses (this module's
//! own small, additive reuse of that type — see its own doc comment for
//! the two-phase push/borrow discipline and for exactly why the tiny,
//! per-caption `StyledRun` array still uses `Vec::leak`/`Box::leak`
//! regardless, a genuine Rust type-system wall, not an incomplete fix) —
//! the resulting flow composes/paginates/paints/exports through the
//! EXISTING `Block::Paragraph` pipeline with ZERO new code anywhere else
//! in this crate.

use std::collections::HashMap;

use uzor::fonts::FontFamily;
use uzor_text::{FontSpec, StyledRun};

use crate::scene::{resolve_block_ids, Block, BlockId, BlockNode, CaptionKind};
use crate::toc::TocArena;

/// Style knobs for [`attach_captions`]/[`resolve_refs`]'s generated text —
/// mirrors [`crate::toc::TocStyle`]'s own shape (a plain, caller-suppliable
/// config type, not literally embedded inside [`crate::style::Theme`] —
/// same placement precedent `TocStyle`/`crate::master::PageNumberStyle`
/// already set: "configurable via theme/master" means a caller-suppliable
/// style struct, not a forced `Theme` field).
#[derive(Debug, Clone, Copy)]
pub struct CaptionStyle {
    /// Font every generated caption paragraph paints with.
    pub font: FontSpec,
    /// The FULL caption line's own format — `(kind, number, body_text) ->
    /// "Figure 1 — body_text"` by default ([`default_caption_label`]).
    /// Configurable (this feature's own explicit "not hardcoded in the
    /// engine" ask) — override for a different locale/punctuation
    /// convention (e.g. Russian `"Рисунок 1. body_text"`) without touching
    /// this crate at all.
    pub label: fn(CaptionKind, u32, &str) -> String,
    /// The BARE cross-reference label a [`RefSegment::Ref`] resolves to —
    /// `(kind, number) -> "Figure 3"` by default
    /// ([`default_caption_ref_label`]). Kept SEPARATE from `label` (not
    /// derived from it by string-splitting): a full caption line and an
    /// in-text reference are legitimately different strings in general
    /// (e.g. `"Figure 3"` inline vs. `"Figure 3 — a bar chart..."` under
    /// the figure itself).
    pub ref_label: fn(CaptionKind, u32) -> String,
}

/// `(kind, number, text) -> "Figure 1 — text"` / `"Table 2 — text"`.
pub fn default_caption_label(kind: CaptionKind, number: u32, text: &str) -> String {
    let noun = match kind {
        CaptionKind::Figure => "Figure",
        CaptionKind::Table => "Table",
    };
    format!("{noun} {number} — {text}")
}

/// `(kind, number) -> "Figure 3"` / `"Table 1"`.
pub fn default_caption_ref_label(kind: CaptionKind, number: u32) -> String {
    let noun = match kind {
        CaptionKind::Figure => "Figure",
        CaptionKind::Table => "Table",
    };
    format!("{noun} {number}")
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self { font: FontSpec::new(FontFamily::Roboto, 11.0), label: default_caption_label, ref_label: default_caption_ref_label }
    }
}

/// Assign every [`crate::scene::BlockNode::caption`]-tagged node in `flow`
/// a `(kind, number)` pair — a per-[`CaptionKind`] counter incrementing in
/// document order (1-based). Pure function of `flow`'s own shape (see this
/// module's own top doc comment for why this needs no fixpoint, unlike
/// `crate::toc`'s own page-number resolution) — stable across repeated
/// calls over the same tree, matching [`resolve_block_ids`]'s own
/// stability guarantee.
pub fn resolve_caption_numbers(flow: &[BlockNode<'_>]) -> HashMap<BlockId, (CaptionKind, u32)> {
    let ids = resolve_block_ids(flow);
    let mut counters: HashMap<CaptionKind, u32> = HashMap::new();
    let mut out = HashMap::new();
    for (node, id) in flow.iter().zip(ids.iter()) {
        if let Some(caption) = &node.caption {
            let counter = counters.entry(caption.kind).or_insert(0);
            *counter += 1;
            out.insert(*id, (caption.kind, *counter));
        }
    }
    out
}

/// Splice a synthesized `Block::Paragraph` (the resolved
/// `"{kind} {number} — {text}"` caption line, via [`CaptionStyle::label`])
/// directly AFTER every [`crate::scene::BlockNode::caption`]-tagged node in
/// `flow` — see this module's own top doc comment for why this needs no
/// change anywhere else in this crate (composes/paginates/paints/exports
/// through the existing `Block::Paragraph` pipeline verbatim). A captioned
/// node whose own `break_control` is [`crate::compose::BreakControl::Auto`]
/// is UPGRADED to [`crate::compose::BreakControl::AvoidAfter`] (never
/// downgraded — an author's own `ForceAfter`/`AvoidInside`/etc. is left
/// exactly as authored) so the figure/table and ITS OWN caption never land
/// separated across a page break; an already-non-`Auto` break control is
/// left untouched (the author already made a deliberate choice there).
///
/// `arena` backs every synthesized caption label's TEXT (the SAME
/// [`TocArena`] `crate::toc::build_toc` uses — see that type's own doc
/// comment for the two-phase push/borrow discipline this function follows
/// too: every captioned node's label text is pushed FIRST, in document
/// order, before any of them is borrowed back out to build a real
/// `Paragraph`). The per-caption `StyledRun` array itself still uses
/// `Vec::leak` — a tiny, fixed-size (one `StyledRun` per caption)
/// residual, proportional only to caption COUNT, never to label length;
/// see `TocArena`'s own doc comment for exactly why that can't also move
/// into the arena without `unsafe`.
pub fn attach_captions<'a>(flow: &'a [BlockNode<'a>], style: &CaptionStyle, arena: &'a mut TocArena) -> Vec<BlockNode<'a>> {
    let numbers = resolve_caption_numbers(flow);
    let ids = resolve_block_ids(flow);

    // Phase 1 (mutate): push every captioned node's resolved label text
    // into `arena` BEFORE taking any borrow of it.
    let mut text_index_for: Vec<Option<usize>> = Vec::with_capacity(flow.len());
    for (node, id) in flow.iter().zip(ids.iter()) {
        let idx = node.caption.as_ref().map(|caption| {
            let (kind, number) = numbers.get(id).copied().unwrap_or((caption.kind, 0));
            arena.push((style.label)(kind, number, &caption.text))
        });
        text_index_for.push(idx);
    }

    // Phase 2 (borrow): `arena` is fully populated for this call now —
    // every text lookup below borrows `arena` SHARED, with lifetime 'a
    // (this arena is never mutated again after this point).
    let mut out = Vec::with_capacity(flow.len());
    for ((node, _id), text_index) in flow.iter().zip(ids.iter()).zip(text_index_for) {
        let Some(caption_text_index) = text_index else {
            out.push(reborrow(node));
            continue;
        };
        let mut carried = reborrow(node);
        if carried.break_control == crate::compose::BreakControl::Auto {
            carried.break_control = crate::compose::BreakControl::AvoidAfter;
        }
        out.push(carried);

        let label_text: &'a str = arena.text(caption_text_index);
        let run: &'a [StyledRun<'a>] = &*vec![StyledRun::new(label_text, style.font)].leak();
        let paragraph = uzor_text::Paragraph::new(run, f64::MAX);
        out.push(BlockNode::new(Block::Paragraph(paragraph)));
    }

    out
}

/// Reconstruct a `BlockNode<'a>` carrying the SAME logical content as
/// `node` — needed because [`Block`] is not `Clone` (a
/// [`crate::scene::FigureBlock`]'s own `&'a dyn TypesetFigure` reference IS
/// trivially `Copy`, but the enum as a whole was never given a blanket
/// derive, so this crate's own established "reconstruct field-by-field,
/// never a generic clone" pattern — see `crate::slice::pages`'s own
/// per-page header-placeholder substitution for the identical shape — is
/// used here too. Every field is either `Copy` or (`outline`/`caption`)
/// cheaply `Clone`d.
fn reborrow<'a>(node: &BlockNode<'a>) -> BlockNode<'a> {
    BlockNode {
        id: node.id,
        kind: reborrow_block(&node.kind),
        break_control: node.break_control,
        outline: node.outline.clone(),
        link_target: node.link_target,
        caption: node.caption.clone(),
        footnotes: node.footnotes,
        header_placeholder: node.header_placeholder,
    }
}

/// Field-by-field `Block` reconstruction — see [`reborrow`]'s own doc
/// comment for why this exists instead of a blanket `#[derive(Clone)]`.
fn reborrow_block<'a>(kind: &Block<'a>) -> Block<'a> {
    match kind {
        Block::Paragraph(p) => Block::Paragraph(*p),
        Block::Figure(f) => Block::Figure(crate::scene::FigureBlock::new(f.figure, f.sizing)),
        Block::Image(i) => Block::Image(crate::scene::ImageBlock::new(i.rgba, i.intrinsic_width, i.intrinsic_height, i.sizing, i.fit)),
        Block::Island(isl) => {
            Block::Island(crate::scene::AnchoredIsland::new(
                crate::scene::ImageBlock::new(isl.image.rgba, isl.image.intrinsic_width, isl.image.intrinsic_height, isl.image.sizing, isl.image.fit),
                isl.anchor,
                isl.width,
                isl.margin,
            ))
        }
        Block::Table(t) => Block::Table(crate::scene::TableBlock { columns: t.columns, rows: t.rows, cell_padding: t.cell_padding, header_repeat: t.header_repeat }),
        Block::List(l) => Block::List(crate::scene::ListBlock::new(l.items, l.marker.clone(), l.indent_px)),
        Block::Spacer(g) => Block::Spacer(*g),
    }
}

/// One inline content unit for a caption-cross-reference-aware paragraph
/// (typography-gap WAVE 3) — either literal text or a typed reference to
/// another block's own auto-assigned caption number. See this module's own
/// top doc comment for why this is resolved BEFORE a real
/// `uzor_text::Paragraph` is constructed, not spliced into one directly.
pub enum RefSegment<'a> {
    Text(&'a str),
    Ref(BlockId),
}

/// Resolve `segments` into one owned string: [`RefSegment::Text`] copied
/// verbatim, [`RefSegment::Ref`] substituted with that block's own resolved
/// cross-reference label (e.g. `"Figure 3"`, via [`CaptionStyle::
/// ref_label`]). A dangling reference (an id absent from `numbers` — an
/// authoring bug, e.g. referencing a block that was never `.with_caption`-
/// tagged, or a stale id from a different flow) resolves to a documented,
/// visibly-wrong placeholder rather than panicking — this crate's own
/// "overflow/error returned as data, never silent" convention applied to a
/// bad cross-reference (the SAME "report, don't crash on caller input"
/// stance `crate::master::PageMaster`'s own margin-box overflow already
/// takes).
pub fn resolve_refs(segments: &[RefSegment<'_>], numbers: &HashMap<BlockId, (CaptionKind, u32)>, style: &CaptionStyle) -> String {
    let mut out = String::new();
    for seg in segments {
        match seg {
            RefSegment::Text(t) => out.push_str(t),
            RefSegment::Ref(id) => match numbers.get(id) {
                Some((kind, number)) => out.push_str(&(style.ref_label)(*kind, *number)),
                None => out.push_str("[missing ref]"),
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::render::RenderContext;
    use uzor::types::Rect;
    use uzor_figures::FigureTheme;
    use uzor_text::Paragraph;

    use crate::scene::{Block, TypesetFigure};

    struct StubFig;
    impl TypesetFigure for StubFig {
        fn render(&self, _ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {}
    }

    fn style() -> CaptionStyle {
        CaptionStyle::default()
    }

    fn stub_figure_node(fig: &StubFig) -> BlockNode<'_> {
        BlockNode::new(Block::Figure(crate::scene::FigureBlock::new(fig, crate::scene::BlockSizing::FixedHeight(100.0))))
    }

    #[test]
    fn numbers_increment_per_kind_independently_in_document_order() {
        let a = StubFig;
        let b = StubFig;
        let c = StubFig;

        let flow = [
            stub_figure_node(&a).with_caption(CaptionKind::Figure, "first figure"),
            BlockNode::new(Block::Spacer(4.0)).with_caption(CaptionKind::Table, "a table pretending to be a spacer for this fixture"),
            stub_figure_node(&b).with_caption(CaptionKind::Figure, "second figure"),
            stub_figure_node(&c), // untagged — must not consume a number
        ];

        let numbers = resolve_caption_numbers(&flow);
        let ids = resolve_block_ids(&flow);
        assert_eq!(numbers.get(&ids[0]), Some(&(CaptionKind::Figure, 1)));
        assert_eq!(numbers.get(&ids[1]), Some(&(CaptionKind::Table, 1)), "Table counter is independent of the Figure counter");
        assert_eq!(numbers.get(&ids[2]), Some(&(CaptionKind::Figure, 2)));
        assert_eq!(numbers.get(&ids[3]), None, "an untagged node must never appear in the resolved map");
    }

    #[test]
    fn numbering_is_stable_across_repeated_calls_over_the_same_flow() {
        let a = StubFig;
        let flow = [stub_figure_node(&a).with_caption(CaptionKind::Figure, "x")];
        let first = resolve_caption_numbers(&flow);
        let second = resolve_caption_numbers(&flow);
        assert_eq!(first, second);
    }

    #[test]
    fn attach_captions_splices_a_resolved_label_paragraph_immediately_after_each_captioned_node() {
        let a = StubFig;
        let flow = [stub_figure_node(&a).with_caption(CaptionKind::Figure, "a seeded bar chart")];
        let mut arena = TocArena::new();
        let attached = attach_captions(&flow, &style(), &mut arena);

        assert_eq!(attached.len(), 2, "one figure + one synthesized caption paragraph");
        assert!(matches!(attached[0].kind, Block::Figure(_)));
        let Block::Paragraph(p) = &attached[1].kind else { panic!("caption must be a real Block::Paragraph") };
        assert_eq!(p.runs.len(), 1);
        assert_eq!(p.runs[0].text, "Figure 1 — a seeded bar chart");
    }

    #[test]
    fn attach_captions_upgrades_auto_break_control_to_avoid_after_but_never_overrides_an_explicit_choice() {
        let a = StubFig;
        let b = StubFig;
        let flow = [
            stub_figure_node(&a).with_caption(CaptionKind::Figure, "auto"),
            stub_figure_node(&b).with_caption(CaptionKind::Figure, "explicit").with_break_control(crate::compose::BreakControl::ForceAfter),
        ];
        let mut arena = TocArena::new();
        let attached = attach_captions(&flow, &style(), &mut arena);
        assert_eq!(attached[0].break_control, crate::compose::BreakControl::AvoidAfter, "an Auto captioned node must be upgraded");
        assert_eq!(attached[2].break_control, crate::compose::BreakControl::ForceAfter, "an explicit break control must survive untouched");
    }

    #[test]
    fn attach_captions_leaves_untagged_nodes_completely_unchanged_in_place() {
        let runs = [StyledRun::new("plain paragraph", style().font)];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 200.0)))];
        let mut arena = TocArena::new();
        let attached = attach_captions(&flow, &style(), &mut arena);
        assert_eq!(attached.len(), 1, "an untagged node never gains a synthesized sibling");
    }

    #[test]
    fn resolve_refs_substitutes_a_typed_ref_with_its_resolved_caption_label() {
        let a = StubFig;
        let flow = [stub_figure_node(&a).with_caption(CaptionKind::Figure, "seeded")];
        let numbers = resolve_caption_numbers(&flow);
        let ids = resolve_block_ids(&flow);

        let segments = [RefSegment::Text("As shown in "), RefSegment::Ref(ids[0]), RefSegment::Text(", sales grew.")];
        let text = resolve_refs(&segments, &numbers, &style());
        assert_eq!(text, "As shown in Figure 1, sales grew.");
    }

    #[test]
    fn resolve_refs_resolves_a_dangling_reference_to_a_visible_placeholder_never_a_panic() {
        let numbers: HashMap<BlockId, (CaptionKind, u32)> = HashMap::new();
        let segments = [RefSegment::Text("See "), RefSegment::Ref(BlockId(999)), RefSegment::Text(".")];
        let text = resolve_refs(&segments, &numbers, &style());
        assert_eq!(text, "See [missing ref].");
    }
}
