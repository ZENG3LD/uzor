//! [`Page`]/[`slice_pages`] — the page slicing target (design doc §4.1).
//! Uses [`PageRegionSequence`] under the hood for the page BODY: one
//! region per page, shrinking within a page (a block placed 2/3 down a
//! page only gets the remaining 1/3, then a fresh full region on the next
//! page) — Typst's own model, adopted directly.
//!
//! [`crate::master::PageMaster`] itself (geometry + margins +
//! header/footer + page-number token) moved to `master::page_master` this
//! phase (P2) — see that module's own doc comment for why. This module
//! keeps [`Page`] + [`slice_pages`] + the header/footer/page-number
//! WIRING: header/footer margin-box content is composed ONCE (via a
//! [`FixedRegionSequence`] over its own margin band) and reused UNCHANGED
//! on every page — the ONLY per-page-varying piece is the page-number
//! STRING, precomputed here at slice time via
//! [`crate::master::PageNumberStyle::format_for`] (design doc §4.1:
//! "page_number_token ... resolved per-page during slicing").
//!
//! ## Multi-column pages — `Page` holds N column frames (coordinator
//! decision, report)
//!
//! When [`crate::master::PageMaster::columns`] is `> 1`,
//! [`PageRegionSequence::with_columns`] yields `columns` region PULLS per
//! physical page (column 1, column 2, ..., then the next page's column 1,
//! ...) — [`compose`] therefore returns `columns` [`Frame`]s per page
//! instead of one. Rather than refactor `Page::frame: Frame<'a>` into a
//! `Vec` (which would touch every existing caller of that field —
//! `render.rs`, `export/pdf_adapter.rs`, and this crate's own P0-P4
//! proofs), [`Page`] gains an ADDITIVE [`Page::extra_frames`] field:
//! `frame` stays column 1 (byte-identical to the pre-column single-frame
//! shape when `columns == 1`, since `extra_frames` is simply empty then),
//! `extra_frames` holds columns 2..N. `crate::render::draw_page_layers`/
//! `export::pdf_adapter::pages_to_pdf` both walk `extra_frames` the SAME
//! way they already walk `frame`/`header`/`footer` (design law 1 — same
//! paint/collection recursion, just one more frame list).

use std::collections::{HashMap, HashSet};

use uzor::types::Rect;
use uzor_text::{LineShaper, Paragraph, StyledRun};

use crate::compose::{compose, ComposeStyle};
use crate::master::{HeaderPlaceholder, PageMaster, PageNumberStyle};
use crate::region::{FixedRegionSequence, Frame, PageRegionSequence, Region};
use crate::scene::{resolve_block_ids, Block, BlockId, BlockNode, Footnote, ListBlock, ListItem, MarkerStyle};

/// One sliced page: its 0-based index + the composed [`Frame`] that
/// landed on it, plus (P2) the shared header/footer frames and this
/// page's own resolved page-number placement.
pub struct Page<'a> {
    pub index: u32,
    /// Total pages this `slice_pages` call produced — the SAME value on
    /// every `Page` it returns, needed for `"n of total"`-style page
    /// numbering.
    pub total: u32,
    /// This page's own FIRST column (column 1 when
    /// [`crate::master::PageMaster::columns`] is `> 1`; the WHOLE body
    /// when it's `1`, unchanged from every pre-column phase).
    pub frame: Frame<'a>,
    /// This page's remaining columns (2..N), in order — always empty
    /// when `columns <= 1` (see this module's own "Multi-column pages"
    /// doc comment for why this is additive rather than a `frame: Vec`
    /// refactor).
    pub extra_frames: Vec<Frame<'a>>,
    /// [`crate::master::PageMaster::header`] composed ONCE, shared
    /// verbatim across every page (`None` when the master has no header).
    pub header: Option<Frame<'a>>,
    /// [`crate::master::PageMaster::footer`] composed ONCE, shared
    /// verbatim across every page (`None` when the master has no footer).
    pub footer: Option<Frame<'a>>,
    /// This page's own resolved page-number text + paint rect (`None`
    /// when the master has no [`crate::master::PageMaster::
    /// page_number_token`]).
    pub page_number: Option<PageNumberPlacement>,
    /// Every [`crate::scene::BlockNode::with_outline`]-tagged block that
    /// landed on THIS page (document order), each already resolved to
    /// `page_index == self.index` at [`slice_pages`] time — document
    /// navigation feature pass. Additive shape decision (report): rather
    /// than a second `slice_pages_with_outline(..) -> (Vec<Page>,
    /// Vec<OutlineEntry>)` entry point, entries attach directly to the
    /// `Page` they landed on — matching this module's own established
    /// convention (`header`/`footer`/`page_number` already attach to
    /// `Page` rather than growing `slice_pages`'s own return shape).
    /// [`renumber_pages`] is the "merge outlines across a multi-run
    /// concatenation" helper this attachment shape needs: it re-derives
    /// every contained entry's own `page_index` from its containing
    /// page's (possibly renumbered) `Page::index` — always correct
    /// regardless of how many independent `slice_pages` calls were
    /// concatenated beforehand, since it's recomputed fresh from the
    /// FINAL page positions, never carried forward stale.
    pub outline: Vec<OutlineEntry>,
    /// Every [`crate::scene::BlockNode::with_link_target`]-tagged block's
    /// own placed rect(s) on THIS page — one [`LinkEntry`] per placed
    /// FRAGMENT (a block split across pages gets one entry per fragment,
    /// each covering only that fragment's own on-page rect; unlike
    /// [`Page::outline`], this is intentionally NOT deduplicated to "first
    /// occurrence only" — every visible fragment is a real clickable
    /// area). `target_page` is expected to ALREADY be expressed in
    /// whatever the FINAL concatenated document's page-index space will
    /// be (the same expectation `crate::toc::compose_document_with_toc`'s
    /// own generated rows satisfy via `OutlineEntry::page_index`) —
    /// [`renumber_pages`] deliberately does NOT rewrite `target_page`
    /// (unlike `OutlineEntry::page_index`, which is self-referential to
    /// the page it's attached to and therefore always safe to overwrite;
    /// a link's target is a DIFFERENT page, which `renumber_pages` has no
    /// way to remap without knowing the caller's own pre-concatenation
    /// section boundaries).
    pub links: Vec<LinkEntry>,
    /// This page's own resolved footnote zone (typography-gap WAVE 3) —
    /// `None` when [`crate::master::PageMaster::footnote_zone_height`] is
    /// unset OR no footnote marker landed on this specific page. See
    /// [`FootnotePlacement`]/this module's own "Footnotes" doc section.
    pub footnotes: Option<FootnotePlacement<'a>>,
}

/// This page's own resolved footnote zone content (typography-gap WAVE 3)
/// — see this module's own "Footnotes" doc section for the resolution
/// mechanism.
pub struct FootnotePlacement<'a> {
    /// The footnote zone's own composed content (a single
    /// [`crate::scene::Block::List`], `MarkerStyle::Numbered`, reusing the
    /// EXISTING list-splitting/marker/paint/PDF-export machinery wholesale
    /// — no new render/export code needed for footnote numbering or
    /// marker glyphs). `Frame::region.rect` equals [`crate::master::
    /// PageMaster::footnote_zone_rect`] exactly.
    pub frame: Frame<'a>,
    /// `true` when at least one footnote on this page didn't fully fit
    /// [`crate::master::PageMaster::footnote_zone_height`]'s own reserved
    /// band — v1 honest limit: NO multi-page footnote splitting (a
    /// footnote's own content never spills onto a second page's own
    /// zone); the overflowing remainder is placed visibly past the
    /// zone's own reserved height rather than silently dropped (this
    /// crate's own established "overflow returned as data, never silent"
    /// convention — the SAME degrade [`PageMaster::header`]/
    /// [`PageMaster::footer`]'s own margin-box overflow already uses,
    /// applied here to the footnote zone).
    pub clamped: bool,
}

