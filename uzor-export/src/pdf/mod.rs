//! Neutral PDF assembly — Arc 4 Phase P5 (`nemo/docs/uzor-engines/
//! uzor_typeset_arc4_design.md` §6), overhauled per the export SOTA
//! research pass (`nemo/docs/uzor-engines/research_export_sota_2026.md`,
//! items 1-4). This module defines an **engine-agnostic** content model +
//! builder for a hybrid-fidelity PDF (real, selectable/searchable/
//! **full-Unicode** vector text + one full-page raster background per
//! page); it holds no `uzor-text`/`uzor-figures`/`uzor-typeset` knowledge
//! whatsoever (design law 9 / §6.1's dependency-boundary constraint) — the
//! adapter that knows `Page`/`ParagraphLayout` lives in
//! `uzor-typeset::export` instead, and calls exactly the same public
//! surface as before this pass (this rewrite is a drop-in replacement:
//! [`PdfBuilder`]/[`PdfFont`]/[`PdfTextRun`]/[`PdfPageSpec`]'s public
//! shapes are byte-for-byte unchanged; only [`PdfBuilder::set_meta`] is
//! new).
//!
//! ## Hybrid fidelity model
//!
//! Unchanged from the original P5 design: every page is (up to) two
//! layers, painted back-to-front — one full-page opaque raster background
//! (figures/tables/images/suppressed paragraph ink), then real vector text
//! runs on top of it.
//!
//! ## Coordinate convention
//!
//! Unchanged: every public coordinate is top-left origin, y growing
//! downward; [`PdfBuilder`] converts to PDF's own bottom-left origin
//! internally (see `write_page`'s own doc comment).
//!
//! ## Font embedding: Type0/CIDFontType2, Identity-H, subsetted, Flate
//!
//! **The original P5 phase shipped a hard functional bug for this crate's
//! own stated purpose**: fonts embedded as PDF **simple** TrueType fonts
//! under the predefined `/Encoding /WinAnsiEncoding` name can only address
//! Latin-1 plus the `0x80..=0x9F` cp1252 punctuation block — Cyrillic
//! (and every other non-Latin script) silently rendered as `'?'`. This
//! pass replaces that path wholesale with the reference architecture
//! `typst-pdf` (the same `pdf-writer` author's own consumer) uses:
//!
//! 1. **Type0/CIDFontType2, `/Encoding /Identity-H`.** Every font is now a
//!    composite font: a thin `Type0` wrapper dict referencing a descendant
//!    `CIDFontType2` dict (`CIDSystemInfo` = Adobe/Identity/0,
//!    `/CIDToGIDMap /Identity`). A page's content stream shows 2-byte
//!    codes per glyph (`Content::show`, still `Tj` — `pdf-writer`'s own
//!    `Str` writer picks hex-vs-literal-with-escapes per string
//!    automatically, see its own doc comment) instead of the old 1-byte
//!    WinAnsi codes.
//! 2. **Full-Unicode `cmap` reader.** [`ttf::TtfMetrics`] now walks its
//!    preferred `cmap` format-4 subtable in full, building a `char -> gid`
//!    map over every codepoint that subtable actually covers (still BMP
//!    only — a documented v1 scope limit, see that module's own doc
//!    comment — but that's every script this crate's own report/deck
//!    consumers need, Cyrillic included) instead of a fixed 256-byte
//!    WinAnsi table.
//! 3. **Subsetting** via `subsetter` (`typst/subsetter`, same
//!    author/lineage as `pdf-writer`) — [`subset::build_font_data`]
//!    collects the actual glyph-id set a font used across the WHOLE
//!    document, remaps it to a compact CID space, and asks `subsetter` to
//!    produce a subset font program containing only those glyphs (falling
//!    back to the full, unmodified font — never a panic — if `subsetter`
//!    can't handle an exotic font program).
//! 4. **`/ToUnicode`**, built from the SAME `(gid, char)` pairs the CIDs
//!    themselves come from (never independently — see `subset`'s own doc
//!    comment for the exact bug class this avoids), so copy-paste/search
//!    recovers the original text verbatim.
//!
//! `winansi.rs` (the old `WinAnsiEncoding` char<->byte table) is DELETED,
//! not deprecated — nothing in this crate reads a byte-keyed encoding
//! anymore (hard cutover, per this workspace's own convention for a
//! superseded internal module).
//!
//! ## Flate compression
//!
//! Every stream this module writes (content streams, raster image
//! XObjects, embedded font programs, `/ToUnicode` CMaps) is now
//! zlib/DEFLATE-compressed (`Filter::FlateDecode`) via [`flate_compress`].
//! `pdf-writer` deliberately does no compression itself (`Chunk::stream`'s
//! own doc comment shows the exact `miniz_oxide` pattern this module
//! follows) — this was the single largest contributor to the pre-pass
//! 66MB/11-page file size (a full-page raster background embedded as raw,
//! undecoded RGB8, 3 bytes/pixel, no filter at all).
//!
//! ## Metadata
//!
//! [`PdfBuilder::set_meta`] additively accepts a caller-supplied
//! [`PdfMeta`] (title/producer/creation date) written as the PDF `/Info`
//! dictionary. This module holds no wall-clock access of its own — a
//! creation date only appears if the caller supplies one via [`PdfDate`].
//! XMP metadata (`xmp-writer`) was evaluated and deliberately NOT added
//! this pass — the research doc's own escape hatch ("XMP only if trivial,
//! else skip") — `/Info` alone already satisfies "professional polish, no
//! new correctness risk" without a second metadata representation that
//! could drift from the first.

mod font_cache;
mod outline;
mod render_context;
mod subset;
mod ttf;

use std::collections::HashMap;

use pdf_writer::types::{ActionType, AnnotationType, CidFontType, FontFlags, SystemInfo};
use pdf_writer::{Content, Date, Filter, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};

use crate::ExportError;
use outline::write_outline_tree;
use subset::{build_font_data, FontData, ADOBE_IDENTITY_UCS};

pub use font_cache::PdfFontCache;
pub use render_context::{PdfContentStream, PdfRenderContext};
use render_context::{emit_ops, PdfOp};

/// Opaque handle to a font registered via [`PdfBuilder::register_font`].
/// Deliberately a lightweight `Copy` id, not a borrowed `&PdfFont` — a
/// [`PdfBuilder`] must stay mutably reachable for [`PdfBuilder::add_page`]
/// after fonts were registered earlier, which a borrow into the SAME
/// builder's own font registry would make impossible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(u32);

/// A font registered with a [`PdfBuilder`] — its handle plus the (full,
/// unmodified — subsetting happens once at [`PdfBuilder::finish`] time,
/// see this module's own doc comment) TTF bytes that will be embedded.
pub struct PdfFont {
    pub id: FontId,
    pub ttf_bytes: Vec<u8>,
}

/// One run of same-font, same-color text painted starting at a single
/// baseline origin `(x_pt, y_pt)` (top-left page coordinates — see this
/// module's own doc comment). `text` is resolved to this run's own font's
/// real glyph ids when the page is added ([`ttf::TtfMetrics::gid_for_char`]);
/// a character with no glyph in the font falls back to glyph id `0`
/// (`.notdef`) rather than being dropped.
pub struct PdfTextRun<'a> {
    pub font: FontId,
    pub size_pt: f64,
    pub x_pt: f64,
    pub y_pt: f64,
    /// Packed `0xRRGGBB`.
    pub rgb: u32,
    pub text: &'a str,
    /// Real per-glyph shaped advance (px == pt, this module's own "1pt =
    /// 1px" convention), one entry per `char` in `text` (typography-gap
    /// WAVE 2 — per-glyph PDF kerning). `None` trusts the embedded font's
    /// own static declared widths verbatim for every glyph in this run —
    /// byte-identical to this field's pre-WAVE-2 absence, the default for
    /// EVERY figure/chrome text call (`PdfRenderContext::fill_text` never
    /// has a real shaper's own advances to hand — see `render_context.rs`)
    /// and for a caller that genuinely doesn't know its own real advances.
    /// `Some(advances)` shorter than `text`'s own char count is padded
    /// with the font's own static width for the missing tail (never a
    /// panic, never silently truncates the run); a LONGER `advances` has
    /// its extra entries ignored.
    pub glyph_advances_pt: Option<Vec<f64>>,
}

