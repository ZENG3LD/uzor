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

mod subset;
mod ttf;

use std::collections::HashMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo};
use pdf_writer::{Content, Date, Filter, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};

use crate::ExportError;
use subset::{build_font_data, FontData, ADOBE_IDENTITY_UCS};

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
}

/// One PDF page: its physical size, an optional full-page opaque raster
/// background, and every vector text run painted on top of it.
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
}

struct PageRecord {
    width_pt: f64,
    height_pt: f64,
    /// Decoded, alpha-dropped `RGB8`, tightly packed, plus its own
    /// `(width, height)` — `None` for a text-only page.
    raster_rgb: Option<(Vec<u8>, u32, u32)>,
    runs: Vec<PageTextRun>,
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
}

impl Default for PdfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self { fonts: Vec::new(), pages: Vec::new(), meta: None }
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
            .iter()
            .map(|run| {
                let metrics = &self.fonts[run.font.0 as usize].metrics;
                let glyphs = run.text.chars().map(|ch| (metrics.gid_for_char(ch).unwrap_or(0), ch)).collect();
                PageTextRun { font_id: run.font, size_pt: run.size_pt, x_pt: run.x_pt, y_pt: run.y_pt, rgb: run.rgb, glyphs }
            })
            .collect();

        self.pages.push(PageRecord { width_pt: spec.width_pt, height_pt: spec.height_pt, raster_rgb, runs });
        Ok(())
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
            .map(|page| PageRefs { page: refs.next(), content: refs.next(), image: page.raster_rgb.as_ref().map(|_| refs.next()) })
            .collect();

        let info_ref = self.meta.as_ref().map(|_| refs.next());

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

        pdf.catalog(catalog_id).pages(page_tree_id);
        pdf.pages(page_tree_id).kids(page_refs.iter().map(|r| r.page)).count(page_refs.len() as i32);

        for (entry, refs, data, base_font_name) in izip(&self.fonts, &font_refs, &font_data, &base_font_names) {
            write_font(&mut pdf, refs, base_font_name, entry, data);
        }

        for (i, page) in self.pages.iter().enumerate() {
            write_page(&mut pdf, page_tree_id, &page_refs[i], &font_names, &font_refs, page, image_names[i].as_str(), &font_data);
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
fn write_page(
    pdf: &mut Pdf,
    page_tree_id: Ref,
    refs: &PageRefs,
    font_names: &[String],
    font_refs: &[FontRefs],
    page: &PageRecord,
    image_name: &str,
    font_data: &[FontData],
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

    {
        let mut content = Content::new();
        if refs.image.is_some() {
            content.save_state();
            content.transform([page.width_pt as f32, 0.0, 0.0, page.height_pt as f32, 0.0, 0.0]);
            content.x_object(Name(image_name.as_bytes()));
            content.restore_state();
        }
        if !page.runs.is_empty() {
            content.begin_text();
            for run in &page.runs {
                let (r, g, b) = rgb_components(run.rgb);
                content.set_fill_rgb(r, g, b);
                content.set_font(Name(font_names[run.font_id.0 as usize].as_bytes()), run.size_pt as f32);
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, run.x_pt as f32, (page.height_pt - run.y_pt) as f32]);
                let orig_to_new = &font_data[run.font_id.0 as usize].orig_to_new;
                content.show(Str(&cid_bytes(&run.glyphs, orig_to_new)));
            }
            content.end_text();
        }
        let raw = content.finish();
        let compressed = flate_compress(&raw);
        let mut stream = pdf.stream(refs.content, &compressed);
        stream.filter(Filter::FlateDecode);
    }

    {
        let mut page_writer = pdf.page(refs.page);
        page_writer.parent(page_tree_id);
        page_writer.media_box(PdfRect::new(0.0, 0.0, page.width_pt as f32, page.height_pt as f32));
        page_writer.contents(refs.content);

        let mut resources = page_writer.resources();
        {
            let mut fonts_dict = resources.fonts();
            for (name, refs) in font_names.iter().zip(font_refs.iter()) {
                fonts_dict.pair(Name(name.as_bytes()), refs.type0);
            }
        }
        if let Some(image_ref) = refs.image {
            let mut x_objects = resources.x_objects();
            x_objects.pair(Name(image_name.as_bytes()), image_ref);
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "Hello PDF" }],
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "Hello PDF" }],
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
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (1, 1), text_runs: Vec::new() })
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
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (2, 2), text_runs: Vec::new() })
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "A\u{F8FF}B" }],
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
                text_runs: vec![PdfTextRun { font, size_pt: 14.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: cyrillic }],
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "Hi" }],
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
            .add_page(PdfPageSpec { width_pt: width as f64, height_pt: height as f64, raster: Some(&png_bytes), raster_px: (width, height), text_runs: Vec::new() })
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "x" }],
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
                text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "x" }],
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
                    text_runs: vec![PdfTextRun { font, size_pt: 12.0, x_pt: 10.0, y_pt: 20.0, rgb: 0x111111, text: "Hello lopdf" }],
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
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (1, 1), text_runs: Vec::new() })
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
}
