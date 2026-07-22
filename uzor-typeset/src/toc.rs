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
//! ## Row text backing — a caller-held [`TocArena`], not a leak (report)
//!
//! Every `BlockNode<'a>`/`Paragraph<'a>` in this crate is BORROWED by
//! design (design law 3: stateless layout over borrowed snapshots,
//! mirroring `uzor_text::Paragraph<'a>`'s own convention) — every
//! existing fixture in this crate (tests, the showcase) authors its
//! runs/blocks as literal `&'static str`/caller-owned `String`s already
//! alive in the SAME scope `compose()`/`slice_pages()` is called from. A
//! TOC row's own text (title + a MEASURED leader + a resolved page
//! number) is instead SYNTHESIZED at compose time — there is no caller
//! scope that could have authored it ahead of time, because the numbers
//! it prints depend on a prior composition pass.
//!
//! [`build_toc`]/[`compose_document_with_toc`] used to resolve this via
//! `Vec::leak`/`Box::leak` (this module's OLD approach, superseded here —
//! see [`TocArena`]'s own doc comment for exactly why that specific leak
//! is now gone for row text, and why a small, fixed residual leak
//! remains for each row's tiny `StyledRun`/`InlineBoxSlot` backing
//! regardless). [`TocArena`] is the caller-held replacement: the caller
//! constructs one, keeps it alive at least as long as it uses whatever
//! `Page`s [`compose_document_with_toc`] hands back, and lets it drop
//! naturally wherever it goes out of scope.

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

/// Caller-held backing storage for synthesized TOC row text (and, per
/// [`crate::caption::attach_captions`]'s own additive reuse of this SAME
/// type, synthesized figure/table caption text) — the arena that
/// replaces this module's former `Box::leak`-per-row-text convention.
///
/// The caller constructs ONE `TocArena`, keeps it alive at least as long
/// as it uses whatever `Page`s [`compose_document_with_toc`]/
/// [`crate::caption::attach_captions`] hand back (a placed `Paragraph`
/// holds a live `&'arena str` back into this arena's own storage — design
/// law 1, painting reads straight from `Block`, never a second copy),
/// then lets it drop naturally wherever it goes out of scope. A plain
/// owned-strings holder: `Vec<String>` push + borrow, no `unsafe`, no new
/// dependency.
///
/// ## Why this holds ONLY text, not the whole per-row `Paragraph` backing
/// (report — a genuine Rust type-system wall, not an incomplete fix)
///
/// `uzor_text::Paragraph<'a>::runs: &'a [StyledRun<'a>]` is a BORROWED
/// slice (same for this module's own leading-indent `InlineBoxSlot`) —
/// building one needs SOME stable-address storage holding the actual
/// values, and that storage would need to borrow ITS OWN text back out of
/// `self.texts` — a struct that owns BOTH data AND a second field whose
/// elements borrow `&'a str` from that SAME owned data is the textbook
/// Rust self-referential-struct limitation: a struct's own lifetime
/// parameter cannot be tied to "however long THIS SPECIFIC INSTANCE's own
/// other field happens to remain valid." There is no 100%-safe,
/// standard-library-only way around this — every real safe-Rust arena
/// crate that supports "push now, borrow indefinitely, push again later"
/// (`typed-arena`, `bumpalo`) does so via `unsafe` internally (raw
/// pointers into a bump allocator); the ONLY 100%-safe escape hatch the
/// standard library itself offers is exactly the `Vec::leak`/`Box::leak`
/// this arena exists to reduce (leak makes a reference `'static` —
/// valid forever, sidestepping "tied to one instance's own lifetime" by
/// not being tied to any instance at all). Given this feature's own "no
/// `unsafe`, no new deps" constraint, [`TocArena`] is therefore scoped to
/// eliminate the leak for the part that actually DOMINATES memory
/// (row/caption TEXT — proportional to title length and TOC/document
/// size); each row's tiny, FIXED-SIZE (one `StyledRun`, one
/// `InlineBoxSlot`) structural backing still uses `Vec::leak`/`Box::leak`
/// — proportional only to ROW COUNT, never to title length, i.e. the part
/// of the ORIGINAL leak that was never the actual size concern.
#[derive(Debug, Default)]
pub struct TocArena {
    texts: Vec<String>,
}

