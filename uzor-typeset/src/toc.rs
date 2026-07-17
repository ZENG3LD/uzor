//! Table-of-contents generation — document-navigation feature pass
//! (`nemo/docs/uzor-engines/research_typesetting_sota_2026.md`'s
//! counter-introspection item, §1/top-10 #9: "counter introspection as
//! multi-pass fixpoint over a location index ... powers counters/TOC/
//! cross-refs uniformly"). This module owns exactly the TOC half of that
//! item: [`OutlineEntry`](crate::slice::OutlineEntry) collection already
//! lives in `slice::pages` (the "introspection index" — where did every
//! `.with_outline()`-tagged block actually land); this module turns a
//! resolved `&[OutlineEntry]` into real, composable rows
//! ([`build_toc`]/[`build_toc_rows`]) and drives the standard 2-pass
//! fixpoint that a TOC's own page-shifting side effect requires
//! ([`compose_document_with_toc`]).
//!
//! ## The leader-row mechanism (the one new layout primitive)
//!
//! Every other part of a TOC row is ordinary: a title, a page number, an
//! indent. The one genuinely new mechanism is [`dotted_leader`] — a pure
//! function that measures a title and a page-number string (both already-
//! resolved widths, via the SAME `uzor_text::paragraph_intrinsic_size`
//! measure path every other block in this crate uses) and fills the
//! remaining horizontal gap with as many repeats of a leader character
//! (`'.'` by default) as REALLY measure to fit — not a linear estimate
//! alone, since proportional-font kerning between repeated glyphs is rare
//! but not guaranteed absent (see [`dotted_leader`]'s own doc comment).
//! [`build_toc_rows`] is the row-level caller of this mechanism: title
//! left, leader filling the middle, page number at the row's own right
//! edge — a title long enough to leave no room for a leader at all
//! degrades the leader to `""` (never truncating the page number, which
//! always stays intact and right-aligned).
//!
//! ## Indentation — a leading [`InlineBox`], never leading whitespace
//!
//! A naive `"    Title"` (literal leading space characters) does NOT
//! work in this crate: `uzor_text::layout::greedy::pack_lines` NEVER
//! starts a line with a glue (whitespace) atom — `if current.is_empty()
//! { continue; }` in `pack_lines` drops a LEADING whitespace atom
//! unconditionally, including at the very START of a paragraph (verified
//! by direct inspection of that function; its own test,
//! `pack_lines_never_starts_a_line_with_leading_whitespace`, proves the
//! same rule for every wrapped line, and the FIRST line is no exception —
//! `current` starts empty there too). A per-level indent is instead
//! reserved via a leading [`InlineBox::in_flow`] spacer (an ATOM kind
//! `pack_lines` never trims, unlike a glue atom) — a real, already-built
//! crate mechanism (parley's "reserved rectangle spliced into a
//! paragraph's run sequence"), not a new one.
//!
//! ## Why row text needs owned, `'static` backing (report — a genuine
//! constraint, not an oversight)
//!
//! Every `BlockNode<'a>`/`Paragraph<'a>` in this crate is BORROWED by
//! design (design law 3: stateless layout over borrowed snapshots,
//! mirroring `uzor_text::Paragraph<'a>`'s own convention) — every
//! existing fixture in this crate (tests, the showcase) authors its
//! runs/blocks as literal `&'static str`/caller-owned `String`s already
//! alive in the SAME scope `compose()`/`slice_pages()` is called from.
//! A TOC row's own text (title + a MEASURED leader + a resolved page
//! number) is instead SYNTHESIZED at compose time — there is no caller
//! scope that could have authored it ahead of time, because the numbers
//! it prints depend on a prior composition pass. Bundling "owns a
//! `String`" and "borrows that `String`" into ONE self-contained return
//! value is the classic Rust self-referential-struct limitation (no
//! `unsafe`/pinning crate is used anywhere in this workspace, so that
//! route is out) — [`build_toc`] resolves this the same way the standard
//! library itself offers for exactly this case: `Vec::leak`/`Box::leak`
//! (safe, `std`-sanctioned, NOT `unsafe` code) turns each row's owned
//! `String`/backing arrays into `'static` data, which trivially unifies
//! with ANY caller lifetime `'a` via reference covariance. This is a
//! **bounded, per-call leak** (total size proportional to one document's
//! own TOC entry count — typically a few hundred bytes to a few KB) —
//! acceptable for a document-generation library whose `slice_pages`/
//! `pages_to_pdf` entry points are called a bounded number of times per
//! process (the CLI/batch/report-export usage this crate targets), NOT
//! suitable for an unbounded hot loop generating unboundedly many
//! documents inside one long-running process that never restarts.