/// One PDF page: its physical size, an optional full-page opaque raster
/// background, every vector text run painted on top of it, and every
/// internal link annotation on it (document-navigation feature pass).
pub struct PdfPageSpec<'a> {
    pub width_pt: f64,
    pub height_pt: f64,
    /// Encoded PNG bytes for a full-page, fully-opaque background image
    /// (see this module's own doc comment for why no alpha/SMask is ever
    /// embedded), or `None` for a text-only page.
    pub raster: Option<&'a [u8]>,
    /// The raster's own pixel dimensions, cross-checked against the
    /// decoded PNG's own header at [`PdfBuilder::add_page`] time
    /// (`ExportError::RasterDimensionMismatch` on a mismatch — never
    /// silently trusting one over the other).
    pub raster_px: (u32, u32),
    pub text_runs: Vec<PdfTextRun<'a>>,
    /// Internal-link (`GoTo`) rectangles on this page — additive,
    /// defaults to empty for every pre-existing caller. See [`PdfLink`].
    pub links: Vec<PdfLink>,
    /// Real PDF content-stream ops (paths/fills/strokes/clips/text/images/
    /// gradients) accumulated by a [`PdfRenderContext`] — typography-gap
    /// WAVE 1: figures/tables/page chrome render THROUGH this instead of
    /// the whole-page raster background. Additive (`PdfContentStream::
    /// empty()` for every pre-existing caller); painted AFTER `raster`
    /// (if any) and BEFORE `text_runs`, in recorded order (see
    /// [`render_context`]'s own module doc for the paint-order
    /// reasoning).
    pub content: PdfContentStream,
}

/// One document-outline (bookmark) entry — a caller-resolved heading
/// already tied to its own PDF page index (0-based, matching this
/// module's own per-`add_page`-call ordering). Plain, engine-agnostic
/// data (design law: this module holds no `uzor-typeset`/`Page`
/// knowledge — the adapter that knows `Page::outline` is the only place
/// allowed to construct this, same dependency-boundary convention
/// [`PdfTextRun`]/[`PdfPageSpec`] already follow).
#[derive(Debug, Clone, PartialEq)]
pub struct PdfOutlineEntry {
    /// 1-based, matching heading-level convention (`1` = top-level
    /// section) — see [`outline::write_outline_tree`]'s own doc comment
    /// for how nesting/open-by-default is derived from this.
    pub level: u8,
    pub title: String,
    pub page_index: u32,
}

/// One internal-link (`GoTo`) rectangle on a page — `rect` in the SAME
/// top-left-origin coordinate convention every other public coordinate in
/// this module already uses (`x_pt`/`y_pt` = top-left corner). URI
/// (external) links are explicitly NOT in scope this pass — see this
/// crate's own `CLAUDE.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfLink {
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    /// 0-based target page index, matching [`PdfOutlineEntry::page_index`]'s
    /// own convention.
    pub target_page: u32,
}

/// Caller-supplied document metadata for the PDF `/Info` dictionary
/// ([`PdfBuilder::set_meta`]). Every field is optional; whichever the
/// caller supplies is the ONLY thing written. This crate holds no
/// wall-clock access of its own (design law: no clocks inside a neutral
/// library) — [`Self::creation_date`] must come from the caller if wanted
/// at all.
#[derive(Debug, Clone, Default)]
pub struct PdfMeta {
    pub title: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<PdfDate>,
}

/// A caller-supplied point in time for [`PdfMeta::creation_date`] — plain
/// calendar fields, never resolved from a system clock inside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl PdfDate {
    fn to_pdf_writer_date(self) -> Date {
        Date::new(self.year).month(self.month).day(self.day).hour(self.hour).minute(self.minute).second(self.second)
    }
}

struct FontEntry {
    font: PdfFont,
    metrics: ttf::TtfMetrics,
}

struct PageTextRun {
    font_id: FontId,
    size_pt: f64,
    x_pt: f64,
    y_pt: f64,
    rgb: u32,
    /// `(original glyph id in the FULL, pre-subsetting font, source
    /// Unicode scalar)` pairs, one per rendered character, in run order.
    /// Resolved eagerly here at [`PdfBuilder::add_page`] time (every font
    /// this run references is already registered by then), not deferred
    /// to [`PdfBuilder::finish`] — subsetting/CID-remapping is the only
    /// thing that waits until every page is known.
    glyphs: Vec<(u16, char)>,
    /// Moved verbatim from [`PdfTextRun::glyph_advances_pt`] — typography-
    /// gap WAVE 2 per-glyph PDF kerning. Resolved into real `TJ` array
    /// adjustments at [`write_page`] time (needs each font's own static
    /// declared widths, only fully known once every page's glyph usage is
    /// collected — same timing every other subsetting-dependent value in
    /// this module already waits for).
    real_advances_pt: Option<Vec<f64>>,
}

struct PageRecord {
    width_pt: f64,
    height_pt: f64,
    /// Decoded, alpha-dropped `RGB8`, tightly packed, plus its own
    /// `(width, height)` — `None` for a text-only page.
    raster_rgb: Option<(Vec<u8>, u32, u32)>,
    runs: Vec<PageTextRun>,
    /// Owned verbatim from [`PdfPageSpec::links`] — [`PdfLink`] borrows
    /// nothing, so no resolution work is needed at [`PdfBuilder::add_page`]
    /// time (unlike `runs`, which resolves characters to glyph ids
    /// eagerly).
    links: Vec<PdfLink>,
    /// Moved verbatim from [`PdfPageSpec::content`] — every [`PdfOp::Text`]
    /// op already carries its own resolved `(gid, char)` pairs (a
    /// [`PdfRenderContext`] resolves them eagerly at `fill_text` call
    /// time, the SAME timing `runs` above uses), so no further work is
    /// needed here either; [`subset::build_font_data`]'s glyph-usage walk
    /// reads this field too (see that function's own doc comment).
    content_ops: Vec<PdfOp>,
}

/// Accumulates registered fonts + added pages, then assembles one PDF byte
/// buffer via [`PdfBuilder::finish`]. Deliberately builds nothing with
/// `pdf-writer` until `finish()` — subsetting needs every page's own glyph
/// usage known first (see [`subset::build_font_data`]), so no font/page
/// object can be written before the whole document is known.
pub struct PdfBuilder {
    fonts: Vec<FontEntry>,
    pages: Vec<PageRecord>,
    meta: Option<PdfMeta>,
    outline: Vec<PdfOutlineEntry>,
}

