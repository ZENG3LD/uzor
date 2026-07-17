//! Neutral PDF assembly — Arc 4 Phase P5 (`nemo/docs/uzor-engines/
//! uzor_typeset_arc4_design.md` §6). This module defines an
//! **engine-agnostic** content model + builder for a hybrid-fidelity PDF
//! (real, selectable/searchable vector text + one full-page raster
//! background per page); it holds no `uzor-text`/`uzor-figures`/
//! `uzor-typeset` knowledge whatsoever (design law 9 / §6.1's dependency-
//! boundary constraint) — the adapter that knows `Page`/`ParagraphLayout`
//! lives in `uzor-typeset::export` instead, and calls exactly the four
//! types below.
//!
//! ## Hybrid fidelity model
//!
//! Every page is (up to) two layers, painted back-to-front:
//! 1. **One full-page raster background** (`PdfPageSpec::raster`) — every
//!    non-text block (figures, tables, images) plus a caller-suppressed
//!    paragraph-ink pass (the adapter's job, see its own module docs),
//!    embedded as ONE opaque `DeviceRGB` Image XObject. Opaque by
//!    construction (a full-page background always covers every pixel), so
//!    no alpha/SMask is embedded — decoding drops any alpha channel a
//!    caller's PNG happens to carry (see [`decode_opaque_rgb`]).
//! 2. **Real vector text runs** (`PdfPageSpec::text_runs`) painted on top,
//!    via the embedded font's own glyph outlines — selectable, searchable,
//!    small (this is the whole point of the hybrid model, design doc §6.2).
//!
//! ## Coordinate convention
//!
//! Every coordinate this module's own public types take (`PdfTextRun::
//! {x_pt,y_pt}`, `PdfPageSpec::{width_pt,height_pt}`) is **top-left
//! origin, y growing downward** — the SAME convention every other
//! `Rect`/pixel coordinate in this workspace uses (`uzor::types::Rect`,
//! `uzor-typeset`'s own `Frame`/`PlacedBlock` rects). PDF's own coordinate
//! system is bottom-left origin, y growing upward; [`PdfBuilder`] converts
//! **internally** (`pdf_y = page_height_pt - y_pt`) so neither this
//! module's callers nor its own public API ever have to think in PDF-space
//! coordinates — see `write_page`'s own doc comment for exactly where that
//! conversion happens.
//!
//! ## Font embedding: full font, WinAnsiEncoding simple font, no `subsetter`
//!
//! The design doc's §6.3 names `subsetter` (the same author/lineage as
//! `pdf-writer`, Typst's own choice) as the companion crate for font
//! subsetting, with an explicit escape hatch: **"if `subsetter`'s API is a
//! poor fit, FULL-font embedding is an acceptable v1 fallback (bigger
//! files)."** That escape hatch is what got built, for a concrete,
//! load-bearing reason, not a preference: `subsetter::subset` takes
//! ALREADY-KNOWN glyph ids (its own doctest hardcodes `&[68, 69, 70]`) — it
//! has no `cmap`/`hmtx` reader of its own, so USING it at all would have
//! required a real font-parsing crate first, and this phase's dependency
//! law scopes new `uzor-export` dependencies to `pdf-writer` (+ `subsetter`
//! if used) only — no `ttf-parser` et al. This module therefore:
//! - embeds the **whole, unmodified** TTF byte buffer a caller hands
//!   [`PdfBuilder::register_font`] as `/FontFile2` (bigger files, as the
//!   doc's own fallback text warns — acceptable for the `nemo/uzor/out/`
//!   proof deliverables this phase targets);
//! - writes it as a PDF **simple** font (`/Subtype /TrueType`, NOT a
//!   Type0/CID font — a CID font is the shape `subsetter`'s own docs
//!   assume, and buys nothing once subsetting itself is off the table)
//!   under the **predefined** `/Encoding /WinAnsiEncoding` name ([`winansi`]
//!   — Latin-1 plus the `0x80..=0x9F` cp1252 punctuation block; non-Latin1
//!   characters, e.g. CJK/emoji, encode as `'?'`, a documented v1 scope
//!   narrowing — this crate's own report/deck consumers are Latin-script
//!   business text);
//! - resolves that encoding's own real per-glyph advance widths (the
//!   `/Widths` array a PDF viewer uses for `Tj` cursor advance) by
//!   HAND-PARSING the embedded font's own `cmap`/`hmtx`/`head` tables
//!   ([`ttf`]) — genuinely exact (same bytes that get embedded), not an
//!   approximation, and the reason this phase needed real font metrics at
//!   all rather than reusing this crate's own shaper output: [`PdfTextRun`]
//!   is deliberately NEUTRAL (a plain text string + one origin, no
//!   per-glyph pixel positions — `uzor-typeset`'s own `ParagraphLayout`
//!   never crosses this dependency boundary), so this is the ONLY place
//!   glyph-width information could come from.
//!
//! No `/ToUnicode` CMap is written — deliberately out of scope, not a
//! silent gap: `WinAnsiEncoding` is a PDF-predefined name every conformant
//! reader (Acrobat, most viewers, and `lopdf`'s own [`lopdf::Document::
//! extract_text`](https://docs.rs/lopdf) used by this crate's own tests)
//! already knows how to decode WITHOUT a `/ToUnicode` hint; adding one
//! would be pure belt-and-suspenders for a name-based encoding, not a
//! correctness requirement here.