use uzor::fonts::FontFamily;
use uzor_text::{paragraph_intrinsic_size, FontSpec, InlineBox, InlineBoxSlot, LineShaper, Paragraph, StyledRun};

use crate::compose::ComposeStyle;
use crate::master::{PageMaster, PageNumberStyle};
use crate::scene::{Block, BlockNode};
use crate::slice::{renumber_pages, slice_pages, OutlineEntry, Page};

/// Style knobs for [`build_toc`]/[`build_toc_rows`]'s generated rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TocStyle {
    /// Font every row's title/leader/page-number paints with — kept
    /// deliberately uniform across an entire TOC (this module's own v1
    /// scope choice, report): differentiating font/weight per outline
    /// LEVEL is a real, reasonable future extension, but no gate/consumer
    /// this pass needs one — indentation alone (this style's own
    /// `level_indent_px`) is what the task's own brief asks a level to
    /// visually communicate.
    pub font: FontSpec,
    /// The leader character repeated between title and page number.
    pub dot_char: char,
    /// Horizontal indent per outline level beyond level 1 (level `1` gets
    /// zero indent; level `2` gets one step; etc. — 1-based, matching
    /// heading-level convention).
    pub level_indent_px: f64,
    /// Vertical gap between consecutive TOC rows — read by
    /// [`compose_document_with_toc`] (via its own `ComposeStyle` for the
    /// TOC's dedicated `slice_pages` pass), not baked into any one row's
    /// own block.
    pub row_gap_px: f64,
}

impl Default for TocStyle {
    fn default() -> Self {
        Self { font: FontSpec::new(FontFamily::Roboto, 13.0), dot_char: '.', level_indent_px: 16.0, row_gap_px: 6.0 }
    }
}

/// One fully-resolved TOC row — pure, OWNED data (no borrowed `BlockNode`
/// yet; see this module's own "Why row text needs owned, `'static`
/// backing" doc comment for why turning this into a real flow block is a
/// SEPARATE step, [`build_toc`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TocRow {
    pub level: u8,
    /// The row's own single-line text: title, a measured dotted leader,
    /// and the resolved page number, already concatenated — this crate's
    /// own row-construction choice (a single run, single font — see
    /// [`TocStyle::font`]'s own doc comment).
    pub text: String,
    /// This row's own left indent, in px — apply via a leading
    /// [`InlineBox`] spacer (this module's own doc comment explains why,
    /// not literal leading whitespace).
    pub indent_px: f64,
}

/// Small, fixed clearance on EITHER side of the dotted-leader run, so the
/// dots never visually touch the title/number glyphs directly.
const LEADER_PAD_PX: f64 = 4.0;