impl TocArena {
    pub fn new() -> Self {
        Self { texts: Vec::new() }
    }

    /// Push an owned string into this arena's storage, returning the
    /// index [`TocArena::text`] retrieves it back at. Returns an index,
    /// never a reference — this arena's callers ([`build_toc`],
    /// [`crate::caption::attach_captions`]) push EVERY string a call
    /// needs first (this arena's own two-phase discipline), then borrow
    /// them all back in a separate pass; a naive `push(&mut self) ->
    /// &str` signature would tie every subsequent push to the
    /// FIRST push's own returned borrow, making a second push a
    /// genuine, correctly-rejected aliasing conflict (see this type's
    /// own doc comment for the deeper reason this arena can't offer a
    /// per-push returned reference at all).
    pub(crate) fn push(&mut self, s: String) -> usize {
        self.texts.push(s);
        self.texts.len() - 1
    }

    /// Borrow row/caption text back out of this arena at `index` — only
    /// meaningful AFTER every [`TocArena::push`] a call site needs has
    /// already happened (see the two-phase discipline above).
    pub(crate) fn text(&self, index: usize) -> &str {
        &self.texts[index]
    }
}

/// Turn `entries` into real, composable `Block::Paragraph` flow nodes —
/// one per row, title left + dotted leader + page number all on one line
/// (via [`build_toc_rows`]), indented per level via a leading [`InlineBox`]
/// spacer (never literal leading whitespace — this module's own doc
/// comment). Rows are ordinary blocks, composed/sliced exactly like any
/// other paragraph in this crate (`compose()`/`slice_pages()` — real
/// pagination, real vector text once exported via `pages_to_pdf`) — the
/// returned `'arena` lifetime ties to `arena`'s own borrow (see
/// [`TocArena`]'s own doc comment), invisible to the caller beyond the
/// lifetime annotation itself: a `Vec<BlockNode<'arena>>` composes into
/// `Page<'arena>`, a valid `Page<'a>` for any `'a` `arena` outlives — free
/// to concatenate with an ordinary borrowed body flow's own `Page<'a>`s.
/// Each row's own `BlockNode` is ALSO tagged via
/// [`BlockNode::with_link_target`] against its own entry's already-
/// resolved `page_index` — a TOC row is therefore a real internal-link
/// (GoTo) source once composed via [`crate::slice::slice_pages`], no
/// separate wiring needed at the PDF-export layer beyond reading
/// [`crate::slice::Page::links`] (which `uzor-typeset::export::pdf_adapter`
/// already does).
pub fn build_toc<'arena>(
    arena: &'arena mut TocArena,
    entries: &[OutlineEntry],
    style: &TocStyle,
    row_width: f64,
    shaper: &dyn LineShaper,
) -> Vec<BlockNode<'arena>> {
    let rows = build_toc_rows(entries, style, row_width, shaper);

    // Phase 1 (mutate): push every row's synthesized text into `arena`
    // BEFORE taking any borrow of it — see `TocArena`'s own doc comment
    // for why interleaving push+borrow can't work in safe Rust.
    let mut text_indices = Vec::with_capacity(rows.len());
    for row in &rows {
        text_indices.push(arena.push(row.text.clone()));
    }

    // Phase 2 (borrow): `arena` is fully populated for this call now —
    // every text lookup below borrows `arena` SHARED, with lifetime
    // 'arena (this arena is never mutated again after this point).
    let mut out = Vec::with_capacity(rows.len());
    for ((row, entry), text_index) in rows.into_iter().zip(entries.iter()).zip(text_indices) {
        let text: &'arena str = arena.text(text_index);
        let runs: &'arena [StyledRun<'arena>] = &*vec![StyledRun::new(text, style.font)].leak();
        let boxes: &'arena [InlineBoxSlot] = &*vec![InlineBoxSlot::new(0, 0, InlineBox::in_flow(0, row.indent_px, 1.0))].leak();
        let paragraph = Paragraph::new(runs, row_width).with_inline_boxes(boxes);
        out.push(BlockNode::new(Block::Paragraph(paragraph)).with_link_target(entry.page_index));
    }
    out
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
///
/// ## Two phases against `arena` (report — a REQUIRED shape, not a style
/// choice)
///
/// `arena: &'a mut TocArena` is borrowed for THIS function's own FULL
/// `'a` (the same lifetime the final returned `Vec<Page<'a>>` needs) —
/// but a `&'a mut` reborrow taken and used ACROSS MULTIPLE LOOP
/// ITERATIONS (one per fixpoint pass) cannot compile: the borrow checker
/// type-checks a loop BODY once, uniformly, for every dynamic pass — if
/// ANY code path inside it (here, the `return` on a converged iteration)
/// requires the reborrow to last `'a`, EVERY iteration's own reborrow is
/// held to that SAME requirement, which then conflicts with the NEXT
/// iteration's own reborrow of the identical `arena`. The fix:
/// **discover the converged page-count OFFSET first, using a throwaway,
/// per-iteration `TocArena` that never touches the caller's own `arena`
/// at all** (a probe iteration only ever reads `toc_pages.len()` — a
/// plain `u32` — so nothing it builds needs to outlive that ONE loop
/// pass, hence no `.leak()` either, an improvement over the OLD
/// per-iteration-leak convention this function used to rely on); **then
/// build the REAL TOC exactly once**, at the now-known offset, against
/// the caller's own long-lived `arena` — a single, non-repeated
/// `&mut *arena` use with no competing reborrow anywhere else in this
/// function, which is unconditionally sound.
pub fn compose_document_with_toc<'a>(
    body_pages: Vec<Page<'a>>,
    toc_master: &PageMaster<'a>,
    toc_style: &TocStyle,
    shaper: &dyn LineShaper,
    page_number_style: Option<&PageNumberStyle>,
    arena: &'a mut TocArena,
) -> Vec<Page<'a>> {
    let toc_row_width = toc_master.body_rect().width;
    let toc_compose_style = ComposeStyle::new(toc_style.row_gap_px, toc_style.font);

    // Phase 1: discover the converged offset. Every probe here is
    // entirely LOCAL (its own throwaway `TocArena`, an owned, non-leaked
    // `Vec<BlockNode<'_>>`) — dropped at the end of each loop pass,
    // never touching the caller's own `arena`.
    let mut toc_page_offset = 0u32;
    let mut converged = false;
    for _ in 0..MAX_TOC_FIXPOINT_ITERATIONS {
        let entries = collect_shifted_entries(&body_pages, toc_page_offset);
        let mut probe_arena = TocArena::new();
        let probe_flow = build_toc(&mut probe_arena, &entries, toc_style, toc_row_width, shaper);
        let probe_pages = slice_pages(&probe_flow, toc_master, &toc_compose_style, shaper);
        let new_offset = probe_pages.len() as u32;
        if new_offset == toc_page_offset {
            converged = true;
            break;
        }
        toc_page_offset = new_offset;
    }

    if converged {
        // Phase 2: build the REAL TOC exactly once, at the converged
        // offset, against the caller's own `arena` — a placed
        // `Paragraph` in the returned pages holds a live `&'a str` back
        // into it (design law 1), so `arena` must outlive whatever the
        // caller does with the returned `Vec<Page<'a>>`.
        let entries = collect_shifted_entries(&body_pages, toc_page_offset);
        // The TOC flow's own backing array (its per-row `StyledRun`/
        // `InlineBoxSlot` arrays) is still leaked here — only the ROW
        // TEXT itself moved to `arena`, see `TocArena`'s own doc comment
        // for exactly why that residual leak can't be eliminated without
        // `unsafe`. The Vec CONTAINER itself must outlive this function
        // call for `slice_pages`'s returned `Page<'a>` to be valid to
        // return (a `&toc_flow` LOCAL-variable borrow would not) — a
        // SINGLE such leak per `compose_document_with_toc` call now
        // (Phase 1's own probes never leak at all), proportional only to
        // this document's own TOC entry COUNT, never to title length
        // (which `arena` now owns instead).
        let toc_flow: &'a [BlockNode<'a>] = build_toc(arena, &entries, toc_style, toc_row_width, shaper).leak();
        let toc_pages = slice_pages(toc_flow, toc_master, &toc_compose_style, shaper);

        let mut all: Vec<Page<'a>> = Vec::with_capacity(toc_pages.len() + body_pages.len());
        all.extend(toc_pages);
        all.extend(body_pages);
        return renumber_pages(all, page_number_style);
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
        let mut arena = TocArena::new();
        let pages = compose_document_with_toc(body_pages_for_toc, &toc_master, &toc_style, &shaper, None, &mut arena);

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

    /// The arena replacement's own core gate: `build_toc`'s returned
    /// `BlockNode`s carry text genuinely BORROWED from `arena` (not
    /// leaked) — every row's rendered paragraph text must byte-for-byte
    /// match [`build_toc_rows`]'s own independently-computed row text.
    #[test]
    fn build_toc_produces_block_nodes_whose_text_is_arena_backed_and_matches_build_toc_rows() {
        let shaper = CosmicShaper::headless();
        let toc_style = style();
        const ROW_WIDTH: f64 = 300.0;

        let entries = [
            OutlineEntry { level: 1, title: "Introduction".to_owned(), page_index: 0 },
            OutlineEntry { level: 2, title: "Background".to_owned(), page_index: 2 },
        ];
        let expected_rows = build_toc_rows(&entries, &toc_style, ROW_WIDTH, &shaper);

        let mut arena = TocArena::new();
        let nodes = build_toc(&mut arena, &entries, &toc_style, ROW_WIDTH, &shaper);
        assert_eq!(nodes.len(), expected_rows.len());

        for (node, expected) in nodes.iter().zip(expected_rows.iter()) {
            let Block::Paragraph(p) = &node.kind else { panic!("a TOC row must be a real Block::Paragraph") };
            assert_eq!(p.runs.len(), 1);
            assert_eq!(p.runs[0].text, expected.text, "row text must round-trip through the arena verbatim");
        }
    }

    /// `TocArena` is a REUSABLE, caller-held value — a caller may drive
    /// more than one `build_toc` call against the SAME arena
    /// SEQUENTIALLY (once an earlier batch's own data has been read out,
    /// its exclusive borrow ends and a later call can reuse the same
    /// arena; overlapping the two would be a genuine, correctly-rejected
    /// aliasing conflict — the reason `compose_document_with_toc`'s own
    /// fixpoint loop uses a fresh, throwaway per-iteration arena for its
    /// PROBE passes and only ever touches the caller's real arena once,
    /// see that function's own doc comment). Pushing a second batch of
    /// rows must never corrupt the FIRST batch's own already-read text.
    #[test]
    fn toc_arena_can_be_reused_sequentially_across_multiple_build_toc_calls_without_corrupting_earlier_rows() {
        let shaper = CosmicShaper::headless();
        let toc_style = style();
        const ROW_WIDTH: f64 = 300.0;

        let entries_a = [OutlineEntry { level: 1, title: "First Pass Heading".to_owned(), page_index: 0 }];
        let entries_b = [OutlineEntry { level: 1, title: "Second Pass Heading".to_owned(), page_index: 3 }];

        let mut arena = TocArena::new();
        let first_text = {
            let nodes_a = build_toc(&mut arena, &entries_a, &toc_style, ROW_WIDTH, &shaper);
            let Block::Paragraph(pa) = &nodes_a[0].kind else { panic!("expected a paragraph") };
            pa.runs[0].text.to_owned()
        };

        let nodes_b = build_toc(&mut arena, &entries_b, &toc_style, ROW_WIDTH, &shaper);
        let Block::Paragraph(pb) = &nodes_b[0].kind else { panic!("expected a paragraph") };

        assert!(first_text.starts_with("First Pass Heading"), "the FIRST batch's own text must have resolved correctly before the arena was reused, got {first_text:?}");
        assert!(pb.runs[0].text.starts_with("Second Pass Heading"), "the SECOND batch must resolve its own distinct text after reuse, got {:?}", pb.runs[0].text);
    }
}