impl Default for PdfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self { fonts: Vec::new(), pages: Vec::new(), meta: None, outline: Vec::new() }
    }

    /// Register a font's raw TTF/OpenType bytes, returning a handle to
    /// reference from [`PdfTextRun::font`]. Parsing this font's metrics
    /// ([`ttf::TtfMetrics::parse`]) never fails — a malformed font degrades
    /// to conservative fallback constants rather than erroring, so this
    /// method itself never fails either.
    ///
    /// Registering the SAME logical font (family/weight/style) more than
    /// once embeds it more than once — deduplication across a whole
    /// document (design doc §6.1: "one embedded/subset font resource per
    /// unique `FontSpec` ... deduped, never re-embedded per page") is the
    /// ADAPTER's responsibility (it already knows what a "unique
    /// `FontSpec`" is; this neutral builder deliberately does not).
    pub fn register_font(&mut self, ttf_bytes: &[u8]) -> FontId {
        let id = FontId(self.fonts.len() as u32);
        let metrics = ttf::TtfMetrics::parse(ttf_bytes);
        self.fonts.push(FontEntry { font: PdfFont { id, ttf_bytes: ttf_bytes.to_vec() }, metrics });
        id
    }

    /// Attach document metadata, written as the PDF `/Info` dictionary at
    /// [`Self::finish`] time. Additive — a [`PdfBuilder`] that never calls
    /// this writes no `/Info` dict at all, byte-identical to this pass's
    /// predecessor.
    pub fn set_meta(&mut self, meta: PdfMeta) {
        self.meta = Some(meta);
    }

    /// Attach a document-outline (bookmark) tree, written as the PDF
    /// `/Outlines` entry at [`Self::finish`] time. Additive — a
    /// [`PdfBuilder`] that never calls this writes no `/Outlines` entry
    /// at all (same opt-in convention as [`Self::set_meta`]). `entries`
    /// is a FLAT, level-tagged list in document order — nesting is
    /// derived from consecutive levels (see [`outline::write_outline_tree`]'s
    /// own doc comment); an entry whose `page_index` doesn't correspond
    /// to any page added via [`Self::add_page`] by [`Self::finish`] time
    /// is silently skipped (never a panic).
    pub fn set_outline(&mut self, entries: Vec<PdfOutlineEntry>) {
        self.outline = entries;
    }

    /// Add one page. Decodes `spec.raster` (if present) into a tightly
    /// packed, alpha-dropped `RGB8` buffer and resolves every text run's
    /// own characters to this run's font's real glyph ids up front —
    /// `finish()` therefore does no fallible work at all, every error this
    /// builder can produce surfaces here, at the call site closest to its
    /// actual cause.
    pub fn add_page(&mut self, spec: PdfPageSpec<'_>) -> Result<(), ExportError> {
        let raster_rgb = match spec.raster {
            Some(png_bytes) => Some(decode_opaque_rgb(png_bytes, spec.raster_px)?),
            None => None,
        };

        let runs = spec
            .text_runs
            .into_iter()
            .map(|run| {
                let metrics = &self.fonts[run.font.0 as usize].metrics;
                let glyphs = run.text.chars().map(|ch| (metrics.gid_for_char(ch).unwrap_or(0), ch)).collect();
                PageTextRun {
                    font_id: run.font,
                    size_pt: run.size_pt,
                    x_pt: run.x_pt,
                    y_pt: run.y_pt,
                    rgb: run.rgb,
                    glyphs,
                    real_advances_pt: run.glyph_advances_pt,
                }
            })
            .collect();

        self.pages.push(PageRecord { width_pt: spec.width_pt, height_pt: spec.height_pt, raster_rgb, runs, links: spec.links, content_ops: spec.content.0 });
        Ok(())
    }

    /// This font's own [`ttf::TtfMetrics`] — [`PdfRenderContext::fill_text`]'s
    /// own read access to the SAME per-font metrics [`Self::add_page`]
    /// already uses to resolve a [`PdfTextRun`]'s characters to glyph ids,
    /// so a figure/chrome `fill_text` call and a paragraph text run
    /// resolve identically. `id` is always one this builder itself handed
    /// out via [`Self::register_font`] (a [`PdfFontCache`] never
    /// fabricates one), so the index is always in range.
    fn font_metrics(&self, id: FontId) -> &ttf::TtfMetrics {
        &self.fonts[id.0 as usize].metrics
    }

    /// Assemble every registered font + added page into one complete,
    /// standalone PDF byte buffer.
    pub fn finish(self) -> Vec<u8> {
        let mut pdf = Pdf::new();
        let mut refs = RefAllocator::new();

        let catalog_id = refs.next();
        let page_tree_id = refs.next();

        // Per-font Type0/CID data: used-glyph collection -> subsetting ->
        // widths/ToUnicode/CID-remap tables. Must happen before any page's
        // content stream is written (page writing needs each font's own
        // `orig_to_new` table to translate glyph runs into CIDs).
        let font_data: Vec<FontData> =
            self.fonts.iter().enumerate().map(|(i, entry)| build_font_data(i as u32, &entry.font.ttf_bytes, &entry.metrics, &self.pages)).collect();

        let font_refs: Vec<FontRefs> = self
            .fonts
            .iter()
            .map(|_| FontRefs { type0: refs.next(), cid: refs.next(), descriptor: refs.next(), file: refs.next(), to_unicode: refs.next() })
            .collect();

        let page_refs: Vec<PageRefs> = self
            .pages
            .iter()
            .map(|page| PageRefs {
                page: refs.next(),
                content: refs.next(),
                image: page.raster_rgb.as_ref().map(|_| refs.next()),
                annotations: page.links.iter().map(|_| refs.next()).collect(),
            })
            .collect();

        let info_ref = self.meta.as_ref().map(|_| refs.next());

        // Document-outline (bookmark) tree — allocates its OWN item/root
        // refs internally; needs every page's own `Ref` + physical height
        // already known (page-top `/XYZ` destinations), so it runs AFTER
        // `page_refs` above but before anything is actually WRITTEN.
        let page_only_refs: Vec<Ref> = page_refs.iter().map(|r| r.page).collect();
        let page_heights_pt: Vec<f64> = self.pages.iter().map(|p| p.height_pt).collect();
        let outline_root_ref = write_outline_tree(&mut pdf, &mut refs, &self.outline, &page_only_refs, &page_heights_pt);

        let font_names: Vec<String> = (0..self.fonts.len()).map(|i| format!("F{i}")).collect();
        let base_font_names: Vec<String> = font_data
            .iter()
            .enumerate()
            .map(|(i, data)| match &data.subset_tag {
                Some(tag) => format!("{tag}+EmbeddedFont{i}"),
                None => format!("EmbeddedFont{i}"),
            })
            .collect();
        let image_names: Vec<String> = (0..self.pages.len()).map(|i| format!("Im{i}")).collect();

        {
            let mut catalog = pdf.catalog(catalog_id);
            catalog.pages(page_tree_id);
            if let Some(root) = outline_root_ref {
                catalog.outlines(root);
            }
        }
        pdf.pages(page_tree_id).kids(page_refs.iter().map(|r| r.page)).count(page_refs.len() as i32);

        for (entry, refs, data, base_font_name) in izip(&self.fonts, &font_refs, &font_data, &base_font_names) {
            write_font(&mut pdf, refs, base_font_name, entry, data);
        }

        for (i, page) in self.pages.iter().enumerate() {
            write_page(
                &mut pdf,
                &mut refs,
                page_tree_id,
                &page_refs[i],
                &font_names,
                &font_refs,
                page,
                image_names[i].as_str(),
                &font_data,
                &page_only_refs,
                &page_heights_pt,
                &self.fonts,
            );
        }

        if let (Some(meta), Some(info_id)) = (&self.meta, info_ref) {
            write_info(&mut pdf, info_id, meta);
        }

        pdf.finish()
    }
}

/// A tiny 4-way zip — `Iterator::zip` only nests two at a time and
/// `((a, b), c, d)` tuple-destructuring in a `for` loop reads worse than
/// this at every one of `finish()`'s own call sites below.
fn izip<'a, A, B, C, D>(a: &'a [A], b: &'a [B], c: &'a [C], d: &'a [D]) -> impl Iterator<Item = (&'a A, &'a B, &'a C, &'a D)> {
    a.iter().zip(b.iter()).zip(c.iter()).zip(d.iter()).map(|(((a, b), c), d)| (a, b, c, d))
}

struct RefAllocator(i32);

impl RefAllocator {
    fn new() -> Self {
        Self(1)
    }

    fn next(&mut self) -> Ref {
        let r = Ref::new(self.0);
        self.0 += 1;
        r
    }
}