/// Fill `available_width` with as many repeats of `dot_char` as REALLY
/// measure to fit (via `shaper`/`font`) — not a linear single-glyph-width
/// estimate alone: a repeated-character run's shaped width isn't
/// guaranteed perfectly linear (rare inter-glyph kerning in proportional
/// fonts), so this walks a real, bounded, decreasing search from a
/// generous starting estimate down to the first count whose ACTUAL
/// measured width fits. Returns `""` when `available_width <= 0.0` or
/// even a single dot's own width doesn't fit — a row's leader shrinking
/// to nothing is a normal, non-error degrade (see [`build_toc_row_text`]).
pub fn dotted_leader(available_width: f64, dot_char: char, font: FontSpec, shaper: &dyn LineShaper) -> String {
    if available_width <= 0.0 {
        return String::new();
    }
    let mut dot_buf = [0u8; 4];
    let single_dot: &str = dot_char.encode_utf8(&mut dot_buf);
    let single_width = paragraph_intrinsic_size(single_dot, &font, f64::MAX, shaper).0;
    if single_width <= 0.0 || single_width > available_width {
        return String::new();
    }

    // Generous starting guess (never an UNDER-estimate, so the search
    // below only ever needs to walk DOWN, never up) — bounded: at most
    // `count + 1` measurements, `count` itself bounded by
    // `available_width` (a real, finite row width every caller passes).
    let mut count = (available_width / single_width).floor() as i64 + 2;
    while count > 0 {
        let candidate: String = std::iter::repeat(dot_char).take(count as usize).collect();
        let measured = paragraph_intrinsic_size(&candidate, &font, f64::MAX, shaper).0;
        if measured <= available_width {
            return candidate;
        }
        count -= 1;
    }
    String::new()
}

/// Build one row's own single-line text: `title`, a [`dotted_leader`]
/// filling the gap, then `number_text` — the leader-row math [`build_toc_rows`]
/// delegates to, a pure function of already-measured widths (the "two-part
/// row: measure title + number, fill between with '.' repeats" mechanism).
/// `available_width` is the row's OWN content width (already excluding
/// this row's own indent — see [`build_toc_rows`]). A title wide enough to
/// leave no room for the leader (or even the leader's own surrounding pad)
/// degrades to `"{title} {number_text}"` — the page number is NEVER
/// truncated or dropped, only the leader shrinks, down to nothing.
fn build_toc_row_text(title: &str, number_text: &str, available_width: f64, style: &TocStyle, shaper: &dyn LineShaper) -> String {
    let title_width = paragraph_intrinsic_size(title, &style.font, f64::MAX, shaper).0;
    let number_width = paragraph_intrinsic_size(number_text, &style.font, f64::MAX, shaper).0;
    let leader_budget = available_width - title_width - number_width - 2.0 * LEADER_PAD_PX;
    let leader = dotted_leader(leader_budget, style.dot_char, style.font, shaper);
    if leader.is_empty() {
        format!("{title} {number_text}")
    } else {
        format!("{title} {leader} {number_text}")
    }
}

/// Turn resolved [`OutlineEntry`]s into pure, owned [`TocRow`]s — the
/// "measure + fill" half of TOC generation, directly tested (see this
/// module's own tests). `row_width` is the FULL width a TOC row occupies
/// (before any per-row indent is subtracted) — typically a TOC-dedicated
/// [`PageMaster::body_rect`]'s own width.
pub fn build_toc_rows(entries: &[OutlineEntry], style: &TocStyle, row_width: f64, shaper: &dyn LineShaper) -> Vec<TocRow> {
    entries
        .iter()
        .map(|entry| {
            let indent_px = style.level_indent_px * entry.level.saturating_sub(1) as f64;
            let available_width = (row_width - indent_px).max(0.0);
            let number_text = (entry.page_index + 1).to_string();
            let text = build_toc_row_text(&entry.title, &number_text, available_width, style, shaper);
            TocRow { level: entry.level, text, indent_px }
        })
        .collect()
}