/// One document-outline/TOC entry, resolved to a concrete page (design
/// doc: counter-introspection multi-pass fixpoint — this is the
/// "introspection index" half: `slice_pages` is the one pass that knows
/// where every tagged block actually landed). `page_index` is 0-based,
/// matching [`Page::index`]'s own convention.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub page_index: u32,
}

/// One internal-link (GoTo) source rect, resolved at [`slice_pages`] time
/// from a [`crate::scene::BlockNode::with_link_target`]-tagged block's own
/// placed [`crate::region::PlacedBlock::rect`] (design law 1 — no second
/// position formula: the SAME rect the block itself painted at).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkEntry {
    pub rect: Rect,
    pub target_page: u32,
}

/// A page's own resolved page-number text, ready to paint at `rect`
/// verbatim (design law 1 — text AND position both resolved once, at
/// slice time, never re-derived at paint time).
#[derive(Debug, Clone, PartialEq)]
pub struct PageNumberPlacement {
    pub text: String,
    pub rect: Rect,
}

/// Compose `flow` over `master`'s page geometry and slice the result into
/// pages — one [`Page`] per [`Frame`] `compose()` produced. Stops as soon
/// as every block is placed (never emits a trailing empty page).
/// [`crate::master::PageMaster::header`]/`footer` (if present) are
/// composed once against their own margin bands and attached to every
/// returned `Page` unchanged; [`crate::master::PageMaster::
/// page_number_token`] (if present) is formatted per-page from this
/// call's own total page count.
///
/// `style`/`shaper` are not part of the design doc's own abbreviated
/// `slice_pages(scene, master)` pseudocode (§4.1) — added here for the
/// same reason [`compose`] needs them (see this crate's `CLAUDE.md`
/// "Divergences from the design doc"), and because P0 does not build
/// `SceneSpec`/`Theme` (§2.1/§5 wasn't part of P0's own deliverables
/// list), `flow` is taken directly rather than via a `&SceneSpec`. This
/// signature is UNCHANGED from P0/P1 (additive law: `PageMaster`/`Page`
/// grew new fields this phase, `slice_pages` needed no new parameter to
/// wire them — painting a theme onto the result is `crate::render::
/// draw_page`'s own job, not this function's).
pub fn slice_pages<'a>(flow: &'a [BlockNode<'a>], master: &PageMaster<'a>, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Page<'a>> {
    let mut regions = PageRegionSequence::new(master.body_rect()).with_columns(master.columns, master.column_gap);
    let raw_frames = compose(flow, &mut regions, style, shaper);
    let columns = master.columns.max(1);

    let mut pages: Vec<Page<'a>> = Vec::new();
    let mut raw_frames = raw_frames.into_iter();
    let mut index = 0u32;
    while let Some(frame) = raw_frames.next() {
        let mut extra_frames = Vec::new();
        for _ in 1..columns {
            match raw_frames.next() {
                Some(f) => extra_frames.push(f),
                None => break,
            }
        }
        pages.push(Page {
            index,
            total: 0,
            frame,
            extra_frames,
            header: None,
            footer: None,
            page_number: None,
            outline: Vec::new(),
            links: Vec::new(),
            footnotes: None,
        });
        index += 1;
    }

    let total = pages.len() as u32;

    // Running headers (typography-gap WAVE 3): whichever of header/footer
    // carries NO `.with_header_placeholder()`-tagged node keeps the
    // ORIGINAL "compose once, shared verbatim across every page" path
    // byte-identical to every pre-WAVE-3 caller. A placeholder-bearing
    // side is left `None` here and resolved per-page BELOW, once
    // `attach_navigation_entries` has populated `Page::outline` (the
    // running title's own source of truth).
    let header_has_placeholder = master.header.is_some_and(header_uses_placeholder);
    let footer_has_placeholder = master.footer.is_some_and(header_uses_placeholder);
    let header_frame = if header_has_placeholder { None } else { master.header.map(|content| compose_margin_box(content, master.header_rect(), style, shaper)) };
    let footer_frame = if footer_has_placeholder { None } else { master.footer.map(|content| compose_margin_box(content, master.footer_content_rect(), style, shaper)) };

    for page in &mut pages {
        page.total = total;
        page.header = header_frame.clone();
        page.footer = footer_frame.clone();
        page.page_number = master
            .page_number_token
            .as_ref()
            .map(|token| PageNumberPlacement { text: token.format_for(page.index, total), rect: master.page_number_rect() });
    }

    attach_navigation_entries(flow, &mut pages);

    if header_has_placeholder || footer_has_placeholder {
        let titles = resolve_running_titles(&pages);
        for (page, title) in pages.iter_mut().zip(titles.iter()) {
            if header_has_placeholder {
                if let Some(content) = master.header {
                    page.header = Some(compose_placeholder_margin_box(content, master.header_rect(), style, shaper, title));
                }
            }
            if footer_has_placeholder {
                if let Some(content) = master.footer {
                    page.footer = Some(compose_placeholder_margin_box(content, master.footer_content_rect(), style, shaper, title));
                }
            }
        }
    }

    attach_footnotes(flow, &mut pages, master, style, shaper);

    pages
}