mod ttf;
mod winansi;

use pdf_writer::types::FontFlags;
use pdf_writer::{Content, Name, Pdf, Rect as PdfRect, Ref, Str};

use crate::ExportError;

/// Opaque handle to a font registered via [`PdfBuilder::register_font`].
/// Deliberately a lightweight `Copy` id, not a borrowed `&PdfFont` — a
/// [`PdfBuilder`] must stay mutably reachable for [`PdfBuilder::add_page`]
/// after fonts were registered earlier, which a borrow into the SAME
/// builder's own font registry would make impossible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(u32);

/// A font registered with a [`PdfBuilder`] — its handle plus the (full,
/// unsubsetted — see this module's own doc comment) TTF bytes that will be
/// embedded verbatim as this font's `/FontFile2`.
pub struct PdfFont {
    pub id: FontId,
    pub ttf_bytes: Vec<u8>,
}

/// One run of same-font, same-color text painted starting at a single
/// baseline origin `(x_pt, y_pt)` (top-left page coordinates — see this
/// module's own doc comment). `text` is encoded via [`winansi::encode`]
/// when the page is added; unencodable characters become `'?'`.
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
    bytes: Vec<u8>,
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
/// `pdf-writer` until `finish()` — every font's `/Widths` array spans the
/// SAME fixed `WinAnsiEncoding` byte range (32..=255) regardless of which
/// bytes a given document actually uses (simpler than tracking per-font
/// usage, and harmless: an unused byte code's width is simply never
/// queried by a real PDF viewer), so no page-order dependency exists
/// either.
pub struct PdfBuilder {
    fonts: Vec<FontEntry>,
    pages: Vec<PageRecord>,
}