/// Leak `s` into a `'static` `&str` — see this module's own "Why row text
/// needs owned, `'static` backing" doc comment for the full rationale.
/// `std`-sanctioned (`Box::leak`), not `unsafe` code — a documented,
/// bounded, per-[`build_toc`]-call trade-off.
fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Turn `entries` into real, composable `Block::Paragraph` flow nodes —
/// one per row, title left + dotted leader + page number all on one line
/// (via [`build_toc_rows`]), indented per level via a leading [`InlineBox`]
/// spacer (never literal leading whitespace — this module's own doc
/// comment). Rows are ordinary blocks, composed/sliced exactly like any
/// other paragraph in this crate (`compose()`/`slice_pages()` — real
/// pagination, real vector text once exported via `pages_to_pdf`) — the
/// returned `'static` lifetime is this function's own documented,
/// bounded-leak trade-off (see this module's own top doc comment),
/// invisible to the caller beyond the lifetime annotation itself: a
/// `Vec<BlockNode<'static>>` composes into `Page<'static>`, which is a
/// valid `Page<'a>` for ANY `'a` via reference covariance — free to
/// concatenate with an ordinary borrowed body flow's own `Page<'a>`s.
/// See this function's own doc comment above for the leak rationale.
/// Each row's own `BlockNode` is ALSO tagged via
/// [`BlockNode::with_link_target`] against its own entry's already-
/// resolved `page_index` — a TOC row is therefore a real internal-link
/// (GoTo) source once composed via [`crate::slice::slice_pages`], no
/// separate wiring needed at the PDF-export layer beyond reading
/// [`crate::slice::Page::links`] (which `uzor-typeset::export::pdf_adapter`
/// already does).
pub fn build_toc(entries: &[OutlineEntry], style: &TocStyle, row_width: f64, shaper: &dyn LineShaper) -> Vec<BlockNode<'static>> {
    build_toc_rows(entries, style, row_width, shaper)
        .into_iter()
        .zip(entries.iter())
        .map(|(row, entry)| {
            let text: &'static str = leak_str(row.text);
            let runs: &'static [StyledRun<'static>] = &*vec![StyledRun::new(text, style.font)].leak();
            let boxes: &'static [InlineBoxSlot] = &*vec![InlineBoxSlot::new(0, 0, InlineBox::in_flow(0, row.indent_px, 1.0))].leak();
            let paragraph = Paragraph::new(runs, row_width).with_inline_boxes(boxes);
            BlockNode::new(Block::Paragraph(paragraph)).with_link_target(entry.page_index)
        })
        .collect()
}