/// `true` when any node in `content` is tagged
/// [`crate::scene::BlockNode::with_header_placeholder`] — the guard
/// deciding whether [`slice_pages`] takes the ORIGINAL "compose once"
/// path or the per-page [`compose_placeholder_margin_box`] path for that
/// side (header or footer).
fn header_uses_placeholder(content: &[BlockNode<'_>]) -> bool {
    content.iter().any(|n| n.header_placeholder.is_some())
}

/// This page's own "current chapter" running-header title: the title of
/// the LAST [`OutlineEntry`] on/before this page (an outline entry from a
/// LATER page never counts) — `""` on every page before the document's
/// first `.with_outline()`-tagged block. A single forward pass over
/// `pages` (already in final document order) carrying the most recent
/// title seen so far.
fn resolve_running_titles(pages: &[Page<'_>]) -> Vec<String> {
    let mut current = String::new();
    pages
        .iter()
        .map(|page| {
            if let Some(last) = page.outline.last() {
                current = last.title.clone();
            }
            current.clone()
        })
        .collect()
}

/// Reconstruct a `BlockNode<'a>` sharing `node`'s own non-`kind` fields
/// with a NEW `kind` — [`Block`] carries no blanket `#[derive(Clone)]` (a
/// deliberate choice, see `crate::caption::reborrow`'s own doc comment for
/// the full rationale), so [`resolve_header_content`] reconstructs
/// field-by-field instead of a generic clone.
fn rebuild_simple_node<'a>(node: &BlockNode<'a>, kind: Block<'a>) -> BlockNode<'a> {
    BlockNode {
        id: node.id,
        kind,
        break_control: node.break_control,
        outline: node.outline.clone(),
        link_target: node.link_target,
        caption: node.caption.clone(),
        footnotes: node.footnotes,
        header_placeholder: node.header_placeholder,
    }
}

/// Build a PER-PAGE substituted copy of placeholder-bearing margin-box
/// `content`: a [`HeaderPlaceholder::CurrentOutlineTitle`]-tagged
/// `Block::Paragraph` node has its own runs REPLACED (never appended to)
/// by a single run carrying `title` verbatim, in the SAME font its own
/// first authored run used (falling back to `style.default_font` for an
/// empty-runs paragraph) — `title`'s owned text is leaked to `'static`
/// (`crate::toc::build_toc`'s own established "synthesized text needs
/// owned backing" convention), which trivially coerces to this function's
/// own `'a` via reference covariance.
///
/// **v1 scope limit (report, not silent):** only `Block::Paragraph`/
/// `Block::Spacer` nodes may share a placeholder-bearing header/footer — a
/// header/footer carrying any OTHER block kind (a figure/table/list/
/// image/island) alongside a placeholder tag has that OTHER node silently
/// DROPPED from this page's own per-page recompose (never a panic; the
/// SAME-content non-placeholder side of the SAME header/footer keeps
/// working normally via the unaffected "compose once" path). Margin-box
/// content in every fixture this crate ships is already paragraph/spacer-
/// only (a header/footer is expected to be a couple of short lines), so
/// this is a genuine, narrow, documented v1 limit, not an observed
/// regression.
fn resolve_header_content<'a>(content: &'a [BlockNode<'a>], title: &str, style: &ComposeStyle) -> Vec<BlockNode<'a>> {
    content
        .iter()
        .filter_map(|node| {
            if node.header_placeholder == Some(HeaderPlaceholder::CurrentOutlineTitle) {
                let Block::Paragraph(p) = &node.kind else { return None };
                let leaked_text: &'static str = Box::leak(title.to_owned().into_boxed_str());
                let font = p.runs.first().map(|r| r.font).unwrap_or(style.default_font);
                let run: &'static [StyledRun<'static>] = &*vec![StyledRun::new(leaked_text, font)].leak();
                let substituted = Paragraph { runs: run, ..*p };
                return Some(rebuild_simple_node(node, Block::Paragraph(substituted)));
            }
            match &node.kind {
                Block::Paragraph(p) => Some(rebuild_simple_node(node, Block::Paragraph(*p))),
                Block::Spacer(g) => Some(rebuild_simple_node(node, Block::Spacer(*g))),
                _ => None,
            }
        })
        .collect()
}

/// Compose placeholder-bearing margin-box `content` for ONE specific page
/// (`title` already resolved via [`resolve_running_titles`]) — the per-page
/// counterpart to [`compose_margin_box`]'s own "compose once, shared
/// verbatim" path. The substituted flow's own backing `Vec` is leaked
/// (bounded: one leak per page per placeholder-bearing side, the same
/// documented trade-off `crate::toc::compose_document_with_toc`'s own
/// per-iteration leak already uses) so the returned `Frame<'a>` can borrow
/// from it.
fn compose_placeholder_margin_box<'a>(content: &'a [BlockNode<'a>], rect: Rect, style: &ComposeStyle, shaper: &dyn LineShaper, title: &str) -> Frame<'a> {
    let substituted: &'a [BlockNode<'a>] = resolve_header_content(content, title, style).leak();
    compose_margin_box(substituted, rect, style, shaper)
}

/// Walk `flow`'s own `.with_outline()`/`.with_link_target()`-tagged nodes
/// and attach the resulting [`OutlineEntry`]/[`LinkEntry`]s to whichever
/// [`Page`]s their placed content appears on — never header/footer (a
/// SEPARATE flow/id namespace, see [`compose_margin_box`]'s own call
/// sites: cross-referencing `flow`'s ids against header/footer
/// `PlacedBlock`s would be comparing [`BlockId`]s from two UNRELATED
/// [`resolve_block_ids`] calls, which could coincidentally collide). An
/// outline-tagged block is attached to its FIRST page only (`emitted`
/// guards against a duplicate entry for a later fragment sharing the same
/// id) — a link-tagged block is attached on EVERY page its own fragments
/// land on (see [`Page::links`]'s own doc comment for why the two differ).
fn attach_navigation_entries<'a>(flow: &'a [BlockNode<'a>], pages: &mut [Page<'a>]) {
    let ids = resolve_block_ids(flow);
    let mut outline_tags: HashMap<BlockId, &crate::scene::OutlineTag> = HashMap::new();
    let mut link_targets: HashMap<BlockId, u32> = HashMap::new();
    for (node, id) in flow.iter().zip(ids.iter()) {
        if let Some(tag) = &node.outline {
            outline_tags.insert(*id, tag);
        }
        if let Some(target) = node.link_target {
            link_targets.insert(*id, target);
        }
    }
    if outline_tags.is_empty() && link_targets.is_empty() {
        return;
    }

    let mut emitted: HashSet<BlockId> = HashSet::new();
    for page in pages.iter_mut() {
        let mut outline_entries = Vec::new();
        let mut link_entries = Vec::new();
        for placed in page.frame.blocks.iter().chain(page.extra_frames.iter().flat_map(|f| f.blocks.iter())) {
            if let Some(tag) = outline_tags.get(&placed.id) {
                if emitted.insert(placed.id) {
                    outline_entries.push(OutlineEntry { level: tag.level, title: tag.title.clone(), page_index: page.index });
                }
            }
            if let Some(&target_page) = link_targets.get(&placed.id) {
                link_entries.push(LinkEntry { rect: placed.rect, target_page });
            }
        }
        page.outline = outline_entries;
        page.links = link_entries;
    }
}

/// Renumber a concatenation of several independently-sliced [`Page`]
/// sections into one continuous document (the showcase's own 3-run
/// concat pattern, and [`crate::toc::compose_document_with_toc`]'s own
/// TOC-pages-then-body-pages concatenation) — fixes up `Page::index`/
/// `Page::total`, every contained [`OutlineEntry::page_index`] (so a
/// generated TOC/PDF-outline built from a LATER `pages[i].outline` read
/// always matches this call's own FINAL page positions, never a stale
/// per-section index), and — when `page_number_style` is given —
/// re-formats every already-attached [`PageNumberPlacement::text`]. Only
/// TEXT/index bookkeeping is rewritten; every already-baked geometry
/// field (frame/extra_frames/header/footer content, the page-number's own
/// paint rect) is untouched.
pub fn renumber_pages<'a>(mut pages: Vec<Page<'a>>, page_number_style: Option<&PageNumberStyle>) -> Vec<Page<'a>> {
    let total = pages.len() as u32;
    for (i, page) in pages.iter_mut().enumerate() {
        page.index = i as u32;
        page.total = total;
        for entry in &mut page.outline {
            entry.page_index = page.index;
        }
        if let Some(style) = page_number_style {
            if let Some(number) = &mut page.page_number {
                number.text = style.format_for(page.index, total);
            }
        }
    }
    pages
}