impl Default for PdfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self { fonts: Vec::new(), pages: Vec::new() }
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

    /// Add one page. Decodes `spec.raster` (if present) into a tightly
    /// packed, alpha-dropped `RGB8` buffer up front and WinAnsi-encodes
    /// every text run's own bytes up front too — `finish()` therefore does
    /// no fallible work at all, every error this builder can produce
    /// surfaces here, at the call site closest to its actual cause.
    pub fn add_page(&mut self, spec: PdfPageSpec<'_>) -> Result<(), ExportError> {
        let raster_rgb = match spec.raster {
            Some(png_bytes) => Some(decode_opaque_rgb(png_bytes, spec.raster_px)?),
            None => None,
        };

        let runs = spec
            .text_runs
            .iter()
            .map(|run| PageTextRun {
                font_id: run.font,
                size_pt: run.size_pt,
                x_pt: run.x_pt,
                y_pt: run.y_pt,
                rgb: run.rgb,
                bytes: text_to_winansi_bytes(run.text),
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

        let font_refs: Vec<FontRefs> = self.fonts.iter().map(|_| FontRefs { font: refs.next(), descriptor: refs.next(), file: refs.next() }).collect();
        let page_refs: Vec<PageRefs> = self
            .pages
            .iter()
            .map(|page| PageRefs { page: refs.next(), content: refs.next(), image: page.raster_rgb.as_ref().map(|_| refs.next()) })
            .collect();

        let font_names: Vec<String> = (0..self.fonts.len()).map(|i| format!("F{i}")).collect();
        let base_font_names: Vec<String> = (0..self.fonts.len()).map(|i| format!("EmbeddedFont{i}")).collect();
        let image_names: Vec<String> = (0..self.pages.len()).map(|i| format!("Im{i}")).collect();

        pdf.catalog(catalog_id).pages(page_tree_id);
        pdf.pages(page_tree_id).kids(page_refs.iter().map(|r| r.page)).count(page_refs.len() as i32);

        for (entry, (refs, base_font_name)) in self.fonts.iter().zip(font_refs.iter().zip(base_font_names.iter())) {
            write_font(&mut pdf, refs, base_font_name, entry);
        }

        for (i, page) in self.pages.iter().enumerate() {
            write_page(&mut pdf, page_tree_id, &page_refs[i], &font_names, &font_refs, page, image_names[i].as_str());
        }

        pdf.finish()
    }
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
    font: Ref,
    descriptor: Ref,
    file: Ref,
}

struct PageRefs {
    page: Ref,
    content: Ref,
    image: Option<Ref>,
}

/// The fixed `WinAnsiEncoding` byte range this builder always writes a
/// `/Widths` entry for, regardless of which bytes a document actually
/// uses (see [`PdfBuilder`]'s own doc comment for why that's simpler and
/// harmless).
const FIRST_CHAR: u16 = 32;
const LAST_CHAR: u16 = 255;

fn write_font(pdf: &mut Pdf, refs: &FontRefs, base_font_name: &str, entry: &FontEntry) {
    let widths: Vec<f32> = (FIRST_CHAR..=LAST_CHAR).map(|b| entry.metrics.width_1000(b as u8) as f32).collect();
    let bbox = entry.metrics.bbox_1000();

    {
        let mut descriptor = pdf.font_descriptor(refs.descriptor);
        descriptor.name(Name(base_font_name.as_bytes()));
        descriptor.flags(FontFlags::NON_SYMBOLIC);
        descriptor.bbox(PdfRect::new(bbox[0] as f32, bbox[1] as f32, bbox[2] as f32, bbox[3] as f32));
        // Not parsed from the font's own `post` table (out of this
        // module's narrow scope, see its own doc comment) — `0.0` never
        // affects the embedded glyph outlines' actual rendered shape,
        // only cosmetic/heuristic PDF metadata.
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
        let mut file = pdf.stream(refs.file, &entry.font.ttf_bytes);
        file.pair(Name(b"Length1"), entry.font.ttf_bytes.len() as i32);
    }

    {
        // Hand-built, not `pdf.type1_font(..)` — that writer hardcodes
        // `/Subtype /Type1`; a TrueType simple font needs `/Subtype
        // /TrueType` instead (no dedicated writer for that subtype exists
        // in `pdf-writer` 0.15 — the generic `Dict` writer covers it
        // exactly, since a simple TrueType font dictionary's shape is
        // otherwise identical to Type1's).
        let mut dict = pdf.indirect(refs.font).dict();
        dict.pair(Name(b"Type"), Name(b"Font"));
        dict.pair(Name(b"Subtype"), Name(b"TrueType"));
        dict.pair(Name(b"BaseFont"), Name(base_font_name.as_bytes()));
        dict.pair(Name(b"FirstChar"), i32::from(FIRST_CHAR));
        dict.pair(Name(b"LastChar"), i32::from(LAST_CHAR));
        dict.insert(Name(b"Widths")).array().items(widths.iter().copied());
        dict.pair(Name(b"FontDescriptor"), refs.descriptor);
        dict.pair(Name(b"Encoding"), Name(b"WinAnsiEncoding"));
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
fn write_page(pdf: &mut Pdf, page_tree_id: Ref, refs: &PageRefs, font_names: &[String], font_refs: &[FontRefs], page: &PageRecord, image_name: &str) {
    if let (Some(image_ref), Some((rgb, width, height))) = (refs.image, page.raster_rgb.as_ref()) {
        let mut image = pdf.image_xobject(image_ref, rgb);
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
                content.show(Str(&run.bytes));
            }
            content.end_text();
        }
        pdf.stream(refs.content, &content.finish());
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
                fonts_dict.pair(Name(name.as_bytes()), refs.font);
            }
        }
        if let Some(image_ref) = refs.image {
            let mut x_objects = resources.x_objects();
            x_objects.pair(Name(image_name.as_bytes()), image_ref);
        }
    }
}

fn rgb_components(rgb: u32) -> (f32, f32, f32) {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    (r, g, b)
}

/// Encode `text` as `WinAnsiEncoding` bytes, substituting `'?'` for any
/// character outside that table (see [`winansi`]'s own doc comment) —
/// deliberate and visible, never a silently dropped character.
fn text_to_winansi_bytes(text: &str) -> Vec<u8> {
    text.chars().map(|ch| winansi::encode(ch).unwrap_or(b'?')).collect()
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
    fn one_page_one_run_pdf_contains_a_page_object_and_a_font_object() {
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
        assert!(text.contains("/Type /Font"), "must contain a font object");
        assert!(text.contains("/Subtype /TrueType"), "font must be a simple TrueType font");
    }

    #[test]
    fn a_raster_page_contains_an_image_xobject() {
        let mut builder = PdfBuilder::new();
        let png_bytes = one_pixel_rgba_png([255, 0, 0, 255]);
        builder
            .add_page(PdfPageSpec { width_pt: 50.0, height_pt: 50.0, raster: Some(&png_bytes), raster_px: (1, 1), text_runs: Vec::new() })
            .expect("add_page with a raster should succeed");

        let bytes = builder.finish();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Image"), "must contain an image XObject");
        assert!(text.contains("/DeviceRGB"), "the raster must embed as DeviceRGB");
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
    fn unencodable_characters_become_a_visible_question_mark() {
        assert_eq!(text_to_winansi_bytes("A\u{4e2d}B"), vec![b'A', b'?', b'B']);
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