/// Read every `.with_outline()`-tagged entry already attached to `pages`
/// (via [`crate::slice::slice_pages`]'s own collection pass), shifting
/// each `page_index` by `page_offset` (the number of pages a TOC section
/// is ASSUMED to occupy BEFORE `pages`, in [`compose_document_with_toc`]'s
/// own fixpoint loop).
fn collect_shifted_entries(pages: &[Page<'_>], page_offset: u32) -> Vec<OutlineEntry> {
    let mut out = Vec::new();
    for page in pages {
        for entry in &page.outline {
            out.push(OutlineEntry { level: entry.level, title: entry.title.clone(), page_index: entry.page_index + page_offset });
        }
    }
    out
}

/// Bounded fixpoint iteration cap — see this function's own "why 3 passes
/// is provably enough" note below.
const MAX_TOC_FIXPOINT_ITERATIONS: usize = 3;

/// Compose an ALREADY-sliced `body_pages` together with a generated table
/// of contents — the standard 2-pass counter-introspection fixpoint
/// (design doc: "counter introspection ... multi-pass fixpoint over a
/// location index").
///
/// `body_pages` is taken pre-composed (rather than a `(flow, master)`
/// pair this crate's other `slice_*` entry points take) so this function
/// works uniformly whether the body is ONE homogeneous `slice_pages` call
/// or — the more common real-document shape — several independently-
/// sliced sections already concatenated (e.g. a single-column opener +
/// a 2-column section + more single-column pages, exactly the showcase's
/// own "master/columns seam" pattern, `export::showcase`'s own doc
/// comment) — the TOC-insertion fixpoint below only ever needs to COUNT
/// pages and read each one's own already-attached [`Page::outline`],
/// never re-derive how `body_pages` itself was composed.
///
/// **Pass 1** builds a CANDIDATE TOC assuming it occupies ZERO pages
/// (`page_offset = 0`) purely to learn the candidate TOC's OWN page
/// count. **Pass 2** rebuilds the TOC using THAT page count as the offset
/// (the TOC's real row NUMBERS, now correctly shifted past however many
/// pages the TOC itself will occupy) — if the offset changes again, the
/// loop repeats (bounded at [`MAX_TOC_FIXPOINT_ITERATIONS`]); once the
/// TOC's own rebuilt page count stops changing, the TOC pages are
/// concatenated BEFORE `body_pages` and the whole document is renumbered
/// via [`crate::slice::renumber_pages`] (using `page_number_style`, if
/// any, as the canonical numbering style for the concatenation).
///
/// ## Why this provably converges (assert-worthy, not just "usually")
///
/// `body_pages` is a fixed INPUT, never recomputed by this function — so
/// the entry LIST (title/level per row) is invariant across every
/// iteration; only each entry's own `page_index` (hence its printed
/// NUMBER TEXT) changes with the offset. [`build_toc_row_text`] ALWAYS
/// measures the number text's OWN actual width before deciding the
/// leader's dot count — a wider number (e.g. `"9"` -> `"10"`) simply
/// shrinks the leader by roughly one glyph's width, it does not grow the
/// row's own total (already `row_width`-bounded) footprint. A TOC row's
/// height therefore never depends on which specific digits its own page
/// number prints — only on whether the row count and `row_width` cause a
/// wrap, both of which are offset-independent. Hence `toc_pages.len()`
/// (a pure function of row count + row height + `toc_master`'s own page
/// geometry) is ALREADY invariant to the offset in every case this crate
/// can construct: the loop's second iteration (using the correct,
/// non-zero offset from iteration 1) reproduces the SAME
/// `toc_pages.len()`, converging in exactly 2 of the
/// [`MAX_TOC_FIXPOINT_ITERATIONS`] allotted iterations — the 3rd is a
/// defensive margin for the general multi-pass-fixpoint shape (research
/// doc's own port-algorithm item), never actually needed by this
/// function's own construction. Reaching the bound WITHOUT converging
/// would mean that invariant was broken — this function's own bug, not a
/// caller-input problem, hence the documented panic below rather than a
/// silent best-effort return.
pub fn compose_document_with_toc<'a>(
    body_pages: Vec<Page<'a>>,
    toc_master: &PageMaster<'a>,
    toc_style: &TocStyle,
    shaper: &dyn LineShaper,
    page_number_style: Option<&PageNumberStyle>,
) -> Vec<Page<'a>> {
    let toc_row_width = toc_master.body_rect().width;
    let toc_compose_style = ComposeStyle::new(toc_style.row_gap_px, toc_style.font);

    let mut toc_page_offset = 0u32;
    for _ in 0..MAX_TOC_FIXPOINT_ITERATIONS {
        let entries = collect_shifted_entries(&body_pages, toc_page_offset);
        // The TOC flow's own backing array is ALSO leaked here (on top of
        // `build_toc`'s own per-row leaks) — the Vec CONTAINER itself
        // must outlive this loop iteration for `slice_pages`'s returned
        // `Page<'static>` to be valid to return from this function (a
        // `&toc_flow` LOCAL-variable borrow would not). Bounded: at most
        // `MAX_TOC_FIXPOINT_ITERATIONS` such leaks per
        // `compose_document_with_toc` call, each proportional to this
        // document's own TOC entry count.
        let toc_flow: &'static [BlockNode<'static>] = build_toc(&entries, toc_style, toc_row_width, shaper).leak();
        let toc_pages = slice_pages(toc_flow, toc_master, &toc_compose_style, shaper);
        let new_offset = toc_pages.len() as u32;

        if new_offset == toc_page_offset {
            let mut all: Vec<Page<'a>> = Vec::with_capacity(toc_pages.len() + body_pages.len());
            all.extend(toc_pages);
            all.extend(body_pages);
            return renumber_pages(all, page_number_style);
        }
        toc_page_offset = new_offset;
    }

    panic!(
        "compose_document_with_toc: TOC page count failed to converge within {MAX_TOC_FIXPOINT_ITERATIONS} iterations. \
         This function's own row-builder always reserves space for the page-number text's OWN width before deciding \
         the dotted-leader's dot count, which makes a TOC row's height (hence `toc_pages.len()`) invariant to the \
         page-number offset in every case this crate can construct — reaching this panic means that invariant was \
         violated, a genuine bug in this function, not a caller-input problem."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_text::CosmicShaper;

    fn style() -> TocStyle {
        TocStyle::default()
    }

    #[test]
    fn dotted_leader_fills_the_available_width_as_tightly_as_possible_without_exceeding_it() {
        let shaper = CosmicShaper::headless();
        let font = style().font;
        let available = 120.0;

        let leader = dotted_leader(available, '.', font, &shaper);
        assert!(!leader.is_empty(), "a generous available width must produce a real leader run");

        let measured = paragraph_intrinsic_size(&leader, &font, f64::MAX, &shaper).0;
        assert!(measured <= available, "leader must never exceed its own available width, got {measured} > {available}");

        // Tightly packed: one MORE dot must no longer fit (proves this
        // isn't an arbitrarily short/lazy fill).
        let one_more: String = format!("{leader}.");
        let one_more_width = paragraph_intrinsic_size(&one_more, &font, f64::MAX, &shaper).0;
        assert!(one_more_width > available, "one additional dot must no longer fit — leader must be the tightest fit, not an underfill");
    }

    #[test]
    fn dotted_leader_is_empty_when_available_width_is_non_positive_or_smaller_than_one_dot() {
        let shaper = CosmicShaper::headless();
        let font = style().font;
        assert_eq!(dotted_leader(0.0, '.', font, &shaper), "");
        assert_eq!(dotted_leader(-5.0, '.', font, &shaper), "");
        assert_eq!(dotted_leader(0.001, '.', font, &shaper), "");
    }

    /// Leader-row math (task gate 1): title + dots + number fill the row
    /// width as tightly as possible without exceeding it, and the page
    /// number sits at the row's own right edge (verified directly: the
    /// row's own tail is EXACTLY the number text).
    #[test]
    fn a_normal_row_fills_the_row_width_tightly_with_the_number_at_the_right_edge() {
        let shaper = CosmicShaper::headless();
        let toc_style = style();
        const ROW_WIDTH: f64 = 300.0;

        let entries = [OutlineEntry { level: 1, title: "Introduction".to_owned(), page_index: 0 }];
        let rows = build_toc_rows(&entries, &toc_style, ROW_WIDTH, &shaper);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];

        assert!(row.text.ends_with('1'), "row text must end with the resolved page number, got {:?}", row.text);
        assert!(row.text.contains("..."), "a normal-width row must contain a real dot leader, got {:?}", row.text);

        let full_width = paragraph_intrinsic_size(&row.text, &toc_style.font, f64::MAX, &shaper).0;
        assert!(full_width <= ROW_WIDTH - row.indent_px + 1.0, "row must fit within its own available width, got {full_width}");
        assert!(full_width > ROW_WIDTH - row.indent_px - 40.0, "row must fill its own width tightly, not underfill by a wide margin, got {full_width}");
    }

    /// Leader-row math (task gate 1, "long title truncates dots not the
    /// number"): a title deliberately wide enough to leave no room for a
    /// leader must degrade the LEADER to nothing, never drop or truncate
    /// the page number.
    #[test]
    fn a_long_title_truncates_the_leader_never_the_page_number() {
        let shaper = CosmicShaper::headless();
        let toc_style = style();
        const ROW_WIDTH: f64 = 200.0;

        let long_title = "A deliberately very long section title that already consumes almost the entire available row width on its own";
        let entries = [OutlineEntry { level: 1, title: long_title.to_owned(), page_index: 41 }];
        let rows = build_toc_rows(&entries, &toc_style, ROW_WIDTH, &shaper);
        let row = &rows[0];

        assert!(!row.text.contains("..."), "an overflowing title must leave the leader empty, not a partial/degenerate dot run");
        assert!(row.text.ends_with("42"), "the resolved page number (42, 1-based) must always be present intact, got {:?}", row.text);
        assert!(row.text.starts_with(long_title), "the title must never itself be truncated by this function", );
    }

    /// Outline collection + a multi-run concatenation (task gate 3):
    /// two independently-sliced sections, each with its own
    /// `.with_outline()`-tagged headings, concatenated and renumbered —
    /// every resulting `OutlineEntry::page_index` must match the FINAL,
    /// global page position, never a stale per-section index.
    #[test]
    fn outline_levels_and_pages_are_correct_across_a_multi_run_concat_with_renumbering() {
        use uzor::fonts::FontFamily;
        use uzor_text::StyledRun;

        use crate::master::Margins;

        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 14.0);
        let style = ComposeStyle::new(4.0, font);
        let master = PageMaster::new(300.0, 200.0, Margins::uniform(20.0));
        let body_width = master.body_rect().width;

        // Section A: one page, one level-1 heading at its own top.
        let a_run = [StyledRun::new("Section A", font)];
        let a_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&a_run, body_width))).with_outline(1, "Section A")];
        let pages_a = slice_pages(&a_flow, &master, &style, &shaper);
        assert_eq!(pages_a.len(), 1);

        // Section B: filler tall enough to push its OWN level-1 heading
        // onto a SECOND page within this section alone, plus a level-2
        // sub-heading right after it.
        let filler_run = [StyledRun::new(
            "Filler text repeated to consume enough vertical space that the following heading \
            lands on this section's own second page rather than its first.",
            font,
        )];
        let b_heading_run = [StyledRun::new("Section B", font)];
        let b_sub_run = [StyledRun::new("Section B.1", font)];
        let mut b_flow_vec: Vec<BlockNode<'_>> = Vec::new();
        for _ in 0..10 {
            b_flow_vec.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_run, body_width))));
        }
        b_flow_vec.push(BlockNode::new(Block::Paragraph(Paragraph::new(&b_heading_run, body_width))).with_outline(1, "Section B"));
        b_flow_vec.push(BlockNode::new(Block::Paragraph(Paragraph::new(&b_sub_run, body_width))).with_outline(2, "Section B.1"));
        let pages_b = slice_pages(&b_flow_vec, &master, &style, &shaper);
        assert!(pages_b.len() >= 2, "fixture must be tuned so section B's own heading lands on ITS second page");

        let mut all: Vec<Page<'_>> = Vec::new();
        all.extend(pages_a);
        all.extend(pages_b);
        let all = renumber_pages(all, None);

        let mut collected: Vec<OutlineEntry> = Vec::new();
        for page in &all {
            collected.extend(page.outline.iter().cloned());
        }
        assert_eq!(collected.len(), 3, "exactly 3 tagged headings across both sections");

        assert_eq!(collected[0].title, "Section A");
        assert_eq!(collected[0].level, 1);
        assert_eq!(collected[0].page_index, 0, "section A's heading is on the FIRST global page");

        assert_eq!(collected[1].title, "Section B");
        assert_eq!(collected[1].level, 1);
        assert!(collected[1].page_index >= 2, "section B's heading must be on its own SECOND section page, which is GLOBAL page >= 2 (1 from section A + at least 1 filler page)");

        assert_eq!(collected[2].title, "Section B.1");
        assert_eq!(collected[2].level, 2);
        assert!(collected[2].page_index >= collected[1].page_index, "the sub-heading must land on the SAME or a LATER global page than its immediately preceding heading");
    }

    /// Fixpoint (task gate 2): a fixture where inserting the TOC
    /// genuinely shifts a heading's own GLOBAL page number (because the
    /// TOC itself spans >= 1 page before the body) must converge within
    /// `MAX_TOC_FIXPOINT_ITERATIONS`, and the FINAL toc pages' own printed
    /// numbers must match the FINAL body pages' own global positions.
    #[test]
    fn toc_insertion_shifts_a_headings_page_and_converges_with_matching_final_numbers() {
        use uzor::fonts::FontFamily;
        use uzor_text::StyledRun;

        use crate::master::Margins;

        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 14.0);
        let style = ComposeStyle::new(4.0, font);

        // A small physical page (so a handful of TOC rows genuinely
        // spans more than one TOC page) and a body tall enough that its
        // own heading sits on body-local page 2.
        let body_master = PageMaster::new(300.0, 220.0, Margins::uniform(20.0));
        let toc_master = PageMaster::new(300.0, 220.0, Margins::uniform(20.0));
        let body_width = body_master.body_rect().width;

        let filler_run = [StyledRun::new(
            "Filler text repeated to consume enough vertical space that the tagged heading below \
            lands on the body's own second page, not its first.",
            font,
        )];
        let heading_run = [StyledRun::new("Deep Section", font)];
        let mut body: Vec<BlockNode<'_>> = Vec::new();
        for _ in 0..8 {
            body.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_run, body_width))));
        }
        body.push(BlockNode::new(Block::Paragraph(Paragraph::new(&heading_run, body_width))).with_outline(1, "Deep Section"));

        let toc_style = TocStyle { row_gap_px: 40.0, ..TocStyle::default() }; // exaggerated row gap forces the tiny 1-entry TOC to still behave predictably
        let body_pages_for_toc = slice_pages(&body, &body_master, &style, &shaper);
        let pages = compose_document_with_toc(body_pages_for_toc, &toc_master, &toc_style, &shaper, None);

        // Independently confirm the body alone (no TOC) would have put
        // the heading on body-local page index >= 1 (i.e. NOT page 0) —
        // otherwise this fixture doesn't actually exercise a shift.
        let body_alone = slice_pages(&body, &body_master, &style, &shaper);
        let body_local_index = body_alone
            .iter()
            .find(|p| !p.outline.is_empty())
            .expect("heading must land on some body-local page")
            .index;
        assert!(body_local_index >= 1, "fixture must be tuned so the heading is NOT on the body's own first page");

        // The TOC section occupies the pages BEFORE the body's own first
        // page in the final concatenation — the body's own page COUNT
        // never changes (its composition is independent of the TOC), so
        // the TOC's own page count is exactly the difference.
        let toc_page_count = pages.len() - body_alone.len();
        let final_entry = pages.iter().flat_map(|p| p.outline.iter()).find(|e| e.title == "Deep Section").expect("heading's outline entry must survive to the final document");

        assert_eq!(
            final_entry.page_index,
            toc_page_count as u32 + body_local_index,
            "the final global page index must equal the TOC's own page count plus the heading's body-local index"
        );

        // The printed TOC row's own number must match this SAME final
        // page index (1-based).
        let toc_row_text: Vec<String> = pages
            .iter()
            .take(toc_page_count)
            .flat_map(|p| p.frame.blocks.iter())
            .filter_map(|b| b.paragraph_layout.as_ref())
            .map(|layout| layout.glyphs.iter().map(|g| g.cluster.as_str()).collect::<String>())
            .collect();
        let joined = toc_row_text.join(" ");
        let expected_number = (final_entry.page_index + 1).to_string();
        assert!(joined.contains(&expected_number), "the rendered TOC row must contain the FINAL, correctly-shifted page number {expected_number:?}, got {joined:?}");
    }
}