/// Compose fixed margin-box content (header/footer) ONCE into a single
/// [`Frame`] at `rect` — the SAME content painted unchanged on every page
/// (design doc §4.1). Content that overflows `rect` degrades the same
/// documented way any atomic overflow elsewhere in this crate does (P0's
/// own risk note): whatever fits is placed, the remainder is simply not
/// represented (a margin box has no second region to spill into — a
/// header/footer is expected to be a couple of short lines).
fn compose_margin_box<'a>(content: &'a [BlockNode<'a>], rect: Rect, style: &ComposeStyle, shaper: &dyn LineShaper) -> Frame<'a> {
    let mut regions = FixedRegionSequence::new(rect);
    compose(content, &mut regions, style, shaper)
        .into_iter()
        .next()
        .unwrap_or(Frame { region: Region { rect }, blocks: Vec::new(), overflow: None })
}

/// Marker-gutter width the footnote zone's own generated
/// [`crate::scene::ListBlock`] uses — this feature's own v1 fixed choice
/// (footnote lists are short, numbered `"1."`.."99."`-ish; no per-document
/// config knob this pass adds, matching [`crate::compose::list_layout`]'s
/// own marker font reuse of `ComposeStyle::default_font` for the identical
/// "no new style field this phase needs" reasoning).
const FOOTNOTE_MARKER_INDENT: f64 = 20.0;

/// **Footnotes (typography-gap WAVE 3).** A footnote is authored as an
/// INLINE MARKER inside a `Block::Paragraph`'s own runs — see
/// [`crate::scene::Footnote`]'s own doc comment for the exact
/// `uzor_text::InlineBox`-splicing convention. This function is the
/// resolution pass: walk every already-composed [`Page`]'s own placed
/// paragraph blocks, recover which footnote marker (if any) landed on
/// THIS page (via `ParagraphLayout::boxes`, correlated back to
/// [`crate::scene::BlockNode::footnotes`] by the marker's own local
/// index), and compose the matched footnotes into
/// [`crate::master::PageMaster::footnote_zone_rect`] as a real
/// [`crate::scene::ListBlock`] (`MarkerStyle::Numbered`, numbered
/// CONTINUOUSLY across the WHOLE document — a pure function of flow
/// order, resolved once up front, the identical "no fixpoint needed, no
/// circular dependency on pagination" reasoning `crate::caption::
/// resolve_caption_numbers`'s own doc comment already gives for caption
/// numbers).
///
/// A no-op (every `Page::footnotes` stays `None`) when
/// [`crate::master::PageMaster::footnote_zone_height`] is unset — the
/// SAME "additive, zero cost when not configured" guard
/// [`header_uses_placeholder`] uses for running headers.
fn attach_footnotes<'a>(flow: &'a [BlockNode<'a>], pages: &mut [Page<'a>], master: &PageMaster<'a>, style: &ComposeStyle, shaper: &dyn LineShaper) {
    if master.footnote_zone_height.is_none() {
        return;
    }

    let ids = resolve_block_ids(flow);
    let mut registry: HashMap<BlockId, &'a [Footnote<'a>]> = HashMap::new();
    for (node, id) in flow.iter().zip(ids.iter()) {
        if !node.footnotes.is_empty() {
            registry.insert(*id, node.footnotes);
        }
    }
    if registry.is_empty() {
        return;
    }

    // Global, continuous numbering — a pure function of flow order (which
    // paragraph, which local marker index within it), never of page
    // assignment (that's what THIS function is still in the middle of
    // discovering) — no circularity, no fixpoint.
    let mut numbers: HashMap<(BlockId, u64), u32> = HashMap::new();
    let mut next_number = 1u32;
    for (node, id) in flow.iter().zip(ids.iter()) {
        for local_index in 0..node.footnotes.len() {
            numbers.insert((*id, local_index as u64), next_number);
            next_number += 1;
        }
    }

    let zone_rect = master.footnote_zone_rect();
    for page in pages.iter_mut() {
        let mut found: Vec<(u32, &'a [BlockNode<'a>])> = Vec::new();
        for placed in page.frame.blocks.iter().chain(page.extra_frames.iter().flat_map(|f| f.blocks.iter())) {
            let Some(footnotes) = registry.get(&placed.id) else { continue };
            let Some(layout) = &placed.paragraph_layout else { continue };
            for placed_box in &layout.boxes {
                let Some(footnote) = footnotes.get(placed_box.id as usize) else { continue };
                let number = numbers.get(&(placed.id, placed_box.id)).copied().unwrap_or(0);
                found.push((number, footnote.content));
            }
        }
        if found.is_empty() {
            continue;
        }
        let (frame, clamped) = compose_footnote_zone(&found, zone_rect, style, shaper);
        page.footnotes = Some(FootnotePlacement { frame, clamped });
    }
}