struct FontRefs {
    /// The Type0 (composite) font dict — this is what a page's own
    /// `/Resources /Font` entry points at.
    type0: Ref,
    /// The descendant CIDFontType2 dict.
    cid: Ref,
    descriptor: Ref,
    /// The embedded (subset, when possible) font program (`/FontFile2`).
    file: Ref,
    /// The `/ToUnicode` CMap stream.
    to_unicode: Ref,
}

struct PageRefs {
    page: Ref,
    content: Ref,
    image: Option<Ref>,
    /// One [`Ref`] per this page's own [`PdfLink`] (same order as
    /// [`PageRecord::links`]).
    annotations: Vec<Ref>,
}

/// `/CIDSystemInfo` for the CIDFont itself — `Adobe-Identity-0`: a direct
/// GID-as-CID scheme with no predefined character-collection re-encoding,
/// matching the `/Encoding /Identity-H` chosen on the parent Type0 font.
const ADOBE_IDENTITY_0: SystemInfo<'static> = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };

/// Flate/zlib-compress `data` at a balanced compression level — this
/// crate's one shared compression entry point. Content streams, raster
/// image XObjects, embedded (subset) font programs, and `/ToUnicode` CMap
/// streams all go through this SAME function; `pdf-writer` itself does no
/// compression (`Chunk::stream`'s own doc comment shows this exact
/// `miniz_oxide` call as its recommended pattern — see this module's own
/// doc comment for why `miniz_oxide` directly, not `flate2`).
fn flate_compress(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn write_font(pdf: &mut Pdf, refs: &FontRefs, base_font_name: &str, entry: &FontEntry, data: &FontData) {
    let bbox = entry.metrics.bbox_1000();

    {
        let mut descriptor = pdf.font_descriptor(refs.descriptor);
        descriptor.name(Name(base_font_name.as_bytes()));
        // A CID font addresses glyphs by CID, never through
        // StandardEncoding/WinAnsiEncoding character codes — `SYMBOLIC`
        // (not `NON_SYMBOLIC`) is the spec-correct flag for an embedded
        // Identity-H CID font (matches `typst-pdf`'s own convention for
        // this exact font shape).
        descriptor.flags(FontFlags::SYMBOLIC);
        descriptor.bbox(PdfRect::new(bbox[0] as f32, bbox[1] as f32, bbox[2] as f32, bbox[3] as f32));
        // Not parsed from the font's own `post` table (out of this
        // module's narrow scope) — `0.0` never affects the embedded glyph
        // outlines' actual rendered shape, only cosmetic PDF metadata.
        descriptor.italic_angle(0.0);
        descriptor.ascent(entry.metrics.ascent_1000() as f32);
        descriptor.descent(entry.metrics.descent_1000() as f32);
        descriptor.cap_height(entry.metrics.cap_height_1000() as f32);
        // A common flat approximation (many simple PDF generators use a
        // similar constant); only affects hinting-quality heuristics, not
        // glyph shape.
        descriptor.stem_v(80.0);
        descriptor.font_file2(refs.file);
    }

    {
        let compressed = flate_compress(&data.subset_bytes);
        let mut file = pdf.stream(refs.file, &compressed);
        file.filter(Filter::FlateDecode);
        // `/Length1` is always the UNCOMPRESSED byte length (PDF spec
        // requirement for `/FontFile2`) — deliberately NOT the compressed
        // stream body's own length (`/Length`, written automatically).
        file.pair(Name(b"Length1"), data.subset_bytes.len() as i32);
    }

    {
        let mut cid = pdf.cid_font(refs.cid);
        cid.subtype(CidFontType::Type2);
        cid.base_font(Name(base_font_name.as_bytes()));
        cid.system_info(ADOBE_IDENTITY_0);
        cid.font_descriptor(refs.descriptor);
        {
            let mut widths = cid.widths();
            for &(cid_value, width) in &data.widths {
                widths.consecutive(cid_value, [width]);
            }
        }
        // `subsetter::subset`'s own contract: the remapped glyph id IS the
        // CID for every font it produces, regardless of the source font's
        // original CID/GID relationship — so `/CIDToGIDMap /Identity` is
        // always correct here, subsetted or (fallback path) not.
        cid.cid_to_gid_map_predefined(Name(b"Identity"));
    }

    {
        let mut type0 = pdf.type0_font(refs.type0);
        type0.base_font(Name(base_font_name.as_bytes()));
        type0.encoding_predefined(Name(b"Identity-H"));
        type0.descendant_font(refs.cid);
        type0.to_unicode(refs.to_unicode);
    }

    {
        let compressed = flate_compress(&data.to_unicode);
        let mut cmap = pdf.cmap(refs.to_unicode, &compressed);
        cmap.filter(Filter::FlateDecode);
        cmap.name(Name(b"Adobe-Identity-UCS"));
        cmap.system_info(ADOBE_IDENTITY_UCS);
    }
}

/// Write one page's image XObject (if any), content stream, and page
/// dictionary. The coordinate conversion this module's own doc comment
/// promises happens in exactly one place: the text matrix's own `y`
/// component below (`page.height_pt - run.y_pt`) — everywhere else
/// (`x_pt`, the image transform) top-left and bottom-left agree since
/// they share the same origin corner horizontally and the image itself is
/// placed spanning the WHOLE page (`0,0` to `width_pt,height_pt` in BOTH
/// coordinate systems).
#[allow(clippy::too_many_arguments)]
fn write_page(
    pdf: &mut Pdf,
    ref_alloc: &mut RefAllocator,
    page_tree_id: Ref,
    refs: &PageRefs,
    font_names: &[String],
    font_refs: &[FontRefs],
    page: &PageRecord,
    image_name: &str,
    font_data: &[FontData],
    page_refs_by_index: &[Ref],
    page_heights_pt: &[f64],
    fonts: &[FontEntry],
) {
    if let (Some(image_ref), Some((rgb, width, height))) = (refs.image, page.raster_rgb.as_ref()) {
        let compressed = flate_compress(rgb);
        let mut image = pdf.image_xobject(image_ref, &compressed);
        image.filter(Filter::FlateDecode);
        image.width(*width as i32);
        image.height(*height as i32);
        image.color_space().device_rgb();
        image.bits_per_component(8);
    }

    // `op_resources` collects whatever patterns/gstates/inline images
    // `emit_ops` allocates ON DEMAND while building the content stream
    // below — merged into this page's own `/Resources` dict at the very
    // end of this function (see this crate's own `render_context`
    // module doc for why this ordering costs nothing).
    let op_resources = {
        let mut content = Content::new();
        if refs.image.is_some() {
            content.save_state();
            content.transform([page.width_pt as f32, 0.0, 0.0, page.height_pt as f32, 0.0, 0.0]);
            content.x_object(Name(image_name.as_bytes()));
            content.restore_state();
        }
        let op_resources = emit_ops(pdf, ref_alloc, &mut content, &page.content_ops, page.height_pt, font_data, font_names);
        if !page.runs.is_empty() {
            content.begin_text();
            for run in &page.runs {
                let (r, g, b) = rgb_components(run.rgb);
                content.set_fill_rgb(r, g, b);
                content.set_font(Name(font_names[run.font_id.0 as usize].as_bytes()), run.size_pt as f32);
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, run.x_pt as f32, (page.height_pt - run.y_pt) as f32]);
                let orig_to_new = &font_data[run.font_id.0 as usize].orig_to_new;
                let metrics = &fonts[run.font_id.0 as usize].metrics;
                show_run(&mut content, run, orig_to_new, metrics);
            }
            content.end_text();
        }
        let raw = content.finish();
        let compressed = flate_compress(&raw);
        let mut stream = pdf.stream(refs.content, &compressed);
        stream.filter(Filter::FlateDecode);
        op_resources
    };

    // Internal link annotations — one `/Subtype /Link` object per
    // `PdfLink`, GoTo-ing a page-top `/XYZ` destination on its own
    // `target_page` (same page-top convention `outline::write_outline_tree`
    // uses for bookmarks). A link whose `target_page` is out of range is
    // skipped (never a panic) — same defensive backstop as the outline
    // tree's own out-of-range guard.
    for (link, &annot_ref) in page.links.iter().zip(refs.annotations.iter()) {
        let (Some(&target_page_ref), Some(&target_height)) =
            (page_refs_by_index.get(link.target_page as usize), page_heights_pt.get(link.target_page as usize))
        else {
            continue;
        };
        let x1 = link.x_pt as f32;
        let x2 = (link.x_pt + link.width_pt) as f32;
        let y1 = (page.height_pt - (link.y_pt + link.height_pt)) as f32;
        let y2 = (page.height_pt - link.y_pt) as f32;

        let mut annot = pdf.annotation(annot_ref);
        annot.subtype(AnnotationType::Link);
        annot.rect(PdfRect::new(x1, y1, x2, y2));
        // Suppress the default visible border most viewers draw for a
        // Link annotation (export SOTA research pass, item 5's own API
        // sketch: "`BorderStyle` to suppress the default visible border").
        annot.border_style().width(0.0);
        annot.action().action_type(ActionType::GoTo).destination().page(target_page_ref).xyz(0.0, target_height as f32, None);
    }

    {
        let mut page_writer = pdf.page(refs.page);
        page_writer.parent(page_tree_id);
        page_writer.media_box(PdfRect::new(0.0, 0.0, page.width_pt as f32, page.height_pt as f32));
        page_writer.contents(refs.content);
        if !refs.annotations.is_empty() {
            page_writer.annotations(refs.annotations.iter().copied());
        }

        let mut resources = page_writer.resources();
        {
            let mut fonts_dict = resources.fonts();
            for (name, refs) in font_names.iter().zip(font_refs.iter()) {
                fonts_dict.pair(Name(name.as_bytes()), refs.type0);
            }
        }
        if refs.image.is_some() || !op_resources.images.is_empty() {
            let mut x_objects = resources.x_objects();
            if let Some(image_ref) = refs.image {
                x_objects.pair(Name(image_name.as_bytes()), image_ref);
            }
            for (name, image_ref) in &op_resources.images {
                x_objects.pair(Name(name.as_bytes()), *image_ref);
            }
        }
        if !op_resources.patterns.is_empty() {
            let mut patterns = resources.patterns();
            for (name, pattern_ref) in &op_resources.patterns {
                patterns.pair(Name(name.as_bytes()), *pattern_ref);
            }
        }
        if !op_resources.ext_gstates.is_empty() {
            let mut ext_g_states = resources.ext_g_states();
            for (name, gstate_ref) in &op_resources.ext_gstates {
                ext_g_states.pair(Name(name.as_bytes()), *gstate_ref);
            }
        }
    }
}

/// Encode one text run's own `(gid, char)` pairs as a 2-byte-per-glyph,
/// big-endian CID string (`/Encoding /Identity-H`'s own wire format) —
/// each original glyph id is translated through its font's own
/// `orig_to_new` table (built once per font at [`PdfBuilder::finish`]
/// time, see [`subset::build_font_data`]); a glyph absent from that table
/// (shouldn't happen — every glyph a run resolved at [`PdfBuilder::
/// add_page`] time was collected into the SAME font's used-glyph set)
/// falls back to CID `0` (`.notdef`) rather than panicking.
fn cid_bytes(glyphs: &[(u16, char)], orig_to_new: &HashMap<u16, u16>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(glyphs.len() * 2);
    for &(gid, _) in glyphs {
        let cid = orig_to_new.get(&gid).copied().unwrap_or(0);
        bytes.extend_from_slice(&cid.to_be_bytes());
    }
    bytes
}

/// Minimum `|adjustment|` (thousandths-of-text-space units, `TJ`'s own
/// scale) worth emitting as a real gap — typography-gap WAVE 2 per-glyph
/// PDF kerning. Below this a real PDF viewer's own sub-unit rounding makes
/// the difference visually imperceptible, so the glyph simply joins the
/// current string segment uninterrupted.
const MIN_TJ_ADJUST_UNITS: f32 = 1.0;

/// Show one text run's glyphs (typography-gap WAVE 2). `run.real_advances_pt
/// == None` (every figure/chrome [`crate::pdf::render_context::
/// PdfRenderContext::fill_text`] call, and any [`PdfTextRun`] caller that
/// doesn't know its own real shaped advances) degrades to the EXACT
/// pre-WAVE-2 single `Tj` — byte-identical output for every caller that
/// never opts in (existing `content_str.contains("Tj")`-style assertions
/// stay true). `Some(real)` replaces `Tj` with a `TJ` array carrying a real
/// per-glyph positioning adjustment (`static_width_pt - target_pt`,
/// converted to `TJ`'s own thousandths-of-text-space scale) between any
/// two glyphs whose difference clears [`MIN_TJ_ADJUST_UNITS`] — but ONLY
/// when at least one glyph pair in the run actually needs one; a run whose
/// real advances happen to already match the font's own static widths
/// closely still emits the smaller, byte-identical plain `Tj`.
fn show_run(content: &mut Content, run: &PageTextRun, orig_to_new: &HashMap<u16, u16>, metrics: &ttf::TtfMetrics) {
    let Some(real) = run.real_advances_pt.as_deref() else {
        content.show(Str(&cid_bytes(&run.glyphs, orig_to_new)));
        return;
    };

    // Adjustment (TJ units) to insert AFTER glyph `i` (between it and
    // glyph `i + 1`) — none after the last glyph, nothing follows it in
    // this run. A `real` shorter than `run.glyphs` pads the missing tail
    // with the font's own static width (documented `PdfTextRun::
    // glyph_advances_pt` contract — `target_pt == static_width_pt` there
    // yields a zero adjustment, never a truncated run).
    let glyph_count = run.glyphs.len();
    let mut adjustments: Vec<f32> = Vec::with_capacity(glyph_count.saturating_sub(1));
    for (i, &(gid, _)) in run.glyphs.iter().enumerate() {
        if i + 1 >= glyph_count {
            break;
        }
        let static_width_pt = metrics.advance_1000_for_gid(gid) / 1000.0 * run.size_pt;
        let target_pt = real.get(i).copied().unwrap_or(static_width_pt);
        let adjust_pt = static_width_pt - target_pt;
        adjustments.push((adjust_pt * 1000.0 / run.size_pt) as f32);
    }

    if adjustments.iter().all(|a| a.abs() < MIN_TJ_ADJUST_UNITS) {
        content.show(Str(&cid_bytes(&run.glyphs, orig_to_new)));
        return;
    }

    let mut positioned = content.show_positioned();
    let mut items = positioned.items();
    let mut segment: Vec<u8> = Vec::new();
    for (i, &(gid, _)) in run.glyphs.iter().enumerate() {
        let cid = orig_to_new.get(&gid).copied().unwrap_or(0);
        segment.extend_from_slice(&cid.to_be_bytes());

        if let Some(&adjust_units) = adjustments.get(i) {
            if adjust_units.abs() >= MIN_TJ_ADJUST_UNITS {
                items.show(Str(&segment));
                segment.clear();
                items.adjust(adjust_units);
            }
        }
    }
    if !segment.is_empty() {
        items.show(Str(&segment));
    }
}

fn write_info(pdf: &mut Pdf, info_id: Ref, meta: &PdfMeta) {
    let mut info = pdf.document_info(info_id);
    if let Some(title) = &meta.title {
        info.title(TextStr(title));
    }
    if let Some(producer) = &meta.producer {
        info.producer(TextStr(producer));
    }
    if let Some(date) = meta.creation_date {
        info.creation_date(date.to_pdf_writer_date());
    }
}