/// Compose `footnotes` (already resolved to `(global_number, content)`
/// pairs, in the order their own markers were encountered — flow order,
/// hence globally-consecutive numbers on any one page) into `zone_rect` as
/// ONE [`crate::scene::ListBlock`] — reuses list splitting/marker/paint/
/// PDF-export machinery wholesale (design law 1). `clamped` is `true` when
/// [`Frame::overflow`] reports the list didn't fully fit — v1 honest limit:
/// no second page for the overflow, see [`FootnotePlacement::clamped`]'s
/// own doc comment.
fn compose_footnote_zone<'a>(footnotes: &[(u32, &'a [BlockNode<'a>])], zone_rect: Rect, style: &ComposeStyle, shaper: &dyn LineShaper) -> (Frame<'a>, bool) {
    let empty_frame = || Frame { region: Region { rect: zone_rect }, blocks: Vec::new(), overflow: None };
    if footnotes.is_empty() || zone_rect.height <= 0.0 {
        return (empty_frame(), false);
    }

    let start = footnotes[0].0;
    let items: Vec<ListItem<'a>> = footnotes.iter().map(|(_, content)| ListItem::new(content)).collect();
    let items: &'a [ListItem<'a>] = items.leak();
    let list = ListBlock::new(items, MarkerStyle::numbered(start), FOOTNOTE_MARKER_INDENT);
    let list_flow: &'a [BlockNode<'a>] = vec![BlockNode::new(Block::List(list))].leak();

    let mut regions = FixedRegionSequence::new(zone_rect);
    let frame = compose(list_flow, &mut regions, style, shaper).into_iter().next().unwrap_or_else(empty_frame);

    // `Frame::overflow` alone under-detects: a SINGLE footnote taller than
    // the WHOLE (otherwise-empty) zone force-places via `compose::
    // list_layout::items_fitting`'s own "a fresh region must make
    // progress even if the item overflows it" degrade — which reports
    // `next_item + count >= total_items` (fully placed, `overflow: None`)
    // even though the placed item's own rect plainly exceeds the zone's
    // bottom edge. The SAME fix `crate::slice::slides`'s own P3 divergence
    // #2 already documents for fixed-slide overflow detection applies
    // here verbatim: also check whether any placed block's own bottom
    // edge exceeds the target bound.
    let overflows_bottom_edge = frame.blocks.iter().any(|b| b.rect.y + b.rect.height > zone_rect.y + zone_rect.height + 1e-6);
    let clamped = frame.overflow.is_some() || overflows_bottom_edge;
    (frame, clamped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::master::{Margins, PageNumberFormat, PageNumberStyle};
    use crate::scene::{Block, BlockNode};

    /// A repeated sentence long enough that ~2.5 pages of it wrap to 3
    /// pages at the fixture's chosen width/font/page size, with the last
    /// page only partially filled.
    const SENTENCE: &str =
        "This fixed seeded sentence exists only to consume vertical space \
        for the page-slicing test, repeated enough times to overflow more \
        than two pages but noticeably less than three whole pages of body \
        text at this fixture's chosen page geometry.";

    /// Wraps to several lines at a ~400px width — tall enough to overflow
    /// the remaining 1/3 of the 300px-tall fixture page used by the
    /// region-shrink test below.
    const WRAPPING_PARAGRAPH: &str =
        "A paragraph placed near the bottom of the page, deliberately long \
        enough that it must wrap across several lines and therefore cannot \
        possibly fit whole into the small remaining sliver of vertical \
        space left below a filler spacer that already consumed two thirds \
        of this fixture's short page body.";

    fn body_font() -> FontSpec {
        FontSpec::new(FontFamily::Roboto, 16.0)
    }

    #[test]
    fn content_worth_about_two_and_a_half_pages_slices_into_exactly_three() {
        let font = body_font();
        // 20 short paragraphs at a 16px line height comfortably exceed 2
        // pages of a 595x842 (A4-ish) body at 40px margins, and land short
        // of a full 3rd page.
        let paragraph_texts: Vec<String> = (0..20).map(|i| format!("Paragraph {i}: {SENTENCE}")).collect();
        let runs: Vec<[uzor_text::StyledRun<'_>; 1]> =
            paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> = runs
            .iter()
            .map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, 515.0))))
            .collect();

        let master = PageMaster::new(595.0, 842.0, Margins::uniform(40.0));
        let style = ComposeStyle::new(12.0, font);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 3, "fixture is tuned to land on exactly 3 pages, got {}", pages.len());

        for page in &pages[..2] {
            assert!(!page.frame.blocks.is_empty(), "every non-final page must carry content");
        }
        let last = pages.last().expect("at least one page");
        assert!(!last.frame.blocks.is_empty(), "the final page must still carry its partial content");

        // Determinism: composing the exact same flow twice must produce
        // the exact same page count and per-page block count.
        let pages_again = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), pages_again.len());
        for (a, b) in pages.iter().zip(pages_again.iter()) {
            assert_eq!(a.frame.blocks.len(), b.frame.blocks.len());
        }
    }

    #[test]
    fn a_block_placed_two_thirds_down_a_page_only_gets_the_remaining_third() {
        let font = body_font();
        let master = PageMaster::new(400.0, 300.0, Margins::uniform(0.0));
        let body = master.body_rect();

        let filler_gap = body.height * (2.0 / 3.0);
        // Long enough to wrap to several lines at this width — taller than
        // the remaining 1/3 of the page, so it MUST split across pages
        // rather than fit whole (the meaningful version of this test: a
        // short one-liner would fit either way and prove nothing about
        // region-shrink).
        let runs = [StyledRun::new(WRAPPING_PARAGRAPH, font)];
        let flow = [BlockNode::new(Block::Spacer(filler_gap)), BlockNode::new(Block::Paragraph(Paragraph::new(&runs, body.width)))];

        let style = ComposeStyle::new(0.0, font);
        let shaper = CosmicShaper::headless();

        // Independently confirm the paragraph's OWN full layout (measured
        // at the page body's width) is taller than the remaining 1/3 of
        // the page — otherwise this test's split assumption is untuned.
        let full_layout = layout_paragraph(&Paragraph::new(&runs, body.width), &shaper);
        let remaining_before_paragraph = body.height - filler_gap;
        assert!(
            full_layout.height > remaining_before_paragraph,
            "fixture must be tall enough to overflow the remaining 1/3 of the page"
        );

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture must split across exactly 2 pages");

        let first_page = &pages[0];
        let paragraph_placement = first_page
            .frame
            .blocks
            .iter()
            .find(|b| matches!(b.kind, Block::Paragraph(_)))
            .expect("paragraph must be placed on the first page (even if only partially)");

        assert!(
            paragraph_placement.rect.height <= remaining_before_paragraph + 1.0,
            "paragraph placed 2/3 down the page must only get the remaining 1/3 of region height, got {} vs budget {}",
            paragraph_placement.rect.height,
            remaining_before_paragraph
        );

        let second_page = &pages[1];
        let remainder_placement = second_page
            .frame
            .blocks
            .iter()
            .find(|b| matches!(b.kind, Block::Paragraph(_)))
            .expect("the paragraph's remainder must continue onto the second page");
        let remainder_layout = remainder_placement.paragraph_layout.as_ref().expect("paragraph placements always carry a layout");
        let head_layout = paragraph_placement.paragraph_layout.as_ref().expect("paragraph placements always carry a layout");
        assert_eq!(
            head_layout.lines.len() + remainder_layout.lines.len(),
            full_layout.lines.len(),
            "every line of the paragraph must be conserved across the split, none dropped or duplicated"
        );
    }

    /// P2: a master's header/footer content + page-number token must be
    /// attached to EVERY sliced page, with the correct `"n of total"`
    /// text per page (design doc §4.1: "resolved per-page during
    /// slicing").
    #[test]
    fn header_footer_and_page_number_are_attached_to_every_sliced_page() {
        let font = body_font();
        let shaper = CosmicShaper::headless();

        let header_run = [StyledRun::new("Header", font)];
        let header_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, 200.0)))];
        let footer_run = [StyledRun::new("Footer", font)];
        let footer_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&footer_run, 200.0)))];

        let master = PageMaster::new(400.0, 300.0, Margins::uniform(30.0))
            .with_header(&header_flow)
            .with_footer(&footer_flow)
            .with_page_number(PageNumberStyle::new(PageNumberFormat::OfTotal, 1));

        // Enough repeated paragraphs to force at least 2 pages of this
        // small (400x300, 30px margins) fixture page.
        let paragraph_texts: Vec<String> = (0..8).map(|i| format!("Paragraph {i}: {SENTENCE}")).collect();
        let runs: Vec<[StyledRun<'_>; 1]> = paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> =
            runs.iter().map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, master.body_rect().width)))).collect();

        let style = ComposeStyle::new(0.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert!(pages.len() >= 2, "fixture must span at least 2 pages to prove the number/total wiring");

        for (i, page) in pages.iter().enumerate() {
            assert!(page.header.as_ref().is_some_and(|h| !h.blocks.is_empty()), "every page must carry the header");
            assert!(page.footer.as_ref().is_some_and(|f| !f.blocks.is_empty()), "every page must carry the footer");
            let number = page.page_number.as_ref().expect("page-number token was configured on the master");
            assert_eq!(number.text, format!("{} of {}", i + 1, pages.len()));
        }
    }

    #[test]
    fn a_master_without_header_footer_or_page_number_produces_none_for_all_three() {
        let font = body_font();
        let shaper = CosmicShaper::headless();
        let runs = [StyledRun::new("A single short paragraph.", font)];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 400.0)))];
        let master = PageMaster::new(500.0, 700.0, Margins::uniform(40.0));
        let style = ComposeStyle::new(0.0, font);

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].header.is_none());
        assert!(pages[0].footer.is_none());
        assert!(pages[0].page_number.is_none());
    }

    /// `PageMaster::with_columns`: content that overflows a single column
    /// lands in `Page::frame` (column 1) THEN `Page::extra_frames[0]`
    /// (column 2), both on the SAME physical page (`Page::index == 0`) —
    /// never spilling to a second `Page` while column 2 still has room.
    /// Column 1 must fill BEFORE column 2 (every column-1 block's own `y`
    /// stays within column 1's own vertical span before column 2 gets any
    /// content), and every placed block's rect must stay within its own
    /// column's width — never the full body width.
    #[test]
    fn two_column_master_fills_column_one_before_column_two_on_the_same_page() {
        let font = body_font();
        let shaper = CosmicShaper::headless();
        let master = PageMaster::new(600.0, 500.0, Margins::uniform(20.0)).with_columns(2, 18.0);
        let column_width = master.column_width();
        assert!(column_width < master.body_rect().width, "a 2-column body must be narrower per column than the full body");

        // Enough repeated short paragraphs that column 1 fills up and
        // content spills into column 2, but not so much that it spills
        // onto a second physical page.
        let paragraph_texts: Vec<String> = (0..14).map(|i| format!("Column paragraph number {i} of the fixture.")).collect();
        let runs: Vec<[StyledRun<'_>; 1]> = paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> = runs.iter().map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, column_width)))).collect();

        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "fixture must be tuned to fit within column 1 + column 2 of ONE page");
        assert!(!pages[0].frame.blocks.is_empty(), "column 1 must carry content");
        assert!(!pages[0].extra_frames.is_empty(), "column 2 must carry the overflow from column 1");

        let col1_width = pages[0].frame.region.rect.width;
        let col2_width = pages[0].extra_frames[0].region.rect.width;
        assert!((col1_width - column_width).abs() < 1e-6);
        assert!((col2_width - column_width).abs() < 1e-6);

        // Every block placed in column 1 must never exceed column 1's own
        // width, and likewise for column 2 — text never crosses into the
        // other column or the gap between them.
        for placed in &pages[0].frame.blocks {
            assert!(placed.rect.width <= col1_width + 0.01, "column 1 content must never exceed column 1's own width");
        }
        for placed in &pages[0].extra_frames[0].blocks {
            assert!(placed.rect.width <= col2_width + 0.01, "column 2 content must never exceed column 2's own width");
        }

        // Column 1 must be FULLY consumed (its own frame reports
        // overflow — content continued into column 2) before column 2
        // received anything at all — proving "fills column 1 before
        // column 2," not an arbitrary split.
        assert!(pages[0].frame.overflow.is_some(), "column 1 must report overflow into column 2 for this fixture to be meaningful");
    }

    /// A table placed inside a 2-column master sizes its own columns to
    /// the CONTAINING column's width, never the full (wider) page body.
    #[test]
    fn a_table_inside_a_column_is_sized_to_the_column_width_not_the_full_body() {
        use crate::scene::{Block, ColumnSpec, TableBlock, TableCell, TableRow};

        let font = body_font();
        let shaper = CosmicShaper::headless();
        let master = PageMaster::new(600.0, 500.0, Margins::uniform(20.0)).with_columns(2, 18.0);
        let column_width = master.column_width();

        let cell_run = [StyledRun::new("cell", font)];
        let cell_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&cell_run, f64::MAX)))];
        let cells = [TableCell::new(&cell_nodes)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Fraction(1.0)];
        let table = TableBlock::new(&columns, &rows);
        let flow = [BlockNode::new(Block::Table(table))];

        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        let placed = pages[0].frame.blocks.first().expect("table must be placed in column 1");
        let table_width: f64 = placed.table_placement.as_ref().expect("table placement present").column_widths.iter().sum();

        assert!(
            (table_width - column_width).abs() < 1e-6,
            "a Fraction(1.0) column inside a table must fill the CONTAINING column's width ({column_width}), got {table_width}"
        );
        assert!(table_width < master.body_rect().width - 1.0, "the table must be narrower than the full (un-columned) body width");
    }

    /// Typography-gap WAVE 3: `HeaderPlaceholder::CurrentOutlineTitle`
    /// resolves per page to the LATEST `.with_outline()`-tagged heading on
    /// or before that page — empty before the first heading appears,
    /// updates on the SAME page a new heading lands on, and carries
    /// forward unchanged across every page without a heading of its own.
    #[test]
    fn running_header_resolves_the_current_outline_title_per_page() {
        use crate::master::HeaderPlaceholder;

        let font = body_font();
        let shaper = CosmicShaper::headless();

        let filler_run = [StyledRun::new(SENTENCE, font)];
        let heading_a_run = [StyledRun::new("Chapter One", font)];
        let heading_b_run = [StyledRun::new("Chapter Two", font)];

        let mut flow: Vec<BlockNode<'_>> = Vec::new();
        for _ in 0..6 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_run, 400.0))));
        }
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&heading_a_run, 400.0))).with_outline(1, "Chapter One"));
        for _ in 0..6 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_run, 400.0))));
        }
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&heading_b_run, 400.0))).with_outline(1, "Chapter Two"));
        for _ in 0..6 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_run, 400.0))));
        }

        let placeholder_run = [StyledRun::new("placeholder", font)];
        let header_flow =
            [BlockNode::new(Block::Paragraph(Paragraph::new(&placeholder_run, 400.0))).with_header_placeholder(HeaderPlaceholder::CurrentOutlineTitle)];

        let master = PageMaster::new(500.0, 250.0, Margins::uniform(30.0)).with_header(&header_flow);
        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert!(pages.len() >= 4, "fixture must be tuned to span several pages, got {}", pages.len());

        fn header_text(page: &Page<'_>) -> String {
            page.header
                .as_ref()
                .map(|h| h.blocks.iter().filter_map(|b| b.paragraph_layout.as_ref()).flat_map(|l| l.glyphs.iter().map(|g| g.cluster.clone())).collect::<String>())
                .unwrap_or_default()
        }

        let idx_a = pages.iter().position(|p| p.outline.iter().any(|e| e.title == "Chapter One")).expect("Chapter One must land on some page");
        let idx_b = pages.iter().position(|p| p.outline.iter().any(|e| e.title == "Chapter Two")).expect("Chapter Two must land on some page");
        assert!(idx_a < idx_b, "fixture must be tuned so Chapter One precedes Chapter Two");

        for (i, page) in pages.iter().enumerate() {
            let text = header_text(page);
            if i < idx_a {
                assert!(text.is_empty(), "page {i} (before any heading) must have an EMPTY running header, got {text:?}");
            } else if i < idx_b {
                assert!(text.contains("Chapter One"), "page {i} must show the running title \"Chapter One\", got {text:?}");
            } else {
                assert!(text.contains("Chapter Two"), "page {i} must show the running title \"Chapter Two\", got {text:?}");
            }
        }
    }

    /// A header/footer with NO placeholder-tagged node must keep the
    /// ORIGINAL "compose once, shared verbatim" behavior byte-identical —
    /// this is the regression floor `running_header_resolves_the_current_
    /// outline_title_per_page` above depends on staying additive.
    #[test]
    fn a_plain_header_with_no_placeholder_is_unaffected_by_the_running_header_machinery() {
        let font = body_font();
        let shaper = CosmicShaper::headless();
        let header_run = [StyledRun::new("Static Header", font)];
        let header_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, 400.0)))];
        let runs = [StyledRun::new("A single short paragraph.", font)];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 400.0)))];
        let master = PageMaster::new(500.0, 300.0, Margins::uniform(30.0)).with_header(&header_flow);
        let style = ComposeStyle::new(0.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].header.as_ref().is_some_and(|h| !h.blocks.is_empty()));
    }

    /// Typography-gap WAVE 3: a `Block::Paragraph` carrying one footnote
    /// marker (a superscript `"1"` run + a zero-size correlation
    /// `InlineBox`) produces a real, numbered entry in the reserved
    /// footnote zone — the zone's own rect matches
    /// `PageMaster::footnote_zone_rect` exactly, and the marker text is
    /// `"1."` (`MarkerStyle::numbered`'s own trailing-dot convention).
    #[test]
    fn a_footnote_marker_produces_a_numbered_entry_in_the_reserved_footnote_zone() {
        use uzor_text::{InlineBox, InlineBoxSlot, VerticalAlign};

        use crate::scene::Footnote;

        let font = body_font();
        let shaper = CosmicShaper::headless();

        let footnote_body_run = [StyledRun::new("This is the footnote body text.", font)];
        let footnote_content = [BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_body_run, 300.0)))];
        let footnotes = [Footnote::new(&footnote_content)];

        let body_text = "Body text with a marker here";
        let marker_text = "1";
        let body_run = StyledRun::new(body_text, font);
        let marker_run = StyledRun::new(marker_text, font).with_vertical_align(VerticalAlign::Super);
        let runs = [body_run, marker_run];
        let marker_box = InlineBox::out_of_flow(0); // local index 0 -> footnotes[0]
        let slots = [InlineBoxSlot::new(1, marker_text.len(), marker_box)];
        let paragraph = Paragraph::new(&runs, 400.0).with_inline_boxes(&slots);
        let node = BlockNode::new(Block::Paragraph(paragraph)).with_footnotes(&footnotes);
        let flow = [node];

        let master = PageMaster::new(500.0, 400.0, Margins::uniform(30.0)).with_footnote_zone(100.0);
        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let fp = pages[0].footnotes.as_ref().expect("footnote zone must be populated");
        assert!(!fp.clamped, "a short footnote inside a generous zone must never report clamped");
        assert_eq!(fp.frame.region.rect, master.footnote_zone_rect(), "the footnote frame's own region must be the master's own reserved zone rect exactly");

        let list_placement = fp.frame.blocks.first().and_then(|b| b.list_placement.as_ref()).expect("footnote zone content is a real Block::List placement");
        assert_eq!(list_placement.items.len(), 1);
        assert_eq!(list_placement.items[0].marker_text, "1.");
        let body_text_joined: String = list_placement.items[0]
            .content
            .iter()
            .filter_map(|b| b.paragraph_layout.as_ref())
            .flat_map(|l| l.glyphs.iter().map(|g| g.cluster.clone()))
            .collect();
        assert!(body_text_joined.contains("footnote body"), "the footnote's own authored content must render inside the zone, got {body_text_joined:?}");
    }

    /// A `PageMaster` with NO `footnote_zone_height` configured must never
    /// attach a `Page::footnotes` — additive, zero cost when unused (same
    /// convention every other WAVE-3 addition uses).
    #[test]
    fn no_footnote_zone_configured_means_page_footnotes_is_always_none() {
        use uzor_text::{InlineBox, InlineBoxSlot, VerticalAlign};

        use crate::scene::Footnote;

        let font = body_font();
        let shaper = CosmicShaper::headless();
        let footnote_body_run = [StyledRun::new("body", font)];
        let footnote_content = [BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_body_run, 300.0)))];
        let footnotes = [Footnote::new(&footnote_content)];
        let body_run = StyledRun::new("text", font);
        let marker_run = StyledRun::new("1", font).with_vertical_align(VerticalAlign::Super);
        let runs = [body_run, marker_run];
        let slots = [InlineBoxSlot::new(1, "1".len(), InlineBox::out_of_flow(0))];
        let paragraph = Paragraph::new(&runs, 400.0).with_inline_boxes(&slots);
        let flow = [BlockNode::new(Block::Paragraph(paragraph)).with_footnotes(&footnotes)];

        let master = PageMaster::new(500.0, 400.0, Margins::uniform(30.0)); // no with_footnote_zone
        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert!(pages[0].footnotes.is_none());
    }

    /// Typography-gap WAVE 3 (task gate): a paragraph whose own lines
    /// split across a page boundary carries its footnote markers with
    /// whichever FRAGMENT they actually landed on — an early marker's
    /// footnote appears on page 1, a late marker's footnote (same source
    /// paragraph) appears on page 2, and the GLOBAL numbering continues
    /// across the split (`"1."` then `"2."`, never restarting).
    #[test]
    fn a_footnote_follows_its_own_markers_paragraph_fragment_across_a_page_split() {
        use uzor_text::{InlineBox, InlineBoxSlot};

        use crate::scene::Footnote;

        let font = body_font();
        let shaper = CosmicShaper::headless();

        let footnote_a_run = [StyledRun::new("Early footnote body.", font)];
        let footnote_a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_a_run, 300.0)))];
        let footnote_b_run = [StyledRun::new("Late footnote body.", font)];
        let footnote_b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_b_run, 300.0)))];
        let footnotes = [Footnote::new(&footnote_a_nodes), Footnote::new(&footnote_b_nodes)];

        const TEXT: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron";
        const NARROW_WIDTH: f64 = 120.0;
        const MARKER_A_OFFSET: usize = 16; // right after "alpha beta gamma"
        let body_run = StyledRun::new(TEXT, font);
        let marker_a = InlineBox::out_of_flow(0);
        let marker_b = InlineBox::out_of_flow(1);
        let slots = [InlineBoxSlot::new(0, MARKER_A_OFFSET, marker_a), InlineBoxSlot::new(0, TEXT.len(), marker_b)];
        let runs = [body_run];
        let paragraph_template = Paragraph::new(&runs, NARROW_WIDTH).with_inline_boxes(&slots);

        let full = layout_paragraph(&paragraph_template, &shaper);
        assert!(full.lines.len() >= 6, "fixture must wrap to several lines, got {}", full.lines.len());
        assert_eq!(full.boxes.len(), 2, "both markers must be placed");
        let box_a_line = full.boxes.iter().find(|b| b.id == 0).expect("marker A placed").line_index;
        let box_b_line = full.boxes.iter().find(|b| b.id == 1).expect("marker B placed").line_index;
        assert!(box_a_line < box_b_line, "fixture must place the two markers on DIFFERENT lines, marker A earlier");

        let split_line = box_a_line + 1;
        assert!(split_line <= box_b_line, "fixture must be tuned so marker B's own line lands AFTER the split point");
        // The SAME body height repeats on every page (an infinite
        // `PageRegionSequence`) — pick a height that fits BOTH halves
        // (head lines `[0, split_line)` AND tail lines `[split_line, end)`)
        // so the split lands at exactly `split_line` on page 1 and the
        // WHOLE remainder finishes on page 2, never spilling to a 3rd.
        let head_height: f64 = full.lines[..split_line].iter().map(|l| l.height).sum();
        let tail_height: f64 = full.lines[split_line..].iter().map(|l| l.height).sum();
        let region_height = head_height.max(tail_height) + 0.5;

        let node = BlockNode::new(Block::Paragraph(paragraph_template)).with_footnotes(&footnotes);
        let flow = [node];

        let margins = Margins::uniform(20.0);
        const FOOTNOTE_ZONE: f64 = 80.0;
        let page_height = region_height + margins.top + margins.bottom + FOOTNOTE_ZONE;
        let master = PageMaster::new(NARROW_WIDTH + margins.left + margins.right, page_height, margins).with_footnote_zone(FOOTNOTE_ZONE);
        let style = ComposeStyle::new(0.0, font).with_min_orphan_lines(0).with_min_widow_lines(0);

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture must be tuned to split across exactly 2 pages, got {}", pages.len());

        fn marker_texts(fp: &FootnotePlacement<'_>) -> Vec<String> {
            fp.frame.blocks.first().and_then(|b| b.list_placement.as_ref()).map(|lp| lp.items.iter().map(|i| i.marker_text.clone()).collect()).unwrap_or_default()
        }

        let page1_fn = pages[0].footnotes.as_ref().expect("page 1 must carry the early marker's footnote");
        let page2_fn = pages[1].footnotes.as_ref().expect("page 2 must carry the late marker's footnote");
        assert_eq!(marker_texts(page1_fn), vec!["1."], "page 1's footnote zone must hold ONLY the early marker's own footnote");
        assert_eq!(marker_texts(page2_fn), vec!["2."], "page 2's footnote zone must hold the late marker's footnote, continuing the GLOBAL numbering");
    }

    /// Typography-gap WAVE 3 (task gate — "no multi-page footnote
    /// splitting"): a footnote whose own body content is taller than the
    /// ENTIRE reserved zone must report `clamped: true` (never silently
    /// fit, never spill onto a second page — this crate has no
    /// `slice_pages` call here that could even represent "the next page's
    /// zone continues this footnote") while still being VISIBLY placed
    /// (the SAME "overflow returned as data, content never silently
    /// dropped" convention this crate's own P0 risk note already
    /// establishes elsewhere).
    #[test]
    fn a_footnote_taller_than_its_own_zone_is_clamped_never_spilling_to_a_second_page() {
        use uzor_text::{InlineBox, InlineBoxSlot, VerticalAlign};

        use crate::scene::Footnote;

        let font = body_font();
        let shaper = CosmicShaper::headless();

        let long_run = [StyledRun::new(SENTENCE, font)];
        let footnote_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&long_run, 300.0)))];
        let footnotes = [Footnote::new(&footnote_nodes)];

        let body_run = StyledRun::new("Body text.", font);
        let marker_run = StyledRun::new("1", font).with_vertical_align(VerticalAlign::Super);
        let runs = [body_run, marker_run];
        let slots = [InlineBoxSlot::new(1, "1".len(), InlineBox::out_of_flow(0))];
        let paragraph = Paragraph::new(&runs, 300.0).with_inline_boxes(&slots);
        let node = BlockNode::new(Block::Paragraph(paragraph)).with_footnotes(&footnotes);
        let flow = [node];

        const TINY_ZONE: f64 = 20.0; // room for roughly one short line only
        let master = PageMaster::new(400.0, 300.0, Margins::uniform(20.0)).with_footnote_zone(TINY_ZONE);
        let style = ComposeStyle::new(0.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "the body content itself is short — only the footnote zone is deliberately tight");

        let fp = pages[0].footnotes.as_ref().expect("footnote zone must still be populated");
        assert!(fp.clamped, "a footnote taller than its own reserved zone must report clamped, never silently fit");
        assert!(!fp.frame.blocks.is_empty(), "the overflowing footnote must still be VISIBLY placed, never dropped");
    }

    /// This document's own absolute first-line baseline for every REAL
    /// (non-empty) placed `Block::Paragraph`, across every page and both
    /// `frame`/`extra_frames` — the T2 proof's own shared measurement.
    fn collect_paragraph_baselines(pages: &[Page<'_>]) -> Vec<f64> {
        pages
            .iter()
            .flat_map(|p| p.frame.blocks.iter().chain(p.extra_frames.iter().flat_map(|f| f.blocks.iter())))
            .filter_map(|b| {
                let layout = b.paragraph_layout.as_ref()?;
                let line = layout.lines.first()?;
                Some(b.rect.y + line.baseline_y)
            })
            .collect()
    }

    /// Typography track T2 (baseline grid) — the headline proof: a
    /// 2-column master, a heading, several body paragraphs, a
    /// `Block::Figure` with a deliberately ODD height (`133.0`, not a
    /// multiple of the fixture's own `PITCH`), then more body paragraphs
    /// after it — enough content that it spills from column 1 into
    /// column 2. With the grid ON (`PITCH` deliberately unrelated to any
    /// fixture font's own natural leading, so alignment can only come
    /// from real snapping, never coincidence), EVERY placed paragraph's
    /// own first-line baseline — in BOTH columns, before AND after the
    /// odd-height figure — lands on the SAME shared absolute grid line
    /// set (`(baseline_y - grid_origin) mod PITCH ≈ 0`): the literal
    /// "adjacent columns share a line grid" claim this track exists to
    /// prove, plus proof that a non-text block's own height-rounding
    /// keeps later text on-grid too. With the grid OFF (the default, no
    /// `.with_baseline_grid` call), composing the IDENTICAL fixture
    /// reproduces byte-identical geometry to a second, fully independent
    /// `ComposeStyle::new` run of the SAME flow — this track's own
    /// "never change rendered output silently" gate, exercised directly
    /// (not merely inferred from the rest of this crate's unmodified
    /// suite staying green).
    #[test]
    fn two_column_master_shares_one_baseline_grid_across_both_columns_when_enabled() {
        use crate::scene::{Block, BlockSizing, FigureBlock, TypesetFigure};
        use uzor::render::RenderContext;
        use uzor_figures::FigureTheme;

        struct StubFigure;
        impl TypesetFigure for StubFigure {
            fn render(&self, _ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {}
        }

        const PITCH: f64 = 18.0; // deliberately not a multiple of the fixture fonts' own line heights
        const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 21.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };
        const FIGURE_HEIGHT: f64 = 133.0; // deliberately NOT a multiple of PITCH

        let master = PageMaster::new(620.0, 310.0, Margins::uniform(30.0)).with_columns(2, 20.0);
        let column_width = master.column_width();

        let heading_run = [StyledRun::new("Grid Section", HEADING_FONT)];
        // Deliberately SHORT, single-line body paragraphs — this proof
        // targets the documented guarantee ("a FRESH block's own first
        // line snaps"), not a mid-paragraph continuation fragment's own
        // un-snapped later lines (a separate, already-documented
        // limitation — see `compose::baseline_grid`'s own module doc,
        // "typically the body leading" note). Enough of them still
        // overflow this fixture's own deliberately short column.
        let body_texts: Vec<String> = (0..16).map(|i| format!("Row {i}.")).collect();
        let body_runs: Vec<[StyledRun<'_>; 1]> = body_texts.iter().map(|t| [StyledRun::new(t.as_str(), BODY_FONT)]).collect();
        let stub = StubFigure;

        let mut flow: Vec<BlockNode<'_>> = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&heading_run, column_width))),
            BlockNode::new(Block::Spacer(10.0)),
        ];
        for run in &body_runs[..6] {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(run, column_width))));
        }
        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&stub, BlockSizing::FixedHeight(FIGURE_HEIGHT)))));
        for run in &body_runs[6..] {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(run, column_width))));
        }

        let shaper = CosmicShaper::headless();
        let grid_origin = master.body_rect().y;

        // OFF (the default) — byte-identical geometry to an independent
        // second compose of the SAME flow: this track changes NOTHING on
        // the disabled path.
        let style_off_a = ComposeStyle::new(8.0, BODY_FONT);
        let style_off_b = ComposeStyle::new(8.0, BODY_FONT);
        let pages_off_a = slice_pages(&flow, &master, &style_off_a, &shaper);
        let pages_off_b = slice_pages(&flow, &master, &style_off_b, &shaper);
        assert_eq!(pages_off_a.len(), pages_off_b.len());
        for (a, b) in pages_off_a.iter().zip(pages_off_b.iter()) {
            let a_rects: Vec<Rect> = a.frame.blocks.iter().chain(a.extra_frames.iter().flat_map(|f| f.blocks.iter())).map(|p| p.rect).collect();
            let b_rects: Vec<Rect> = b.frame.blocks.iter().chain(b.extra_frames.iter().flat_map(|f| f.blocks.iter())).map(|p| p.rect).collect();
            assert_eq!(a_rects, b_rects, "grid OFF (the default) must reproduce byte-identical geometry — this track changes nothing on the disabled path");
        }

        // Sanity: the UN-gridded layout must NOT already coincidentally
        // land every baseline on `PITCH`'s own grid — otherwise the ON
        // assertion below would prove nothing.
        let off_baselines = collect_paragraph_baselines(&pages_off_a);
        assert!(
            off_baselines.iter().any(|y| {
                let remainder = (y - grid_origin).rem_euclid(PITCH);
                remainder.min(PITCH - remainder) > 1.0
            }),
            "fixture must be tuned so the UN-gridded layout does NOT already coincidentally land every baseline on the grid"
        );

        // ON — every placed paragraph's own first-line baseline, in
        // EITHER column, before AND after the odd-height figure, lands
        // on the shared grid.
        let style_on = ComposeStyle::new(8.0, BODY_FONT).with_baseline_grid(PITCH);
        let pages_on = slice_pages(&flow, &master, &style_on, &shaper);
        assert!(pages_on.iter().any(|p| !p.extra_frames.is_empty()), "fixture must be tuned so content actually spills into column 2");

        let on_baselines = collect_paragraph_baselines(&pages_on);
        assert!(on_baselines.len() >= 10, "fixture must place enough paragraphs to meaningfully exercise the grid, got {}", on_baselines.len());
        for y in &on_baselines {
            let local = y - grid_origin;
            let remainder = local.rem_euclid(PITCH);
            let distance_to_grid = remainder.min(PITCH - remainder);
            assert!(distance_to_grid < 1e-3, "baseline at absolute y={y} (local={local}) must land on a shared grid multiple of {PITCH}, off by {distance_to_grid}");
        }
    }
}