fn rgb_components(rgb: u32) -> (f32, f32, f32) {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    (r, g, b)
}

/// Decode `png_bytes` into a tightly packed `RGB8` buffer, dropping any
/// alpha channel (a full-page background raster is opaque by construction
/// — see this module's own doc comment) and cross-checking the result
/// against `expected_px`.
fn decode_opaque_rgb(png_bytes: &[u8], expected_px: (u32, u32)) -> Result<(Vec<u8>, u32, u32), ExportError> {
    let mut decoder = png::Decoder::new(png_bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| ExportError::RasterDecode(e.to_string()))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| ExportError::RasterDecode(e.to_string()))?;

    let actual = (info.width, info.height);
    if actual != expected_px {
        return Err(ExportError::RasterDimensionMismatch { expected: expected_px, actual });
    }

    let rgb = to_opaque_rgb8(&buf[..info.buffer_size()], info.color_type, info.width, info.height);
    Ok((rgb, info.width, info.height))
}

/// Normalize any of the (post-`normalize_to_color8`) color types `png`
/// 0.17 can still hand back into tightly packed `RGB8`, dropping alpha.
/// `Indexed` is unreachable in practice (`normalize_to_color8`'s own
/// `EXPAND` flag always expands a palette away before this function ever
/// sees it) — handled defensively (a black placeholder row) rather than
/// omitted, per this crate's own "never panic" rule.
fn to_opaque_rgb8(buf: &[u8], color_type: png::ColorType, width: u32, height: u32) -> Vec<u8> {
    let pixel_count = width as usize * height as usize;
    let mut out = Vec::with_capacity(pixel_count * 3);
    match color_type {
        png::ColorType::Rgb => out.extend_from_slice(&buf[..pixel_count * 3]),
        png::ColorType::Rgba => {
            for px in buf.chunks_exact(4).take(pixel_count) {
                out.extend_from_slice(&px[..3]);
            }
        }
        png::ColorType::Grayscale => {
            for &g in buf.iter().take(pixel_count) {
                out.extend_from_slice(&[g, g, g]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for px in buf.chunks_exact(2).take(pixel_count) {
                out.extend_from_slice(&[px[0], px[0], px[0]]);
            }
        }
        png::ColorType::Indexed => out.resize(pixel_count * 3, 0),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROBOTO_REGULAR: &[u8] = include_bytes!("../../../uzor-fonts/fonts/Roboto-Regular.ttf");

    fn one_pixel_rgba_png(rgba: [u8; 4]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("write PNG header");
            writer.write_image_data(&rgba).expect("write PNG pixel");
        }
        buf
    }

    /// A solid, flat-color square — deliberately highly compressible
    /// synthetic raster content (the report-page backgrounds this hybrid
    /// model paints are typically flat/text-heavy, per the export SOTA
    /// research's own size analysis), used by the Flate-compression size
    /// test below.
    fn solid_rgba_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("write PNG header");
            let pixel_count = (width * height) as usize;
            let mut data = Vec::with_capacity(pixel_count * 4);
            for _ in 0..pixel_count {
                data.extend_from_slice(&rgba);
            }
            writer.write_image_data(&data).expect("write PNG pixels");
        }
        buf
    }

    fn find_stream_with_key<'a>(doc: &'a lopdf::Document, key: &[u8]) -> Option<&'a lopdf::Stream> {
        doc.objects.values().find_map(|obj| match obj {
            lopdf::Object::Stream(stream) if stream.dict.has(key) => Some(stream),
            _ => None,
        })
    }

    #[test]
    fn finished_pdf_starts_with_the_pdf_magic_bytes() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Hello PDF" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        assert!(bytes.starts_with(b"%PDF-"), "output must start with the PDF header");
    }

    #[test]
    fn one_page_one_run_pdf_contains_a_page_object_and_a_type0_font_object() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Hello PDF" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Type /Page"), "must contain a page object");
        assert!(text.contains("/Subtype /Type0"), "font must be a composite Type0 font");
        assert!(text.contains("/CIDFontType2"), "the descendant font must be a CIDFontType2 (TrueType outlines)");
        assert!(text.contains("/Encoding /Identity-H"), "the Type0 font must use Identity-H encoding");
        assert!(!text.contains("WinAnsiEncoding"), "the old simple-font WinAnsi path must be fully gone");
    }

    #[test]
    fn a_raster_page_contains_a_flate_compressed_image_xobject() {
        let mut builder = PdfBuilder::new();
        let png_bytes = one_pixel_rgba_png([255, 0, 0, 255]);
        builder
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (1, 1), text_runs: Vec::new(), links: Vec::new(), content: PdfContentStream::empty() })
            .expect("add_page with a raster should succeed");

        let bytes = builder.finish();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Image"), "must contain an image XObject");
        assert!(text.contains("/DeviceRGB"), "the raster must embed as DeviceRGB");
        assert!(text.contains("/FlateDecode"), "the raster must be Flate-compressed");
    }

    #[test]
    fn raster_dimension_mismatch_is_rejected_not_silently_trusted() {
        let mut builder = PdfBuilder::new();
        let png_bytes = one_pixel_rgba_png([0, 255, 0, 255]);
        let err = builder
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (2, 2), text_runs: Vec::new(), links: Vec::new(), content: PdfContentStream::empty() })
            .expect_err("a wrong raster_px must be rejected");
        assert!(matches!(err, ExportError::RasterDimensionMismatch { expected: (2, 2), actual: (1, 1) }));
    }

    #[test]
    fn an_unmapped_character_falls_back_to_notdef_rather_than_a_question_mark() {
        // Post-Type0-migration behavior replaces the old
        // `unencodable_characters_become_a_visible_question_mark` WinAnsi
        // test: a codepoint this font has no glyph for resolves to glyph
        // id 0 (`.notdef`) — a private-use-area codepoint no real font
        // maps to anything is used here as ground truth.
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "A\u{F8FF}B" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        // Must not panic, and must still produce a well-formed PDF.
        let bytes = builder.finish();
        assert!(bytes.starts_with(b"%PDF-"));
        lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output even with a .notdef glyph present");
    }

    /// Cyrillic — the SOTA research pass's own #1-ranked, "do-now,
    /// CRITICAL" item: a Russian-language run must render AND
    /// round-trip through text extraction verbatim, not render as `'?'`.
    #[test]
    fn cyrillic_text_round_trips_through_lopdf_extraction() {
        let cyrillic = "Отчёт о переводах";
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 300.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 14.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: cyrillic }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");
        let pages = doc.get_pages();
        let page_numbers: Vec<u32> = pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert_eq!(extracted.trim(), cyrillic, "Cyrillic text must round-trip verbatim via ToUnicode, got {extracted:?}");

        // Structural proof: the content stream really uses `Tj` (real
        // vector text operators), decompressing cleanly (Flate) in the
        // process.
        let page_id = *pages.values().next().expect("one page");
        let content = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("Tj"), "content stream must contain a Tj text-showing operator, got {content_str:?}");
    }

    /// Typography-gap WAVE 2 (per-glyph PDF kerning): a run with
    /// `glyph_advances_pt: None` (the pre-WAVE-2, every-figure/chrome-call
    /// default) must still emit the plain, byte-identical `Tj` — never a
    /// `TJ` array — even though this test's own gate cares about the
    /// SIBLING test below actually engaging `TJ`.
    #[test]
    fn a_run_with_no_real_advances_still_emits_a_plain_tj() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 24.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Kerning" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        assert!(content_str.contains("Tj"), "no real advances -> plain Tj, got {content_str:?}");
        assert!(!content_str.contains("TJ"), "no real advances must never produce a TJ array, got {content_str:?}");
    }

    /// Typography-gap WAVE 2 (per-glyph PDF kerning): a run whose
    /// `glyph_advances_pt` genuinely differ from the embedded font's own
    /// static declared widths must emit a real `TJ` array with at least
    /// one non-zero adjustment number, AND every glyph must still be
    /// individually extractable (position-adjusted `Tj`/`TJ` text is still
    /// real, searchable text — `/ToUnicode` doesn't care how the glyphs
    /// were positioned).
    #[test]
    fn a_run_whose_real_advances_differ_from_static_widths_emits_a_tj_array_with_adjustments() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        let text = "Kerning";
        let size_pt = 24.0;
        // Deliberately, wildly different from Roboto's own static glyph
        // widths at 24pt (every glyph advances a flat 40pt) — guarantees
        // at least one adjustment clears `MIN_TJ_ADJUST_UNITS`.
        let fake_advances: Vec<f64> = vec![40.0; text.chars().count()];
        builder
            .add_page(PdfPageSpec {
                width_pt: 400.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun {
                    font,
                    size_pt,
                    x_pt: 10.0,
                    y_pt: 20.0,
                    rgb: 0x111111,
                    glyph_advances_pt: Some(fake_advances),
                    text,
                }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        assert!(content_str.contains("TJ"), "wildly different real advances must produce a real TJ array, got {content_str:?}");

        let page_numbers: Vec<u32> = pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains(text), "TJ-positioned text must still extract verbatim via ToUnicode, got {extracted:?}");
    }

    /// A run whose `glyph_advances_pt` happen to already match the
    /// embedded font's own static widths (within `MIN_TJ_ADJUST_UNITS`)
    /// still degrades to a plain `Tj` — `Some(..)` alone never forces a
    /// `TJ` array; only a genuinely significant per-glyph difference does.
    #[test]
    fn a_run_whose_real_advances_match_static_widths_still_emits_a_plain_tj() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        let text = "Hi";
        let size_pt = 12.0;

        // Resolve each char's REAL static width the same way this crate's
        // own `ttf::TtfMetrics` would, so the supplied advances are
        // (within floating-point noise) identical to what `show_run`
        // would have used anyway.
        let metrics = ttf::TtfMetrics::parse(ROBOTO_REGULAR);
        let matching_advances: Vec<f64> =
            text.chars().map(|ch| metrics.advance_1000_for_gid(metrics.gid_for_char(ch).unwrap_or(0)) / 1000.0 * size_pt).collect();

        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun {
                    font,
                    size_pt,
                    x_pt: 10.0,
                    y_pt: 20.0,
                    rgb: 0x111111,
                    glyph_advances_pt: Some(matching_advances),
                    text,
                }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        assert!(!content_str.contains("TJ"), "advances matching the static widths must never force a TJ array, got {content_str:?}");
    }

    /// Subsetting: the embedded font stream must be smaller than the full
    /// TTF, and extraction must still work (via `/ToUnicode`) against the
    /// SUBSET font's own remapped CIDs.
    #[test]
    fn embedded_font_subset_is_smaller_than_the_full_ttf_and_still_extracts() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Hi" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let font_file = find_stream_with_key(&doc, b"Length1").expect("a subset FontFile2 stream must be present");
        let subset_decompressed = font_file.decompressed_content().expect("FontFile2 must decompress cleanly");
        assert!(
            subset_decompressed.len() < ROBOTO_REGULAR.len(),
            "a 2-glyph subset ({} bytes decompressed) must be far smaller than the full font ({} bytes)",
            subset_decompressed.len(),
            ROBOTO_REGULAR.len()
        );
        // The RAW (still Flate-compressed) stream, as physically stored,
        // must be smaller too — proves the size win survives both stages.
        assert!(font_file.content.len() < ROBOTO_REGULAR.len());

        let pages = doc.get_pages();
        let page_numbers: Vec<u32> = pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("Hi"), "ToUnicode must recover the original text verbatim against the SUBSET font, got {extracted:?}");
    }

    /// Flate: a raster page's image XObject must compress at least 5x
    /// smaller than the raw, uncompressed RGB8 equivalent.
    #[test]
    fn raster_image_xobject_is_flate_compressed_at_least_5x_smaller_than_raw_rgb() {
        let (width, height) = (64u32, 64u32);
        let png_bytes = solid_rgba_png(width, height, [30, 60, 90, 255]);

        let mut builder = PdfBuilder::new();
        builder
            .add_page(PdfPageSpec { width_pt: width as f64, height_pt: height as f64, raster: Some(&png_bytes), raster_px: (width, height), text_runs: Vec::new(), links: Vec::new(), content: PdfContentStream::empty() })
            .expect("add_page with a raster should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let image_stream = find_stream_with_key(&doc, b"Width").expect("the raster page must embed exactly one Image XObject stream");
        let filter = image_stream.dict.get(b"Filter").and_then(|f| f.as_name()).expect("the image XObject must declare a /Filter");
        assert_eq!(filter, b"FlateDecode");

        let raw_rgb_len = width as usize * height as usize * 3;
        let compressed_len = image_stream.content.len();
        assert!(
            raw_rgb_len as f64 / compressed_len as f64 >= 5.0,
            "expected >=5x reduction on a flat-color raster (raw {raw_rgb_len} bytes vs compressed {compressed_len} bytes)"
        );
    }

    /// `/Info` metadata: additive, opt-in, and never present unless the
    /// caller calls [`PdfBuilder::set_meta`].
    #[test]
    fn info_dict_is_absent_unless_meta_is_set_then_present_with_the_given_fields() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 100.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "x" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");
        let bytes_without_meta = builder.finish();
        assert!(!String::from_utf8_lossy(&bytes_without_meta).contains("/Producer"), "no /Info dict must be written when set_meta was never called");

        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 100.0,
                height_pt: 100.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "x" }],
                links: Vec::new(),
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");
        builder.set_meta(PdfMeta {
            title: Some("Case Report".to_owned()),
            producer: Some("uzor-export".to_owned()),
            creation_date: Some(PdfDate { year: 2026, month: 7, day: 17, hour: 12, minute: 0, second: 0 }),
        });
        let bytes_with_meta = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes_with_meta).expect("lopdf must parse this crate's own PDF output");
        let info_dict = doc.trailer.get(b"Info").ok().and_then(|obj| doc.get_dictionary(obj.as_reference().ok()?).ok()).expect("trailer must reference an /Info dict");
        assert_eq!(info_dict.get(b"Title").ok().and_then(|o| o.as_str().ok()), Some(b"Case Report".as_slice()));
        assert_eq!(info_dict.get(b"Producer").ok().and_then(|o| o.as_str().ok()), Some(b"uzor-export".as_slice()));
        assert!(info_dict.has(b"CreationDate"));
    }

    /// `lopdf` (dev-dep) actually PARSES this crate's own output — the gate
    /// this phase's brief asks for, beyond the byte-substring smoke checks
    /// above: real page count, and a non-empty font resource dict per page.
    #[test]
    fn lopdf_parses_page_count_and_a_non_empty_fonts_dict_per_page() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        for i in 0..2 {
            builder
                .add_page(PdfPageSpec {
                    width_pt: 200.0,
                    height_pt: 100.0,
                    raster: None,
                    raster_px: (0, 0),
                    text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Hello lopdf" }],
                    links: Vec::new(),
                content: PdfContentStream::empty(),
                })
                .unwrap_or_else(|e| panic!("add_page {i} should succeed: {e}"));
        }

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2, "lopdf must see exactly the 2 pages this builder added");

        for (_, page_id) in &pages {
            let fonts = doc.get_page_fonts(*page_id).expect("get_page_fonts must succeed");
            assert!(!fonts.is_empty(), "every page's own font resource dict must be non-empty");
        }
    }

    /// Same lopdf ground truth for the raster half: a page with a raster
    /// background carries a real `/XObject` resource entry lopdf itself can
    /// see (not just a byte-substring match on `/Subtype /Image`).
    #[test]
    fn lopdf_sees_the_image_xobject_in_a_raster_pages_own_resources() {
        let mut builder = PdfBuilder::new();
        let png_bytes = one_pixel_rgba_png([10, 20, 30, 255]);
        builder
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (1, 1), text_runs: Vec::new(), links: Vec::new(), content: PdfContentStream::empty() })
            .expect("add_page with a raster should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let pages = doc.get_pages();
        assert_eq!(pages.len(), 1);
        let page_id = *pages.values().next().expect("one page");

        let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources must succeed");
        let resources = resources.expect("this page's Resources dict is written inline, not indirect");
        let x_objects = resources.get(b"XObject").and_then(lopdf::Object::as_dict).expect("XObject dict must be present");
        assert!(!x_objects.is_empty(), "the raster page's XObject dict must carry the embedded image");
    }

    /// Decode a PDF text-string's raw bytes (as returned by
    /// `lopdf::Object::as_str()`) per the SAME convention `pdf_writer`'s
    /// own `TextStr` writes: bare ASCII, or a `U+FEFF` byte-order-mark
    /// followed by UTF-16BE code units. Test-only — production code never
    /// needs to DECODE a `TextStr` it just wrote.
    fn decode_pdf_text_string(bytes: &[u8]) -> String {
        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        }
    }

    /// Document-navigation feature pass: the `/Outlines` tree has the
    /// right STRUCTURE (task gate: "assert via lopdf: /Outlines count,
    /// nested child, Cyrillic title extracts") — two top-level entries,
    /// the second carrying one nested (level-2) child, and a Cyrillic
    /// title round-trips through `/Title` verbatim (proves `TextStr`'s
    /// own UTF-16BE-with-BOM path, not just ASCII).
    #[test]
    fn outline_tree_has_the_right_structure_and_a_cyrillic_title_round_trips() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        for _ in 0..2 {
            builder
                .add_page(PdfPageSpec {
                    width_pt: 200.0,
                    height_pt: 300.0,
                    raster: None,
                    raster_px: (0, 0),
                    text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "Page" }],
                    links: Vec::new(),
                content: PdfContentStream::empty(),
                })
                .expect("add_page should succeed");
        }
        builder.set_outline(vec![
            PdfOutlineEntry { level: 1, title: "Introduction".to_owned(), page_index: 0 },
            PdfOutlineEntry { level: 1, title: "Отчёт".to_owned(), page_index: 1 },
            PdfOutlineEntry { level: 2, title: "Отчёт — подраздел".to_owned(), page_index: 1 },
        ]);

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let catalog = doc.catalog().expect("catalog must be present");
        let outlines_ref = catalog.get(b"Outlines").and_then(lopdf::Object::as_reference).expect("catalog must reference an /Outlines dict");
        let outlines = doc.get_dictionary(outlines_ref).expect("must resolve the /Outlines dict");
        assert_eq!(outlines.get(b"Type").and_then(|o| o.as_name()).expect("/Type must be present"), b"Outlines".as_slice());

        let first_ref = outlines.get(b"First").and_then(lopdf::Object::as_reference).expect("/Outlines must have a /First top-level item");
        let first_item = doc.get_dictionary(first_ref).expect("must resolve the first top-level item");
        assert_eq!(decode_pdf_text_string(first_item.get(b"Title").and_then(|o| o.as_str()).expect("first item must have a /Title")), "Introduction");
        assert!(first_item.get(b"First").is_err(), "the Introduction item has no children");

        let second_ref = first_item.get(b"Next").and_then(lopdf::Object::as_reference).expect("the first item must link to a second top-level sibling via /Next");
        let second_item = doc.get_dictionary(second_ref).expect("must resolve the second top-level item");
        assert_eq!(
            decode_pdf_text_string(second_item.get(b"Title").and_then(|o| o.as_str()).expect("second item must have a /Title")),
            "Отчёт",
            "a Cyrillic outline title must round-trip verbatim through /Title"
        );
        assert!(second_item.get(b"Next").is_err(), "there must be exactly 2 top-level items");

        // The second item's own nested child (level 2).
        let child_ref = second_item.get(b"First").and_then(lopdf::Object::as_reference).expect("the second item must have a nested child");
        let child_item = doc.get_dictionary(child_ref).expect("must resolve the nested child item");
        assert_eq!(
            decode_pdf_text_string(child_item.get(b"Title").and_then(|o| o.as_str()).expect("child item must have a /Title")),
            "Отчёт — подраздел"
        );
        assert_eq!(child_item.get(b"Parent").and_then(lopdf::Object::as_reference).expect("child must reference its own /Parent"), second_ref);

        // Level-1 items must be OPEN by default (positive /Count on the
        // parent that has children); the level-2 child itself is a leaf.
        let second_count = second_item.get(b"Count").and_then(lopdf::Object::as_i64).expect("an item with children must declare /Count");
        assert!(second_count > 0, "a level-1 item with children must default OPEN (positive /Count), got {second_count}");

        // The root /Outlines dict's own /Count sums every initially-
        // visible entry: 2 top-level items + 1 open child = 3.
        assert_eq!(outlines.get(b"Count").and_then(lopdf::Object::as_i64).expect("/Count must be present"), 3);
    }

    /// Document-navigation feature pass: a link annotation is present on
    /// the correct page, with a real `/GoTo` `/Dest` resolving to the
    /// intended TARGET page object (task gate: "link annots present with
    /// correct /Dest page refs").
    #[test]
    fn link_annotation_is_present_with_a_goto_dest_to_the_correct_target_page() {
        let mut builder = PdfBuilder::new();
        let font = builder.register_font(ROBOTO_REGULAR);
        builder
            .add_page(PdfPageSpec {
                width_pt: 200.0,
                height_pt: 300.0,
                raster: None,
                raster_px: (0, 0),
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, glyph_advances_pt: None, text: "See page 2" }],
                links: vec![PdfLink { x_pt: 10.0, y_pt: 20.0, width_pt: 80.0, height_pt: 16.0, target_page: 1 }],
                content: PdfContentStream::empty(),
            })
            .expect("add_page should succeed");
        builder
            .add_page(PdfPageSpec { width_pt: 200.0, height_pt: 300.0, raster: None, raster_px: (0, 0), text_runs: Vec::new(), links: Vec::new(), content: PdfContentStream::empty() })
            .expect("add_page should succeed");

        let bytes = builder.finish();
        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this crate's own PDF output");

        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let page1_id = *pages.get(&1).expect("page 1 must exist");
        let page2_id = *pages.get(&2).expect("page 2 must exist");

        let page1 = doc.get_dictionary(page1_id).expect("must resolve page 1");
        let annots = page1.get(b"Annots").and_then(lopdf::Object::as_array).expect("page 1 must carry an /Annots array");
        assert_eq!(annots.len(), 1, "page 1 must carry exactly the one link this test added");

        let annot_ref = annots[0].as_reference().expect("annotation must be an indirect reference");
        let annot = doc.get_dictionary(annot_ref).expect("must resolve the annotation dict");
        assert_eq!(annot.get(b"Subtype").and_then(|o| o.as_name()).expect("/Subtype must be present"), b"Link".as_slice());

        let action = annot.get(b"A").and_then(lopdf::Object::as_dict).expect("a Link annotation must carry an /A action dict");
        assert_eq!(action.get(b"S").and_then(|o| o.as_name()).expect("/S must be present"), b"GoTo".as_slice());

        let dest = action.get(b"D").and_then(lopdf::Object::as_array).expect("a GoTo action must carry a /D destination array");
        let dest_page_ref = dest[0].as_reference().expect("the destination's first item must be the target page reference");
        assert_eq!(dest_page_ref, page2_id, "the link's own GoTo destination must resolve to the TARGET page object (page 2), not page 1 or any other page");
    }
}
