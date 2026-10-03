//! Theme/palette, page masters, AST -> scene composition, slicing, PDF +
//! PNG output, overflow report.
//!
//! Every scene value this module builds is leaked (`Box::leak`/
//! `Vec::leak`) to `'static` — the press is a one-shot batch CLI tool
//! (parse once, compose once, export once, exit), exactly the "acceptable
//! for this crate's own batch/CLI document generation usage pattern, not
//! for an unbounded hot loop inside one long-running process" trade-off
//! `uzor-typeset`'s own `TocArena` doc comment already blesses for the
//! same reason — it sidesteps the genuine self-referential-arena
//! difficulty of tying a dynamic number of owned strings/collections to
//! one shared lifetime, without reaching for `unsafe`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use uzor::fonts::FontFamily;
use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec, PdfMeta};
use uzor_text::{
    layout_paragraph, layout_text, paragraph_intrinsic_size, BreakStrategy, CosmicShaper, FontSpec, Hyphenation, InlineBox, InlineBoxSlot,
    LineShaper, Paragraph, ParagraphAlign, StyledRun,
};
use uzor_typeset::{
    draw_page, pages_to_pdf, renumber_pages, slice_pages, AnchoredIsland, Block as SceneBlock, BlockId, BlockNode, BlockSizing,
    BrandTokens, BreakControl, CellPadding, ColorRole, ColumnSpec, ComponentStyle, ComposeStyle, DesignTokens, FigureBlock,
    FigureThemeTokens, FontFileRef, FontRole, Frame, ImageBlock, ImageFit, IslandAnchor, ListBlock, ListItem, MarkerStyle, Page,
    PageMaster, PageNumberStyle, PdfExportOptions, PlacedBlock, Region, TableBlock, TableCell, TableRow, TextStyle, Theme,
};

use crate::figures::{
    self, BandCover, CalloutBox, Closing, Cover, CoverImage, Diagram, FooterMark, Heading, KpiRow, KpiTile, LogoAsset,
    InsetBackdrop, PageMark, HEADING_RULE_GAP, HEADING_RULE_H, HEADING_TRAILING_GAP,
};
use crate::preset::{Preset, Sheet};
use crate::parse::{Block as AstBlock, ColumnAlign, CoverLike, Document, FrontMatter, ImageSpec, InlineRun, InsetKind, InsetPart, Section};

/// Extended palette (`SPEC.md`'s own "Themes" table) — `uzor_typeset
/// ::Theme`'s `BrandTokens` only names 4 chrome roles (`ink`/`muted`/
/// `accent`/`background`); the custom figures in `figures.rs` need the
/// rest (`panel`/`second`/`line`/`up`/`down`), so this crate owns its own
/// wider palette rather than stretching `BrandTokens` past its own
/// documented 4-role shape.
#[derive(Debug, Clone)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub ink: u32,
    pub muted: u32,
    pub accent: u32,
    pub second: u32,
    pub line: u32,
    pub up: u32,
    pub down: u32,
    /// Extra data-series colors beyond the 5 chrome-derived ones
    /// (`accent`/`second`/`up`/`down`/`muted`) `build_theme` always
    /// appends to `BrandTokens::categorical_palette`. Empty unless a
    /// theme file lists more.
    pub categorical_extra: Vec<u32>,
    /// Display face — headings, body paragraphs, lists, callouts, and the
    /// cover/closing figures.
    pub font_display: FontFamily,
    /// Data/label face — tables, KPI figures, diagram boxes, and the
    /// cover badge.
    pub font_mono: FontFamily,
    /// Print-document extras (table paint, note box, page mark, cover kind,
    /// DOCX face). `DocStyle::default()` reproduces the original look.
    pub doc: DocStyle,
    /// File name that selects [`LogoAsset::Mark`] instead of a PNG path.
    pub logo_sentinel: Option<String>,
    /// Stroke mark drawn when the logo file name equals [`Self::logo_sentinel`].
    pub logo_mark: Option<StrokeMark>,
}

/// Straight-line mark: a square view box, a stroke width in view-box units,
/// and one polyline per stroked path. `wordmark`, when set, is drawn beside
/// the mark on the cover.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeMark {
    pub viewbox: f64,
    pub stroke_width: f64,
    pub polylines: Vec<Vec<(f64, f64)>>,
    pub wordmark: Option<String>,
}

/// Gridline paint for tables: colour and stroke width in pt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleStyle {
    pub color: u32,
    pub width: f64,
}

/// How the `>` note paragraph is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalloutStyle {
    /// The original look: a bordered box (PDF: ink border, no fill; DOCX:
    /// accent border with the `panel` fill).
    Boxed,
    /// A light fill with a bar on the left edge, in both PDF and DOCX.
    Tinted { fill: u32, bar: u32, bar_width: f64 },
}

/// The small label line above a `>>` / `>>>` block.
#[derive(Debug, Clone, PartialEq)]
pub struct InsetLabel {
    pub text: String,
    /// Bold, in the bar colour; size in pt.
    pub size: f64,
}

/// How one kind of `>>` / `>>>` block is drawn. Every field left at its
/// default takes the colours from the palette, so a theme that sets nothing
/// still gets a block of its own hue.
#[derive(Debug, Clone, PartialEq)]
pub struct InsetStyle {
    /// Fill behind the block. `None`: the bar colour mixed into the page
    /// background (a tint).
    pub fill: Option<u32>,
    /// Bar colour. `None`: the palette's `up` colour for revised text, its
    /// `second` colour for a technical addition.
    pub bar: Option<u32>,
    pub bar_width: f64,
    /// `None`: no label line.
    pub label: Option<InsetLabel>,
}

impl Default for InsetStyle {
    fn default() -> Self {
        Self { fill: None, bar: None, bar_width: 2.0, label: None }
    }
}

/// An [`InsetStyle`] with every colour resolved against its palette.
#[derive(Debug, Clone, PartialEq)]
pub struct InsetPaint {
    pub fill: u32,
    pub bar: u32,
    pub bar_width: f64,
    pub label: Option<InsetLabel>,
}

/// Type sizes and exact line heights (pt) of the text of a `>>` / `>>>`
/// block. Revised text is set like the body it replaces; a technical
/// addition at the callout size, its line scaled from the body line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InsetSizes {
    pub text: f64,
    pub text_leading: f64,
    pub item: f64,
    /// `None`: the sheet sets no exact line for list items.
    pub item_leading: Option<f64>,
}

/// How far the fill and the bar of a block reach past the text column: the
/// text keeps the body measure (so it wraps like the original it replaces)
/// and the block hangs into the margins, like a change bar.
pub const INSET_HANG_LEFT: f64 = 8.0;
/// Fill past the right edge of the text column.
pub const INSET_HANG_RIGHT: f64 = 6.0;
/// Fill above the first line and below the last line of a block, per page.
pub const INSET_PAD_V: f64 = 2.0;

/// Which cover a `--format doc|report` document gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoverStyle {
    /// The `:::cover` block, when the file has one.
    Standard,
    /// A front-matter-driven band cover: logo top-left, a filled
    /// parallelogram of colour `shape` whose top edge shifts by `slant`
    /// of its width, title/subtitle, author lines, date.
    Band { shape: u32, slant: f64 },
}

/// Opt-in print-document paint that `Palette` does not carry per colour.
/// Every field defaults to "off".
#[derive(Debug, Clone, PartialEq)]
pub struct DocStyle {
    /// Font name written into the DOCX runs instead of the bundled family's
    /// name (the PDF keeps the bundled face). `None`: the family's own name.
    pub docx_font: Option<String>,
    /// Table gridline colour and width. `None`: PDF ink at 1 pt, DOCX
    /// `line` at 0.5 pt.
    pub table_rule: Option<RuleStyle>,
    /// Table header-row fill. `None`: no fill in the PDF, `panel` in DOCX.
    pub table_head_fill: Option<u32>,
    /// Fill (and bold) for rows whose first non-empty cell starts with one
    /// of [`Self::total_prefixes`]. `None`: rows are not inspected.
    pub table_total_fill: Option<u32>,
    /// Case-insensitive prefixes that mark a totals row. Empty: no row matches.
    pub total_prefixes: Vec<String>,
    pub callout: CalloutStyle,
    /// `Some(colour)`: the folio reads `N ▰ M` with the parallelogram in
    /// this colour, instead of the bare / `n of N` engine number.
    pub page_mark: Option<u32>,
    pub cover: CoverStyle,
    /// Draw the front-matter `logo:` at the left of the footer line.
    pub footer_logo: bool,
    /// Family that sets the arrow characters U+2190..U+2199 when the text
    /// face lacks them (Roboto has none: they would print as boxes).
    /// `None`: arrows stay in the text face.
    pub symbol_face: Option<FontFamily>,
    /// The `>>` revised-text block and the `>>>` technical addition. The
    /// defaults derive everything from the palette and print no label.
    pub revised: InsetStyle,
    pub technical: InsetStyle,
}

impl Default for DocStyle {
    fn default() -> Self {
        Self {
            docx_font: None,
            table_rule: None,
            table_head_fill: None,
            table_total_fill: None,
            total_prefixes: Vec::new(),
            callout: CalloutStyle::Boxed,
            page_mark: None,
            cover: CoverStyle::Standard,
            footer_logo: true,
            symbol_face: None,
            revised: InsetStyle::default(),
            technical: InsetStyle::default(),
        }
    }
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            bg: 0x0a0f1a,
            panel: 0x131722,
            ink: 0xfeffee,
            muted: 0xa3adc2,
            accent: 0xf4cd63,
            second: 0x2158a4,
            line: 0x243047,
            up: 0x26a69a,
            down: 0xef5350,
            categorical_extra: Vec::new(),
            font_display: FontFamily::Roboto,
            font_mono: FontFamily::Roboto,
            doc: DocStyle::default(),
            logo_sentinel: None,
            logo_mark: None,
        }
    }

    pub fn light() -> Self {
        Self {
            bg: 0xffffff,
            panel: 0xf3f5f9,
            ink: 0x0a0f1a,
            muted: 0x4a5568,
            accent: 0x2158a4,
            second: 0xf4cd63,
            line: 0xd5dae3,
            up: 0x26a69a,
            down: 0xef5350,
            categorical_extra: Vec::new(),
            font_display: FontFamily::Roboto,
            font_mono: FontFamily::Roboto,
            doc: DocStyle::default(),
            logo_sentinel: None,
            logo_mark: None,
        }
    }

    /// The colours of a `>>` / `>>>` block in this theme: the style's own
    /// where it sets them, else the palette's `up` colour (revised text) or
    /// `second` colour (technical addition) as the bar and a tint of it over
    /// the background as the fill (a stronger tint on a dark background,
    /// where a light one would vanish).
    pub fn inset_paint(&self, kind: InsetKind) -> InsetPaint {
        let (style, default_bar) = match kind {
            InsetKind::Revised => (&self.doc.revised, self.up),
            InsetKind::Technical => (&self.doc.technical, self.second),
        };
        let bar = style.bar.unwrap_or(default_bar);
        let fill = style.fill.unwrap_or_else(|| {
            let dark = luminance(self.bg) < 0.5;
            mix(self.bg, bar, if dark { 0.22 } else { 0.10 })
        });
        InsetPaint { fill, bar, bar_width: style.bar_width, label: style.label.clone() }
    }

    /// `0xRRGGBB` -> `"#rrggbb"`.
    pub fn hex(color: u32) -> String {
        format!("#{:06x}", color & 0xff_ffff)
    }

    /// `0xRRGGBB` -> `0xRRGGBBFF`, the packed `uzor_text::StyledRun::color`
    /// shape (fully opaque).
    fn packed(color: u32) -> u32 {
        ((color & 0xff_ffff) << 8) | 0xff
    }
}

/// Per-role font sizes (px) — `SPEC.md`'s own "Sizes" table.
#[derive(Debug, Clone, Copy)]
pub struct Sizes {
    pub h1: f64,
    pub h2: f64,
    pub h3: f64,
    pub body: f64,
    pub list_item: f64,
    pub table_header: f64,
    pub table_cell: f64,
    pub caption: f64,
    pub footer: f64,
    /// Text size inside a `>` note.
    pub callout: f64,
    pub kpi_value: f64,
    pub kpi_label: f64,
    pub cover_title: f64,
    pub cover_sub: f64,
    pub cover_meta: f64,
    pub diagram_title: f64,
    pub diagram_subtitle: f64,
}

/// `doc` (A4 portrait) vs `deck` (16:9) vs `report` (A4 landscape). Paper
/// size only. Margins and type scale live on the preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Doc,
    Deck,
    Report,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "doc" => Some(Format::Doc),
            "deck" => Some(Format::Deck),
            "report" => Some(Format::Report),
            _ => None,
        }
    }

    /// `pub(crate)` — the DOCX writer needs the exact same physical page
    /// dimensions for its own `<w:pgSz>` (`docx::section::sect_pr_xml`).
    pub(crate) fn page_size(self) -> (f64, f64) {
        match self {
            Format::Doc => (595.0, 842.0),
            Format::Deck => (960.0, 540.0),
            Format::Report => (842.0, 595.0),
        }
    }

    /// `pub(crate)` — `docx::body::image_xml` uses the same default when a
    /// non-island image omits `height=`.
    pub(crate) fn image_default_height(self) -> f64 {
        match self {
            Format::Doc | Format::Report => 240.0,
            Format::Deck => 300.0,
        }
    }

    fn diagram_row_height(self) -> f64 {
        match self {
            Format::Doc | Format::Report => 50.0,
            Format::Deck => 66.0,
        }
    }

    fn diagram_row_gap(self) -> f64 {
        match self {
            Format::Doc | Format::Report => 24.0,
            Format::Deck => 36.0,
        }
    }

    /// Cover-corner logo height (pt) — `SPEC.md`: "~120 pt tall for doc,
    /// ~90 pt for deck." `report` shares `doc`'s own value (a landscape
    /// document is still a "doc"-shaped page, not a slide). `pub(crate)` —
    /// `docx::body::cover_title_page_xml` sizes the same corner logo.
    pub(crate) fn logo_height(self) -> f64 {
        match self {
            Format::Doc | Format::Report => 120.0,
            Format::Deck => 90.0,
        }
    }
}

/// Everything that went wrong building the scene from a parsed
/// [`Document`] — distinct from [`crate::parse::ParseError`] (which is
/// about the source TEXT); this is about resolving what the text refers
/// to (an image file on disk).
#[derive(Debug)]
pub enum PressError {
    Image { path: PathBuf, message: String },
}

impl fmt::Display for PressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PressError::Image { path, message } => write!(f, "failed to decode image {}: {message}", path.display()),
        }
    }
}

impl std::error::Error for PressError {}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

fn leak_box<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}

fn build_theme(palette: &Palette, sizes: &Sizes) -> Theme {
    Theme {
        brand: BrandTokens {
            ink: palette.ink,
            muted: palette.muted,
            accent: palette.accent,
            background: palette.bg,
            categorical_palette: {
                let mut colors = vec![palette.accent, palette.second, palette.up, palette.down, palette.muted];
                colors.extend(palette.categorical_extra.iter().copied());
                colors
            },
            // `palette.font_display` for both slots (bold heading role and
            // regular body role).
            font_files: vec![FontFileRef::new(palette.font_display, true, false), FontFileRef::new(palette.font_display, false, false)],
        },
        design: DesignTokens { heading_font: 0, heading_size_px: sizes.h1, body_font: 1, body_size_px: sizes.body, caption_font: 1, caption_size_px: sizes.caption },
        components: ComponentStyle {
            figure_theme: FigureThemeTokens {
                background: ColorRole::Background,
                axis_color: ColorRole::Muted,
                grid_color: ColorRole::Muted,
                label_color: ColorRole::Ink,
                label_font: FontRole::Caption,
                label_size_px: sizes.caption,
            },
            table_header: TextStyle { font: FontRole::Heading, size_px: sizes.table_header, color: ColorRole::Ink },
        },
    }
}

#[derive(Clone, Copy)]
struct DecodedImage {
    rgba: &'static [u8],
    width: u32,
    height: u32,
}

/// Decode a PNG file to straight RGBA8 (design law from `SPEC.md`: "expand
/// palette/gray, add alpha; 8-bit"). The `png` crate's own transformations
/// normalize bit depth/palette/gray-with-tRNS, but the FINAL channel count
/// still depends on the source's own color type — the match below is the
/// "expand palette/gray, add alpha" step for whichever of the 4 remaining
/// shapes (`Grayscale`/`GrayscaleAlpha`/`Rgb`/`Rgba`) survives that.
fn decode_png_rgba8(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(bytes.as_slice());
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(frame.buffer_size());
    let (width, height) = (frame.width, frame.height);

    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|px| [px[0], px[1], px[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|px| [px[0], px[0], px[0], px[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("decoder produced an indexed frame after EXPAND — unexpected".to_owned()),
    };
    Ok((rgba, width, height))
}

/// One press run's own output: the fully composed, renumbered page list
/// (cover pages + body pages) plus every `WARN overflow` line (`deck`
/// only — always empty for `doc`).
pub struct PressOutput {
    pub pages: Vec<Page<'static>>,
    pub warnings: Vec<String>,
}

pub struct Press {
    format: Format,
    palette: Palette,
    preset: Preset,
    sizes: Sizes,
    theme: Theme,
    base_dir: PathBuf,
    body_width: f64,
    image_cache: RefCell<HashMap<PathBuf, DecodedImage>>,
}

impl Press {
    pub fn new(format: Format, palette: Palette, preset: Preset, base_dir: PathBuf) -> Self {
        let sheet = preset.sheet(format).expect("preset has a sheet for this format");
        let sizes = sheet.sizes;
        let theme = build_theme(&palette, &sizes);
        let (w, h) = format.page_size();
        let cols = Self::columns_for(format, sheet);
        let gap = if cols > 1 { sheet.column_gap.max(16.0) } else { 0.0 };
        let master = PageMaster::new(w, h, sheet.margins_for(w)).with_columns(cols, gap);
        let body_width = if cols > 1 { master.column_width() } else { master.body_rect().width };
        Self { format, palette, preset, sizes, theme, base_dir, body_width, image_cache: RefCell::new(HashMap::new()) }
    }

    pub(crate) fn sheet(&self) -> &Sheet {
        self.preset.sheet(self.format).expect("preset has a sheet for this format")
    }

    pub(crate) fn margins(&self) -> uzor_typeset::Margins {
        let (w, _) = self.format.page_size();
        self.sheet().margins_for(w)
    }

    fn columns_for(format: Format, sheet: &Sheet) -> usize {
        let n = sheet.columns.max(1);
        match format {
            Format::Report => n.max(2),
            Format::Deck => 1,
            Format::Doc => n,
        }
    }

    fn column_count(&self) -> usize {
        Self::columns_for(self.format, self.sheet())
    }

    pub(crate) fn text_family(&self) -> FontFamily {
        self.sheet().text_family.unwrap_or(self.palette.font_display)
    }

    /// Compose `document` into a fully sliced, renumbered page list, per
    /// `--format`.
    pub fn build(&self, document: &Document) -> Result<PressOutput, PressError> {
        match self.format {
            Format::Doc | Format::Report => self.build_doc(document),
            Format::Deck => self.build_deck(document),
        }
    }

    /// Render one page to a PNG proof at device pixel ratio 2 (`SPEC.md`'s
    /// own "Proof PNGs" section) — background from `ExportSpec`, then the
    /// context scaled by `dpr` before `draw_page`, mirroring
    /// `uzor-typeset`'s own dpr-aware proof-render convention.
    pub fn render_page_png(&self, page: &Page<'_>) -> Result<Vec<u8>, String> {
        const DPR: f64 = 2.0;
        let (w, h) = self.format.page_size();
        let bg = self.palette.bg;
        let spec = ExportSpec {
            width_px: (w * DPR).round() as u32,
            height_px: (h * DPR).round() as u32,
            dpr: DPR,
            background: Some([((bg >> 16) & 0xff) as u8, ((bg >> 8) & 0xff) as u8, (bg & 0xff) as u8, 255]),
        };
        let theme = &self.theme;
        render_to_png(&spec, |ctx| {
            ctx.scale(DPR, DPR);
            draw_page(ctx, page, theme);
        })
        .map_err(|e| e.to_string())
    }

    pub fn to_pdf(&self, output: &PressOutput, front_matter: &FrontMatter) -> Vec<u8> {
        let (w, h) = self.format.page_size();
        let master = PageMaster::new(w, h, self.margins());
        let meta = PdfMeta {
            title: Some(front_matter.title.clone()),
            author: front_matter.author.clone(),
            subject: front_matter.subject.clone(),
            producer: Some("mlc-pitch".to_owned()),
            ..Default::default()
        };
        let options = PdfExportOptions { tagged: true, lang: Some(front_matter.lang.clone()), meta: Some(meta) };
        pages_to_pdf(&output.pages, &master, &self.theme, options)
    }

    // ── accessors (shared with the `docx` writer) ───────────────────────

    pub(crate) fn palette(&self) -> &Palette {
        &self.palette
    }

    pub(crate) fn sizes(&self) -> &Sizes {
        &self.sizes
    }

    pub(crate) fn format(&self) -> Format {
        self.format
    }

    pub(crate) fn body_width(&self) -> f64 {
        self.body_width
    }

    /// Rasterize the SAME `Diagram` figure `push_diagram` (the PDF path)
    /// builds, at its own natural (width, height) — the shared helper
    /// [`Self::build_diagram_figure`] guarantees the two paths never
    /// disagree on the figure's own geometry.
    pub(crate) fn diagram_figure(&self, rows: &[Vec<crate::parse::DiagramBox>]) -> (Diagram, f64) {
        self.build_diagram_figure(rows)
    }

    // ── decoding ─────────────────────────────────────────────────────

    /// Resolve a content-file-relative or absolute path against
    /// [`Self::base_dir`] — the ONE path-resolution rule shared by
    /// [`Self::decode_image`] (decoded RGBA8, for the PDF path) and
    /// [`Self::read_image_bytes`] (original file bytes, for the DOCX path).
    fn resolve_path(&self, rel_or_abs: &Path) -> PathBuf {
        if rel_or_abs.is_absolute() {
            rel_or_abs.to_path_buf()
        } else {
            self.base_dir.join(rel_or_abs)
        }
    }

    fn decode_image(&self, rel_or_abs: &Path) -> Result<DecodedImage, PressError> {
        let resolved = self.resolve_path(rel_or_abs);
        if let Some(cached) = self.image_cache.borrow().get(&resolved) {
            return Ok(*cached);
        }
        let (rgba, width, height) =
            decode_png_rgba8(&resolved).map_err(|message| PressError::Image { path: resolved.clone(), message })?;
        let decoded = DecodedImage { rgba: leak_slice(rgba), width, height };
        self.image_cache.borrow_mut().insert(resolved, decoded);
        Ok(decoded)
    }

    /// Raw file bytes for a content-file-relative or absolute image
    /// path — the DOCX embedder needs the ORIGINAL PNG bytes (not the
    /// decoded RGBA8 buffer [`Self::decode_image`] produces for the PDF
    /// path), so Word receives the exact source file rather than a
    /// re-encode.
    pub(crate) fn read_image_bytes(&self, rel_or_abs: &Path) -> Result<Vec<u8>, PressError> {
        let resolved = self.resolve_path(rel_or_abs);
        std::fs::read(&resolved).map_err(|e| PressError::Image { path: resolved, message: e.to_string() })
    }

    /// Decoded pixel dimensions only — a thin wrapper over the existing
    /// [`Self::decode_image`] cache, no duplicate decode/caching logic.
    pub(crate) fn image_dimensions(&self, rel_or_abs: &Path) -> Result<(u32, u32), PressError> {
        let decoded = self.decode_image(rel_or_abs)?;
        Ok((decoded.width, decoded.height))
    }

    fn cover_image(&self, path: &Path) -> Result<CoverImage, PressError> {
        let d = self.decode_image(path)?;
        Ok(CoverImage { rgba: d.rgba, width: d.width, height: d.height })
    }

    // ── figures (cover / closing) ────────────────────────────────────

    fn build_cover_figure(&self, cover: &CoverLike, front_matter: &FrontMatter) -> Result<&'static Cover, PressError> {
        let image = cover.image.as_deref().map(|p| self.cover_image(p)).transpose()?;
        let logo = front_matter.logo.as_deref().map(|p| self.resolve_logo(p)).transpose()?;
        Ok(leak_box(Cover {
            title: cover.title.clone(),
            subtitle: cover.subtitle.clone(),
            meta: cover.meta.clone(),
            site: cover.site.clone(),
            image,
            logo,
            logo_height: self.format.logo_height(),
            badge: front_matter.badge.clone(),
            badge_font: self.font_mono_bold(self.sizes.footer),
            palette: self.palette.clone(),
            title_font: self.font_bold(self.sizes.cover_title),
            subtitle_font: self.font(self.sizes.cover_sub),
            meta_font: self.font(self.sizes.cover_meta),
            site_font: self.font(self.sizes.footer),
            wordmark_font: self.font_bold(self.sizes.cover_sub),
        }))
    }

    fn build_closing_figure(&self, closing: &CoverLike) -> Result<&'static Closing, PressError> {
        let image = closing.image.as_deref().map(|p| self.cover_image(p)).transpose()?;
        Ok(leak_box(Closing {
            title: closing.title.clone(),
            subtitle: closing.subtitle.clone(),
            site: closing.site.clone(),
            image,
            palette: self.palette.clone(),
            title_font: self.font_bold(self.sizes.h1),
            subtitle_font: self.font(self.sizes.cover_sub),
            site_font: self.font(self.sizes.footer),
        }))
    }

    /// Resolve a front-matter `logo:` value to a [`LogoAsset`]. A file name
    /// equal to the theme's logo sentinel selects the theme's stroke mark;
    /// any other value decodes as a PNG file path.
    fn resolve_logo(&self, raw: &Path) -> Result<LogoAsset, PressError> {
        if self.names_logo_sentinel(raw) {
            let mark = self.palette.logo_mark.clone().ok_or_else(|| PressError::Image {
                path: raw.to_path_buf(),
                message: "theme logo sentinel has no mark".to_owned(),
            })?;
            return Ok(LogoAsset::Mark {
                viewbox: mark.viewbox,
                stroke_width: mark.stroke_width,
                polylines: mark.polylines,
                wordmark: mark.wordmark,
            });
        }
        Ok(LogoAsset::Raster(self.cover_image(raw)?))
    }

    pub(crate) fn names_logo_sentinel(&self, raw: &Path) -> bool {
        let Some(sentinel) = self.palette.logo_sentinel.as_deref() else { return false };
        raw.file_name().and_then(|name| name.to_str()) == Some(sentinel)
    }

    // ── fonts ────────────────────────────────────────────────────────

    /// Display/UI font at `size`, regular weight — headings, body,
    /// lists, callouts, cover/closing text. Resolves through the active
    /// theme's own `Palette::font_display`.
    fn font(&self, size: f64) -> FontSpec {
        FontSpec::new(self.palette.font_display, size)
    }

    fn font_bold(&self, size: f64) -> FontSpec {
        FontSpec::new(self.palette.font_display, size).bold()
    }

    fn font_italic(&self, size: f64) -> FontSpec {
        FontSpec::new(self.palette.font_display, size).italic()
    }

    /// Data/label font at `size`, regular weight — KPI figures, diagram
    /// boxes, the cover badge. Resolves through the active theme's own
    /// `Palette::font_mono` (tables go through [`Self::inline_runs`]'s
    /// own explicit `family` parameter instead, since that helper is
    /// shared with display-font callers too).
    fn font_mono(&self, size: f64) -> FontSpec {
        FontSpec::new(self.palette.font_mono, size)
    }

    fn font_mono_bold(&self, size: f64) -> FontSpec {
        FontSpec::new(self.palette.font_mono, size).bold()
    }

    // ── inline runs ──────────────────────────────────────────────────

    /// `Plain` runs at `size`/regular, `Bold` runs at `size`/bold — the
    /// literal `**bold**` -> bold-`StyledRun` mapping `SPEC.md` asks for.
    /// `family` lets a caller choose the display font (body/heading/list/
    /// callout content) or the mono font (table content) explicitly —
    /// this helper has no opinion of its own on which theme role applies.
    fn inline_runs(&self, runs: &[InlineRun], size: f64, family: FontFamily) -> &'static [StyledRun<'static>] {
        self.inline_runs_forced(runs, size, family, false)
    }

    /// Same as [`Self::inline_runs`], but every run renders bold
    /// regardless of its own `**...**` marking — table headers
    /// (`ComponentStyle.table_header`) don't need `**` to already read
    /// bold.
    fn inline_runs_forced(&self, runs: &[InlineRun], size: f64, family: FontFamily, force_bold: bool) -> &'static [StyledRun<'static>] {
        let regular = FontSpec::new(family, size);
        let bold = FontSpec::new(family, size).bold();
        let symbol = self.palette.doc.symbol_face.map(|face| FontSpec::new(face, size));
        let mut out: Vec<StyledRun<'static>> = Vec::with_capacity(runs.len());
        for run in runs {
            let (text, is_bold) = match run {
                InlineRun::Plain(t) => (t, false),
                InlineRun::Bold(t) => (t, true),
            };
            let spec = if force_bold || is_bold { bold } else { regular };
            match symbol {
                Some(symbol) if text.chars().any(is_arrow) => {
                    for (segment, arrow) in split_arrows(text) {
                        out.push(StyledRun::new(leak_str(segment.to_owned()), if arrow { symbol } else { spec }));
                    }
                }
                _ => out.push(StyledRun::new(leak_str(text.clone()), spec)),
            }
        }
        leak_slice(out)
    }

    fn plain_text(runs: &[InlineRun]) -> String {
        runs.iter().map(|r| match r { InlineRun::Plain(s) | InlineRun::Bold(s) => s.as_str() }).collect()
    }

    // ── masters ──────────────────────────────────────────────────────

    fn cover_master(&self) -> PageMaster<'static> {
        let (w, h) = self.format.page_size();
        PageMaster::new(w, h, self.margins())
    }

    /// The rest-of-document master: footer text (from the front matter's
    /// own `footer:` key) on the left, `n / total` on the right — the
    /// cover section alone uses [`Self::cover_master`] instead.
    ///
    /// `logo: None` (every pre-existing content file, which never sets
    /// the front matter's own `logo:` key) takes the EXACT pre-existing
    /// plain-paragraph footer path, byte-identical to before this
    /// feature — the small brand mark only replaces it when a logo is
    /// actually configured.
    fn resolved_logo(&self, explicit: Option<&Path>) -> Result<Option<LogoAsset>, PressError> {
        match explicit {
            Some(raw) => Ok(Some(self.resolve_logo(raw)?)),
            None => Ok(None),
        }
    }

    fn footer_master(&self, footer_text: &'static str, logo: Option<LogoAsset>, label: Option<&str>, page_number_style: PageNumberStyle) -> PageMaster<'static> {
        let (w, h) = self.format.page_size();
        let cols = self.column_count();
        let gap = if cols > 1 { self.sheet().column_gap.max(16.0) } else { 0.0 };
        let mut master = PageMaster::new(w, h, self.margins()).with_columns(cols, gap).with_footer_pin(self.sheet().footer_from_edge);
        if let Some(label) = label {
            let run: &'static [StyledRun<'static>] = leak_slice(vec![StyledRun::new(leak_str(label.to_owned()), self.font_mono(self.sizes.footer)).with_color(Palette::packed(self.palette.accent))]);
            let para = Paragraph::new(run, f64::MAX).with_align(ParagraphAlign::Right);
            let header: &'static [BlockNode<'static>] = leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(para))]);
            master = master.with_header(header);
        }
        let footer_flow: &'static [BlockNode<'static>] = match logo {
            None => {
                let footer_run: &'static [StyledRun<'static>] =
                    leak_slice(vec![StyledRun::new(footer_text, self.font(self.sizes.footer)).with_color(Palette::packed(self.palette.muted))]);
                leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(Paragraph::new(footer_run, f64::MAX)))])
            }
            Some(logo) => {
                let figure = leak_box(FooterMark { logo, text: footer_text.to_owned(), font: self.font(self.sizes.footer), palette: self.palette.clone() });
                leak_slice(vec![BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FillRegion)))])
            }
        };
        master.with_footer(footer_flow).with_page_number(page_number_style)
    }

    // ── block -> scene ───────────────────────────────────────────────

    /// Push the scene node(s) for one AST block onto `out`. `tag_outline`
    /// enables PDF-bookmark tagging on H1s (doc format only).
    fn push_block(&self, out: &mut Vec<BlockNode<'static>>, block: &AstBlock, tag_outline: bool) -> Result<(), PressError> {
        match block {
            AstBlock::Heading { level, runs } => self.push_heading(out, *level, runs, tag_outline),
            AstBlock::Paragraph(runs) => self.push_paragraph(out, runs),
            AstBlock::List { ordered, items } => self.push_list(out, *ordered, items),
            AstBlock::Callout(runs) => self.push_callout(out, runs),
            AstBlock::Inset { kind, parts } => self.push_inset(out, *kind, parts),
            AstBlock::Table { header, rows, fractions, aligns } => self.push_table(out, header, rows, fractions, aligns),
            AstBlock::Image(spec) => self.push_image(out, spec)?,
            AstBlock::Kpi(items) => self.push_kpi(out, items),
            AstBlock::Diagram(rows) => self.push_diagram(out, rows),
            AstBlock::Closing(cover) => {
                let figure = self.build_closing_figure(cover)?;
                out.push(BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FillRegion))));
            }
            AstBlock::Spacer(n) => out.push(BlockNode::new(SceneBlock::Spacer(*n))),
            AstBlock::Clear => out.push(BlockNode::new(SceneBlock::IslandEnd)),
        }
        Ok(())
    }

    /// H1 is ONE atomic `Heading` figure (text + its own accent rule),
    /// never a separate heading-paragraph + rule-figure pair — see
    /// [`figures::Heading`]'s own doc comment for the keep-with-next
    /// orphan this fixes (`BreakControl::AvoidAfter` only peeks at the
    /// SINGLE next flow node; a trivially-fitting standalone rule node
    /// used to satisfy that peek and strand the heading+rule pair at a
    /// page bottom while the real next paragraph didn't fit). H2/H3 stay
    /// plain paragraphs (no rule, so no analogous orphan risk).
    fn push_heading(&self, out: &mut Vec<BlockNode<'static>>, level: u8, runs: &[InlineRun], tag_outline: bool) {
        let sheet = self.sheet();
        let space = sheet.extras.heading_space[usize::from(level.clamp(1, 3) - 1)];
        let starts_page = level == 1 && sheet.extras.h1_page_break;
        // Space above the heading is a spacer, except where the heading opens
        // a page anyway (a spacer there would only push it down).
        if space.before > 0.0 && !starts_page && !out.is_empty() {
            out.push(BlockNode::new(SceneBlock::Spacer(space.before)));
        }
        let control = if starts_page { BreakControl::ForceBefore } else { BreakControl::AvoidAfter };

        if level == 1 && !sheet.h1_rule {
            let styled = self.inline_runs_forced(runs, self.sizes.h1, self.palette.font_display, true);
            let paragraph = Paragraph::new(styled, self.body_width).with_line_height(sheet.body_leading);
            let mut node = BlockNode::new(SceneBlock::Paragraph(paragraph)).with_break_control(control);
            if tag_outline {
                node = node.with_outline(1, Self::plain_text(runs));
            }
            out.push(node);
            // A page-opening heading has nothing to keep with, so the rest of
            // its space below (over the paragraph gap) can be a spacer.
            if starts_page && space.after > sheet.paragraph_spacing {
                out.push(BlockNode::new(SceneBlock::Spacer(space.after - sheet.paragraph_spacing)));
            }
            return;
        }
        if level == 1 {
            let shaper = CosmicShaper::headless();
            let styled = self.inline_runs_forced(runs, self.sizes.h1, self.palette.font_display, true);
            let layout = layout_paragraph(&Paragraph::new(styled, self.body_width), &shaper);
            let total_h = layout.height + HEADING_RULE_GAP + HEADING_RULE_H + HEADING_TRAILING_GAP;
            let figure = leak_box(Heading { layout, palette: self.palette.clone() });
            let mut node =
                BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(total_h)))).with_break_control(control);
            if tag_outline {
                node = node.with_outline(1, Self::plain_text(runs));
            }
            out.push(node);
            return;
        }

        let size = if level == 2 { self.sizes.h2 } else { self.sizes.h3 };
        let styled = self.inline_runs_forced(runs, size, self.palette.font_display, true);
        let mut paragraph = Paragraph::new(styled, self.body_width);
        if let Some(leading) = sheet.extras.heading_leading {
            paragraph = paragraph.with_line_height(leading);
        }
        out.push(BlockNode::new(SceneBlock::Paragraph(paragraph)).with_break_control(BreakControl::AvoidAfter));
    }

    fn push_paragraph(&self, out: &mut Vec<BlockNode<'static>>, runs: &[InlineRun]) {
        self.push_paragraph_at(out, runs, self.sizes.body, self.sheet().body_leading);
    }

    /// A body paragraph at `size` on an exact line of `leading`.
    fn push_paragraph_at(&self, out: &mut Vec<BlockNode<'static>>, runs: &[InlineRun], size: f64, leading: f64) {
        let styled = self.inline_runs(runs, size, self.text_family());
        let mut para = Paragraph::new(styled, self.body_width).with_line_height(leading);
        if self.sheet().justify {
            para = para.with_align(ParagraphAlign::Justify);
        } else {
            para = para.with_align(ParagraphAlign::Left);
        }
        if self.sheet().hyphenate || self.sheet().justify {
            para = para.with_break_strategy(BreakStrategy::KnuthPlass);
        }
        if self.sheet().hyphenate {
            para = para.with_hyphenation(Hyphenation::Russian);
        }
        if self.sheet().first_indent > 0.0 {
            let slots: &'static [InlineBoxSlot] =
                leak_slice(vec![InlineBoxSlot::new(0, 0, InlineBox::in_flow(0, self.sheet().first_indent, 0.0))]);
            para = para.with_inline_boxes(slots);
        }
        out.push(BlockNode::new(SceneBlock::Paragraph(para)));
    }

    fn push_list(&self, out: &mut Vec<BlockNode<'static>>, ordered: bool, items: &[Vec<InlineRun>]) {
        self.push_list_at(out, ordered, items, self.sizes.list_item, self.sheet().extras.list_leading);
    }

    /// A list whose items are set at `size`, on an exact line of `leading`
    /// when there is one.
    fn push_list_at(&self, out: &mut Vec<BlockNode<'static>>, ordered: bool, items: &[Vec<InlineRun>], size: f64, leading: Option<f64>) {
        let marker = if ordered { MarkerStyle::numbered(1) } else { MarkerStyle::Bullet('\u{2022}') };
        let sheet = self.sheet();
        let list_items: Vec<ListItem<'static>> = items
            .iter()
            .map(|item_runs| {
                let styled = self.inline_runs(item_runs, size, self.text_family());
                let mut para = Paragraph::new(styled, f64::MAX);
                if let Some(leading) = leading {
                    para = para.with_line_height(leading);
                }
                if sheet.extras.justify_lists {
                    if sheet.justify {
                        para = para.with_align(ParagraphAlign::Justify).with_break_strategy(BreakStrategy::KnuthPlass);
                    }
                    if sheet.hyphenate {
                        para = para.with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::Russian);
                    }
                }
                let nodes: &'static [BlockNode<'static>] = leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(para))]);
                ListItem::new(nodes)
            })
            .collect();
        let list_items = leak_slice(list_items);
        out.push(BlockNode::new(SceneBlock::List(ListBlock::new(list_items, marker, 20.0))));
    }

    /// `>` note. `CalloutStyle::Boxed` is a single-cell padded `TableBlock`
    /// (bordered in the theme ink, no fill: the engine table has neither an
    /// accent border nor a fill). `CalloutStyle::Tinted` is one figure that
    /// paints a light fill, a left bar and its own laid-out paragraph.
    fn push_callout(&self, out: &mut Vec<BlockNode<'static>>, runs: &[InlineRun]) {
        let styled = self.inline_runs(runs, self.sizes.callout, self.text_family());
        match self.palette.doc.callout {
            CalloutStyle::Boxed => {
                let nodes: &'static [BlockNode<'static>] =
                    leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(Paragraph::new(styled, f64::MAX)))]);
                let cells: &'static [TableCell<'static>] = leak_slice(vec![TableCell::new(nodes)]);
                let rows: &'static [TableRow<'static>] = leak_slice(vec![TableRow::new(cells)]);
                let columns: &'static [ColumnSpec] = leak_slice(vec![ColumnSpec::Fraction(1.0)]);
                let table = TableBlock::new(columns, rows).with_cell_padding(CellPadding::new(10.0, 12.0));
                out.push(BlockNode::new(SceneBlock::Table(table)));
            }
            CalloutStyle::Tinted { fill, bar, bar_width } => {
                const PAD_H: f64 = 8.0;
                const PAD_V: f64 = 6.0;
                let leading = self.sheet().body_leading * self.sizes.callout / self.sizes.body;
                let text_width = (self.body_width - bar_width - 2.0 * PAD_H).max(1.0);
                let layout = layout_paragraph(&Paragraph::new(styled, text_width).with_line_height(leading), &CosmicShaper::headless());
                let height = layout.height + 2.0 * PAD_V;
                let figure = leak_box(CalloutBox { layout, fill, bar, bar_width, pad_h: PAD_H, pad_v: PAD_V, ink: self.palette.ink });
                out.push(BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(height)))));
            }
        }
    }

    /// Type sizes and lines of a `>>` / `>>>` block's text. Revised text is
    /// set exactly like the body; a technical addition at the callout size,
    /// with the line scaled the way the `>` note scales it.
    pub(crate) fn inset_sizes(&self, kind: InsetKind) -> InsetSizes {
        let sheet = self.sheet();
        match kind {
            InsetKind::Revised => InsetSizes {
                text: self.sizes.body,
                text_leading: sheet.body_leading,
                item: self.sizes.list_item,
                item_leading: sheet.extras.list_leading,
            },
            InsetKind::Technical => InsetSizes {
                text: self.sizes.callout,
                text_leading: sheet.body_leading * self.sizes.callout / self.sizes.body,
                item: self.sizes.callout,
                item_leading: sheet.extras.list_leading.map(|leading| leading * self.sizes.callout / self.sizes.list_item),
            },
        }
    }

    /// `>>` / `>>>` block: the parts are ordinary paragraphs, lists and tables
    /// (revised text set like the body, a technical addition at the callout
    /// size), all tagged with one author id per block. The fill and the bar
    /// are not part of the flow: [`Self::decorate_insets`] paints them behind
    /// whatever part of the block lands on each page, so the block breaks
    /// wherever the engine breaks its paragraphs and still reads as one box.
    fn push_inset(&self, out: &mut Vec<BlockNode<'static>>, kind: InsetKind, parts: &[InsetPart]) {
        let paint = self.palette.inset_paint(kind);
        let sizes = self.inset_sizes(kind);
        let id = inset_id(kind, out.len());
        let before = out.len();
        if let Some(label) = paint.label.as_ref() {
            let run: &'static [StyledRun<'static>] =
                leak_slice(vec![StyledRun::new(leak_str(label.text.clone()), self.font_bold(label.size)).with_color(Palette::packed(paint.bar))]);
            let leading = self.sheet().body_leading * label.size / self.sizes.body;
            let para = Paragraph::new(run, self.body_width).with_line_height(leading).with_align(ParagraphAlign::Left);
            // The label never stands alone at the foot of a page.
            out.push(BlockNode::new(SceneBlock::Paragraph(para)).with_break_control(BreakControl::AvoidAfter));
        }
        for part in parts {
            match part {
                InsetPart::Paragraph(runs) => self.push_paragraph_at(out, runs, sizes.text, sizes.text_leading),
                InsetPart::List { ordered, items } => self.push_list_at(out, *ordered, items, sizes.item, sizes.item_leading),
                InsetPart::Table { header, rows, fractions, aligns } => self.push_table(out, header, rows, fractions, aligns),
            }
        }
        for node in &mut out[before..] {
            node.id = Some(id);
        }
    }

    /// Paint the fill and the bar of every `>>` / `>>>` block behind its
    /// placed blocks, page by page and column by column: one backdrop per run
    /// of consecutive placed blocks that share a block id, inserted ahead of
    /// the run so it is drawn first. A block that continues on the next page
    /// gets a backdrop there too.
    fn decorate_insets(&self, pages: &mut [Page<'static>]) {
        for page in pages.iter_mut() {
            backdrop_frame(&mut page.frame, &self.palette);
            for extra in &mut page.extra_frames {
                backdrop_frame(extra, &self.palette);
            }
        }
    }

    fn push_table(
        &self,
        out: &mut Vec<BlockNode<'static>>,
        header: &[Vec<InlineRun>],
        rows: &[Vec<Vec<InlineRun>>],
        fractions: &[f64],
        aligns: &[ColumnAlign],
    ) {
        let doc = &self.palette.doc;
        let columns: &'static [ColumnSpec] = leak_slice(fractions.iter().map(|f| ColumnSpec::Fraction(*f)).collect());
        let column_align = |i: usize| match aligns.get(i) {
            Some(ColumnAlign::Left) | None => ParagraphAlign::Left,
            Some(ColumnAlign::Center) => ParagraphAlign::Center,
            Some(ColumnAlign::Right) => ParagraphAlign::Right,
        };

        let header_cells: Vec<TableCell<'static>> = header
            .iter()
            .enumerate()
            .map(|(i, cell_runs)| {
                let styled = self.inline_runs_forced(cell_runs, self.sizes.table_header, self.palette.font_mono, true);
                let paragraph = Paragraph::new(styled, f64::MAX).with_align(column_align(i));
                let nodes: &'static [BlockNode<'static>] = leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(paragraph))]);
                TableCell::new(nodes)
            })
            .collect();
        let mut header_row = TableRow::new(leak_slice(header_cells));
        if let Some(fill) = doc.table_head_fill {
            header_row = header_row.with_fill(fill);
        }
        let mut all_rows: Vec<TableRow<'static>> = vec![header_row];
        for row in rows {
            let total_fill = doc.table_total_fill.filter(|_| is_total_row(row, &doc.total_prefixes));
            let cells: Vec<TableCell<'static>> = row
                .iter()
                .enumerate()
                .map(|(i, cell_runs)| {
                    let styled = self.inline_runs_forced(cell_runs, self.sizes.table_cell, self.palette.font_mono, total_fill.is_some());
                    let paragraph = Paragraph::new(styled, f64::MAX).with_align(column_align(i));
                    let nodes: &'static [BlockNode<'static>] = leak_slice(vec![BlockNode::new(SceneBlock::Paragraph(paragraph))]);
                    TableCell::new(nodes)
                })
                .collect();
            let mut table_row = TableRow::new(leak_slice(cells));
            if let Some(fill) = total_fill {
                table_row = table_row.with_fill(fill);
            }
            all_rows.push(table_row);
        }
        let all_rows = leak_slice(all_rows);
        let mut table = TableBlock::new(columns, all_rows)
            .with_cell_padding(self.sheet().cell_padding)
            .with_rules(self.sheet().table_rules)
            .with_header_repeat(true);
        if let Some(rule) = doc.table_rule {
            table = table.with_rule_style(rule.color, rule.width);
        }
        out.push(BlockNode::new(SceneBlock::Table(table)));
    }

    fn push_image(&self, out: &mut Vec<BlockNode<'static>>, spec: &ImageSpec) -> Result<(), PressError> {
        let decoded = self.decode_image(&spec.path)?;
        let height = spec.height.unwrap_or_else(|| self.format.image_default_height());
        let image_block = ImageBlock::new(decoded.rgba, decoded.width, decoded.height, BlockSizing::FixedHeight(height), ImageFit::Contain);

        if let Some(side) = spec.island {
            let width_fraction = spec.width.unwrap_or(0.45);
            let island_width = self.body_width * width_fraction;
            let anchor = match side {
                crate::parse::IslandSide::Left => IslandAnchor::Left,
                crate::parse::IslandSide::Right => IslandAnchor::Right,
            };
            let island = AnchoredIsland::new(image_block, anchor, island_width, 20.0);
            out.push(BlockNode::new(SceneBlock::Island(island)));
        } else {
            // A captioned picture keeps with its caption.
            let node = BlockNode::new(SceneBlock::Image(image_block));
            out.push(if spec.caption.is_some() { node.with_break_control(BreakControl::AvoidAfter) } else { node });
        }
        if let Some(caption) = &spec.caption {
            self.push_caption(out, caption);
        }
        Ok(())
    }

    fn push_caption(&self, out: &mut Vec<BlockNode<'static>>, caption: &str) {
        let f = self.font_italic(self.sizes.caption);
        let run: &'static [StyledRun<'static>] = leak_slice(vec![StyledRun::new(leak_str(caption.to_owned()), f).with_color(Palette::packed(self.palette.muted))]);
        let para = Paragraph::new(run, self.body_width).with_align(ParagraphAlign::Center);
        out.push(BlockNode::new(SceneBlock::Paragraph(para)));
    }

    /// `:::kpi` -> one [`KpiRow`] figure. Every tile's own value font is
    /// auto-fit to the tile's real inner width (measured against the SAME
    /// `body_width`/`KPI_GAP`/`KPI_PAD_X` geometry `KpiRow::render` itself
    /// uses, so the measurement here and the paint later never disagree).
    /// The value/label vertical SLOTS (baseline + label offset + figure
    /// height) are all sized from the NOMINAL (never-shrunk) value font's
    /// own line metrics, never a per-tile actual (possibly auto-fit-
    /// shrunk) height — so a shrunk value paints on the SAME baseline as
    /// its un-shrunk siblings instead of floating above the row, and the
    /// figure's own `BlockSizing::FixedHeight` is sized from the nominal
    /// value slot plus the TALLEST tile's own (possibly wrapped) label —
    /// never a fixed guess — so no label can ever overflow the tile it's
    /// painted into.
    fn push_kpi(&self, out: &mut Vec<BlockNode<'static>>, items: &[(String, String)]) {
        let shaper = CosmicShaper::headless();
        let n = items.len().max(1) as f64;
        let tile_w = ((self.body_width - figures::KPI_GAP * (n - 1.0)) / n).max(1.0);
        let inner_w = (tile_w - 2.0 * figures::KPI_PAD_X).max(1.0);

        let nominal_value_font = self.font_mono_bold(self.sizes.kpi_value);
        let nominal_layout = layout_text("0", &nominal_value_font, f64::MAX, &shaper);
        let nominal_value_h = nominal_layout.height;
        let nominal_baseline = nominal_layout.lines.first().map_or(nominal_value_h, |l| l.baseline_y);

        let mut tiles = Vec::with_capacity(items.len());
        let mut max_label_h = 0.0f64;
        for (value, label) in items {
            let value_font = self.fit_kpi_value_font(value, inner_w, &shaper);
            let label_font = self.font_mono(self.sizes.kpi_label);
            let label_h = layout_text(label, &label_font, inner_w, &shaper).height;
            max_label_h = max_label_h.max(label_h);
            tiles.push(KpiTile { value: value.clone(), value_font, label: label.clone(), label_font });
        }

        let label_y_offset = figures::KPI_TOP_PAD + nominal_value_h + figures::KPI_VALUE_LABEL_GAP;
        let total_h = label_y_offset + max_label_h + figures::KPI_BOTTOM_PAD + figures::KPI_EXTRA_BOTTOM;
        let figure = leak_box(KpiRow {
            tiles,
            palette: self.palette.clone(),
            value_baseline_y: figures::KPI_TOP_PAD + nominal_baseline,
            label_y_offset,
        });
        out.push(BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(total_h)))));
    }

    /// Shrink `value`'s own bold KPI-value font in 2px steps (floor 12px)
    /// until its unwrapped intrinsic width fits `max_width` — a value
    /// string is always shown whole, on one line, never wrapped.
    fn fit_kpi_value_font(&self, value: &str, max_width: f64, shaper: &dyn LineShaper) -> FontSpec {
        const FLOOR: f64 = 12.0;
        let mut size = self.sizes.kpi_value;
        loop {
            let candidate = self.font_mono_bold(size);
            let (w, _) = paragraph_intrinsic_size(value, &candidate, f64::MAX, shaper);
            if w <= max_width || size <= FLOOR {
                return candidate;
            }
            size = (size - 2.0).max(FLOOR);
        }
    }

    fn push_diagram(&self, out: &mut Vec<BlockNode<'static>>, rows: &[Vec<crate::parse::DiagramBox>]) {
        let (figure, total_height) = self.build_diagram_figure(rows);
        let figure = leak_box(figure);
        out.push(BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(total_height)))));
    }

    /// The `Diagram` figure construction + its own `total_height` formula,
    /// factored out so both [`Self::push_diagram`] (the PDF path, leaks the
    /// result to `'static`) and [`Self::diagram_figure`] (the DOCX path,
    /// rasterizes it immediately and never needs `'static`) build the
    /// EXACT same figure from the exact same formula.
    fn build_diagram_figure(&self, rows: &[Vec<crate::parse::DiagramBox>]) -> (Diagram, f64) {
        let row_h = self.format.diagram_row_height();
        let row_gap = self.format.diagram_row_gap();
        let n_rows = rows.len().max(1) as f64;
        // `+ 12.0`: a visible buffer below the last row, inside this
        // figure's own `BlockSizing::FixedHeight` — `Diagram::render`
        // itself never paints past the last row's own bottom edge, so no
        // corresponding change is needed there (the extra height is
        // simply left blank).
        let total_height = n_rows * row_h + (n_rows - 1.0).max(0.0) * row_gap + 12.0;
        let figure = Diagram {
            rows: rows.to_vec(),
            palette: self.palette.clone(),
            title_font: self.font_mono_bold(self.sizes.diagram_title),
            subtitle_font: self.font_mono(self.sizes.diagram_subtitle),
            row_h,
            row_gap,
            box_gap: 14.0,
        };
        (figure, total_height)
    }

    // ── document -> pages ────────────────────────────────────────────

    /// The cover section's own pages — empty when `cover` is `None` (a
    /// file that opens directly with an ordinary block instead of a
    /// `:::cover` fence), so the body then becomes page 1, composed with
    /// the ordinary footer master (byte-identical to today whenever a
    /// cover IS present).
    fn cover_pages(
        &self,
        cover: Option<&CoverLike>,
        front_matter: &FrontMatter,
        style: &ComposeStyle,
        shaper: &CosmicShaper,
    ) -> Result<Vec<Page<'static>>, PressError> {
        let Some(cover) = cover else { return Ok(Vec::new()) };
        let cover_figure = self.build_cover_figure(cover, front_matter)?;
        let cover_flow: &'static [BlockNode<'static>] =
            leak_slice(vec![BlockNode::new(SceneBlock::Figure(FigureBlock::new(cover_figure, BlockSizing::FillRegion)))]);
        Ok(slice_pages(cover_flow, &self.cover_master(), style, shaper))
    }

    /// Whether this press draws the front-matter-driven band cover
    /// (`CoverStyle::Band`, doc/report only).
    pub(crate) fn band_cover_active(&self) -> bool {
        matches!(self.palette.doc.cover, CoverStyle::Band { .. }) && self.format != Format::Deck
    }

    /// The band cover's text and geometry inputs, without the logo bitmap
    /// (the DOCX writer embeds the logo file itself). `None` unless
    /// [`Self::band_cover_active`]. An explicit `:::cover` block overrides
    /// the front-matter title and subtitle.
    pub(crate) fn band_cover_data(&self, document: &Document) -> Option<BandCover> {
        let CoverStyle::Band { shape, slant } = self.palette.doc.cover else { return None };
        if !self.band_cover_active() {
            return None;
        }
        let front = &document.front_matter;
        let block = document.cover.as_ref();
        let title = block.map(|c| c.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| front.title.clone());
        let subtitle = block.map(|c| c.subtitle.clone()).filter(|t| !t.is_empty()).or_else(|| front.subtitle.clone());
        let authors: Vec<String> = if front.authors.is_empty() {
            front
                .author
                .iter()
                .flat_map(|a| a.split(" / "))
                .map(|line| line.trim().to_owned())
                .filter(|line| !line.is_empty())
                .collect()
        } else {
            front.authors.clone()
        };
        let (page_width, page_height) = self.format.page_size();
        let margins = self.margins();
        Some(BandCover {
            title,
            subtitle,
            authors,
            date: front.date.clone(),
            logo: None,
            shape_color: shape,
            slant,
            palette: self.palette.clone(),
            page_width,
            page_height,
            margin_left: margins.left,
            margin_right: margins.right,
            margin_top: margins.top,
            margin_bottom: margins.bottom,
            regular_font: self.font(self.sizes.cover_meta),
            bold_font: self.font_bold(self.sizes.cover_title),
            title_leading: self.sheet().body_leading,
            authors_leading: self.sizes.cover_meta * 1.2 * 1.075,
        })
    }

    /// The band cover as one page, with the folio master the body pages use
    /// (so the cover carries the page mark too).
    fn band_cover_pages(&self, document: &Document, style: &ComposeStyle, shaper: &CosmicShaper) -> Result<Vec<Page<'static>>, PressError> {
        let Some(mut cover) = self.band_cover_data(document) else { return Ok(Vec::new()) };
        cover.logo = document.front_matter.logo.as_deref().map(|p| self.resolve_logo(p)).transpose()?;
        let figure = leak_box(cover);
        let flow: &'static [BlockNode<'static>] =
            leak_slice(vec![BlockNode::new(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FillRegion)))]);
        let master = self.footer_master("", None, None, self.sheet().page_number);
        Ok(slice_pages(flow, &master, style, shaper))
    }

    /// Replace the engine's plain page number by the `N ▰ M` figure, on every
    /// page that carries a number. The figure sits on the footer line
    /// `footer_from_edge` above the bottom edge, right-aligned to the text block.
    fn apply_page_marks(&self, pages: &mut [Page<'static>], mark: u32) {
        let (page_width, page_height) = self.format.page_size();
        let margins = self.margins();
        let sheet = self.sheet();
        let line_height = self.sizes.footer * 1.2;
        let rect = Rect::new(
            margins.left,
            page_height - sheet.footer_from_edge - line_height,
            (page_width - margins.left - margins.right).max(1.0),
            line_height,
        );
        let start = sheet.page_number.start;
        for page in pages.iter_mut() {
            let Some(number) = page.page_number.take() else { continue };
            if number.text.is_empty() {
                continue;
            }
            let slant = match self.palette.doc.cover {
                CoverStyle::Band { slant, .. } => slant,
                CoverStyle::Standard => 0.0,
            };
            let figure = leak_box(PageMark {
                current: (start + page.index).to_string(),
                last: (start + page.total.saturating_sub(1)).to_string(),
                font: self.font(self.sizes.footer),
                ink: self.palette.ink,
                mark,
                slant,
            });
            let kind = leak_box(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(rect.height))));
            let placed = PlacedBlock {
                id: BlockId::structural(u64::from(page.index)),
                rect,
                kind,
                paragraph_layout: None,
                table_placement: None,
                list_placement: None,
            };
            match &mut page.footer {
                Some(frame) => frame.blocks.push(placed),
                None => page.footer = Some(Frame { region: Region { rect }, blocks: vec![placed], overflow: None }),
            }
        }
    }

    fn build_doc(&self, document: &Document) -> Result<PressOutput, PressError> {
        let shaper = CosmicShaper::headless();
        let style = self.compose_style();
        let page_number_style = self.sheet().page_number;

        let cover_pages = if self.band_cover_active() {
            self.band_cover_pages(document, &style, &shaper)?
        } else {
            self.cover_pages(document.cover.as_ref(), &document.front_matter, &style, &shaper)?
        };

        let footer_text = leak_str(document.front_matter.footer.clone());
        let logo = if self.palette.doc.footer_logo { self.resolved_logo(document.front_matter.logo.as_deref())? } else { None };
        let label = document.front_matter.badge.as_deref();
        let body_master = self.footer_master(footer_text, logo, label, page_number_style);

        let mut flow: Vec<BlockNode<'static>> = Vec::new();
        for (section_idx, section) in document.sections.iter().enumerate() {
            for (block_idx, block) in section.blocks.iter().enumerate() {
                let before = flow.len();
                self.push_block(&mut flow, block, true)?;
                keep_with_following_figure(&mut flow, block, section.blocks.get(block_idx + 1));
                // `---` between two body sections forces a fresh page at
                // the start of every section after the first. A heading's
                // own space-above spacer would only push it down there.
                if section_idx > 0 && block_idx == 0 {
                    if matches!(block, AstBlock::Heading { .. }) && matches!(flow.get(before).map(|n| &n.kind), Some(SceneBlock::Spacer(_))) {
                        flow.remove(before);
                    }
                    if let Some(first) = flow.get_mut(before) {
                        first.break_control = BreakControl::ForceBefore;
                    }
                }
            }
        }
        let flow: &'static [BlockNode<'static>] = leak_slice(flow);
        let mut body_pages = slice_pages(flow, &body_master, &style, &shaper);
        self.decorate_insets(&mut body_pages);

        let mut all_pages = Vec::with_capacity(cover_pages.len() + body_pages.len());
        all_pages.extend(cover_pages);
        all_pages.extend(body_pages);
        let mut all_pages = renumber_pages(all_pages, Some(&page_number_style));
        if let Some(mark) = self.palette.doc.page_mark {
            self.apply_page_marks(&mut all_pages, mark);
        }

        Ok(PressOutput { pages: all_pages, warnings: Vec::new() })
    }

    fn compose_style(&self) -> ComposeStyle {
        let sheet = self.sheet();
        let mut style = ComposeStyle::from_theme(&self.theme, sheet.paragraph_spacing);
        if let Some(pitch) = sheet.baseline {
            style = style.with_baseline_grid(pitch);
        }
        style
    }

    fn build_deck(&self, document: &Document) -> Result<PressOutput, PressError> {
        let shaper = CosmicShaper::headless();
        let style = self.compose_style();
        let page_number_style = self.sheet().page_number;

        let cover_pages = self.cover_pages(document.cover.as_ref(), &document.front_matter, &style, &shaper)?;

        let footer_text = leak_str(document.front_matter.footer.clone());
        let logo = if self.palette.doc.footer_logo { self.resolved_logo(document.front_matter.logo.as_deref())? } else { None };
        let label = document.front_matter.badge.as_deref();

        let mut warnings = Vec::new();
        let mut all_pages: Vec<Page<'static>> = Vec::with_capacity(cover_pages.len());
        all_pages.extend(cover_pages);

        for section in &document.sections {
            let blocks = &section.blocks;
            let mut cuts = vec![0usize];
            for (i, block) in blocks.iter().enumerate().skip(1) {
                if let AstBlock::Heading { level, .. } = block {
                    if *level <= 2 {
                        cuts.push(i);
                    }
                }
            }
            cuts.push(blocks.len());
            for pair in cuts.windows(2) {
                let chunk = &blocks[pair[0]..pair[1]];
                if chunk.is_empty() {
                    continue;
                }
                let body_master = self.footer_master(footer_text, logo.clone(), label, page_number_style);
                let mut flow: Vec<BlockNode<'static>> = Vec::new();
                for (block_idx, block) in chunk.iter().enumerate() {
                    self.push_block(&mut flow, block, false)?;
                    keep_with_following_figure(&mut flow, block, chunk.get(block_idx + 1));
                }
                let flow: &'static [BlockNode<'static>] = leak_slice(flow);
                let mut pages = slice_pages(flow, &body_master, &style, &shaper);
                self.decorate_insets(&mut pages);
                if pages.len() > 1 {
                    warnings.push(format!("WARN overflow: slide \"{}\" produced {} pages", Self::section_title(section), pages.len()));
                }
                all_pages.extend(pages);
            }
        }

        let all_pages = renumber_pages(all_pages, Some(&page_number_style));
        Ok(PressOutput { pages: all_pages, warnings })
    }

    fn section_title(section: &Section) -> String {
        for block in &section.blocks {
            if let AstBlock::Heading { level: 1, runs } = block {
                return Self::plain_text(runs);
            }
        }
        for block in &section.blocks {
            if let AstBlock::Closing(cover) = block {
                if !cover.title.is_empty() {
                    return cover.title.clone();
                }
            }
        }
        "untitled".to_owned()
    }
}

/// High bits of a [`BlockId`] that mark the nodes of one `>>` (revised) or
/// `>>>` (technical) block. The engine's own structural ids use the top bit,
/// so the tags never meet them.
const REVISED_ID_TAG: u64 = 1 << 62;
const TECHNICAL_ID_TAG: u64 = 1 << 61;

/// The author id shared by every node of the block that starts at flow index
/// `index`.
fn inset_id(kind: InsetKind, index: usize) -> BlockId {
    let tag = match kind {
        InsetKind::Revised => REVISED_ID_TAG,
        InsetKind::Technical => TECHNICAL_ID_TAG,
    };
    BlockId(tag | index as u64)
}

/// Which kind of block a placed block belongs to, if any.
fn inset_kind(id: BlockId) -> Option<InsetKind> {
    if id.is_structural() {
        None
    } else if id.0 & REVISED_ID_TAG != 0 {
        Some(InsetKind::Revised)
    } else if id.0 & TECHNICAL_ID_TAG != 0 {
        Some(InsetKind::Technical)
    } else {
        None
    }
}

/// A picture right after a `>>>` block belongs to it: the block's last node
/// keeps with the next one (the picture), so the two land on one page. Only
/// a node with no keep/break choice of its own is touched.
fn keep_with_following_figure(flow: &mut [BlockNode<'static>], block: &AstBlock, next: Option<&AstBlock>) {
    if !matches!(block, AstBlock::Inset { kind: InsetKind::Technical, .. }) || !matches!(next, Some(AstBlock::Image(_))) {
        return;
    }
    if let Some(last) = flow.last_mut() {
        if matches!(last.break_control, BreakControl::Auto) {
            last.break_control = BreakControl::AvoidAfter;
        }
    }
}

/// Insert one [`InsetBackdrop`] ahead of every run of placed blocks that
/// belong to the same `>>` / `>>>` block. The backdrop spans the frame's
/// column plus the hang into the margins, and reaches [`INSET_PAD_V`] above
/// the first block and below the last.
fn backdrop_frame(frame: &mut Frame<'static>, palette: &Palette) {
    let mut runs: Vec<(usize, usize, InsetKind)> = Vec::new();
    let mut i = 0;
    while i < frame.blocks.len() {
        let id = frame.blocks[i].id;
        let Some(kind) = inset_kind(id) else {
            i += 1;
            continue;
        };
        let start = i;
        while i < frame.blocks.len() && frame.blocks[i].id == id {
            i += 1;
        }
        runs.push((start, i, kind));
    }

    let column = frame.region.rect;
    for &(start, end, kind) in runs.iter().rev() {
        let paint = palette.inset_paint(kind);
        let top = frame.blocks[start].rect.y - INSET_PAD_V;
        let bottom = frame.blocks[end - 1].rect.y + frame.blocks[end - 1].rect.height + INSET_PAD_V;
        let rect = Rect::new(column.x - INSET_HANG_LEFT, top, column.width + INSET_HANG_LEFT + INSET_HANG_RIGHT, (bottom - top).max(0.0));
        let figure = leak_box(InsetBackdrop { fill: paint.fill, bar: paint.bar, bar_width: paint.bar_width });
        let kind = leak_box(SceneBlock::Figure(FigureBlock::new(figure, BlockSizing::FixedHeight(rect.height))));
        let placed = PlacedBlock { id: frame.blocks[start].id, rect, kind, paragraph_layout: None, table_placement: None, list_placement: None };
        frame.blocks.insert(start, placed);
    }
}

/// Relative luminance of `0xRRGGBB`, 0.0 (black) to 1.0 (white); the
/// sRGB channels are weighted but not linearised, which is enough to tell a
/// dark page from a light one.
fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| f64::from((color >> shift) & 0xff) / 255.0;
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// `base` with `amount` (0..1) of `over` mixed in, per channel.
fn mix(base: u32, over: u32, amount: f64) -> u32 {
    let blend = |shift: u32| {
        let b = f64::from((base >> shift) & 0xff);
        let o = f64::from((over >> shift) & 0xff);
        ((b + (o - b) * amount).round() as u32).min(255)
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}

/// Arrows block U+2190..U+2199 (left, up, right, down, both, and diagonals).
fn is_arrow(c: char) -> bool {
    ('\u{2190}'..='\u{2199}').contains(&c)
}

/// `text` cut into maximal segments that are all arrows or all non-arrows,
/// each with a flag saying which.
fn split_arrows(text: &str) -> Vec<(&str, bool)> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut current: Option<bool> = None;
    for (index, c) in text.char_indices() {
        let arrow = is_arrow(c);
        if current.is_some_and(|kind| kind != arrow) {
            segments.push((&text[start..index], !arrow));
            start = index;
        }
        current = Some(arrow);
    }
    if let Some(kind) = current {
        segments.push((&text[start..], kind));
    }
    segments
}

/// Anonymous theme used by behavioral tests: a band cover, inset labels,
/// a DOCX face override, and totals prefixes. Not a built-in theme.
#[cfg(test)]
pub(crate) fn example_doc_palette() -> Palette {
    let mut palette = Palette::light();
    palette.doc = DocStyle {
        docx_font: Some("Example Face".to_owned()),
        table_rule: Some(RuleStyle { color: 0x666666, width: 0.5 }),
        table_head_fill: Some(0xD0D7DE),
        table_total_fill: Some(0xFFF4CC),
        total_prefixes: vec!["Total".to_owned(), "Sum".to_owned()],
        callout: CalloutStyle::Tinted { fill: 0xFFF4E5, bar: 0x112233, bar_width: 2.0 },
        page_mark: Some(0x445566),
        cover: CoverStyle::Band { shape: 0x336699, slant: 0.5 },
        footer_logo: false,
        symbol_face: Some(FontFamily::PtRootUi),
        revised: InsetStyle {
            fill: Some(0xE5F6D8),
            bar: Some(0x3D7A1A),
            bar_width: 2.0,
            label: Some(InsetLabel { text: "Revised".to_owned(), size: 9.0 }),
        },
        technical: InsetStyle {
            fill: Some(0xE5F0FA),
            bar: Some(0x1A527A),
            bar_width: 2.0,
            label: Some(InsetLabel { text: "Note".to_owned(), size: 9.0 }),
        },
    };
    palette
}

/// Sheet used by behavioral tests that lock heading space, a page break
/// before H1, list lines, and DOCX cell margins. Starts from [`Preset::long`].
#[cfg(test)]
pub(crate) fn example_doc_preset() -> crate::preset::Preset {
    use crate::preset::{HeadingSpace, SheetExtras};
    use uzor_typeset::TableRules;
    let mut preset = crate::preset::Preset::long();
    preset.page.sizes.list_item = 10.0;
    preset.page.sizes.callout = 9.5;
    preset.page.sizes.table_header = 9.0;
    preset.page.sizes.table_cell = 9.0;
    let line = 11.0 * 1.2 * 1.15;
    preset.page.extras = SheetExtras {
        heading_space: [
            HeadingSpace { before: 14.0, after: 10.0 },
            HeadingSpace { before: 10.0, after: 8.0 },
            HeadingSpace { before: 8.0, after: 6.0 },
        ],
        heading_leading: Some(line),
        h1_page_break: true,
        list_leading: Some(12.0),
        justify_lists: true,
        docx_cell_margins: true,
    };
    preset.page.table_rules = TableRules::Box;
    preset.page.cell_padding = CellPadding::new(6.0, 3.0);
    preset
}

/// `true` when the first non-empty cell of a table row starts with one of
/// `prefixes` (any case): a totals row. Both emitters style such rows.
pub(crate) fn is_total_row(row: &[Vec<InlineRun>], prefixes: &[String]) -> bool {
    row.iter()
        .map(|cell| Press::plain_text(cell).trim().to_lowercase())
        .find(|text| !text.is_empty())
        .is_some_and(|text| prefixes.iter().any(|prefix| text.starts_with(&prefix.to_lowercase())))
}

#[cfg(test)]
mod tests {
    use super::{
        inset_id, inset_kind, is_total_row, keep_with_following_figure, luminance, mix, split_arrows, AstBlock, BlockNode, BreakControl, DocStyle,
        Format, ImageSpec, InlineRun, InsetKind, InsetPart, Palette, Press, INSET_HANG_LEFT, INSET_HANG_RIGHT, INSET_PAD_V,
    };
    use uzor::fonts::FontFamily;
    use uzor_typeset::Block as SceneBlock;

    /// Built-in themes keep Roboto and leave every print extra off.
    #[test]
    fn builtin_themes_keep_roboto_and_leave_doc_style_off() {
        for palette in [Palette::light(), Palette::dark()] {
            assert_eq!(palette.font_display, FontFamily::Roboto);
            assert_eq!(palette.font_mono, FontFamily::Roboto);
            assert_eq!(palette.doc, DocStyle::default());
            assert!(palette.categorical_extra.is_empty());
            assert!(palette.logo_sentinel.is_none());
            assert!(palette.logo_mark.is_none());
        }
    }

    /// A theme file can name the DOCX face, the band cover, and the labels
    /// without changing the built-in themes.
    #[test]
    fn an_anonymous_theme_names_the_docx_face_the_band_and_the_labels() {
        let palette = super::example_doc_palette();
        assert_eq!(palette.doc.docx_font.as_deref(), Some("Example Face"));
        assert_eq!(palette.doc.symbol_face, Some(FontFamily::PtRootUi));
        assert_eq!(palette.doc.page_mark, Some(0x445566));
        assert!(matches!(palette.doc.cover, super::CoverStyle::Band { shape: 0x336699, slant } if slant == 0.5));
        assert_eq!(palette.doc.total_prefixes, vec!["Total".to_owned(), "Sum".to_owned()]);
    }

    #[test]
    fn mix_blends_per_channel_and_luminance_tells_dark_from_light() {
        assert_eq!(mix(0xFFFFFF, 0x000000, 0.0), 0xFFFFFF);
        assert_eq!(mix(0xFFFFFF, 0x000000, 1.0), 0x000000);
        assert_eq!(mix(0x000000, 0xFF8040, 0.5), 0x804020);
        assert!(luminance(0x0a0f1a) < 0.5 && luminance(0xffffff) > 0.5);
    }

    /// An explicit theme names both block kinds; a built-in theme derives
    /// them from its own palette and prints no label.
    #[test]
    fn inset_paint_is_explicit_when_the_theme_says_so_and_derived_otherwise() {
        let revised = super::example_doc_palette().inset_paint(InsetKind::Revised);
        assert_eq!((revised.fill, revised.bar, revised.bar_width), (0xE5F6D8, 0x3D7A1A, 2.0));
        assert_eq!(revised.label.as_ref().map(|l| (l.text.as_str(), l.size)), Some(("Revised", 9.0)));
        let technical = super::example_doc_palette().inset_paint(InsetKind::Technical);
        assert_eq!((technical.fill, technical.bar, technical.bar_width), (0xE5F0FA, 0x1A527A, 2.0));
        assert_eq!(technical.label.as_ref().map(|l| (l.text.as_str(), l.size)), Some(("Note", 9.0)));

        for palette in [Palette::light(), Palette::dark()] {
            for (kind, bar) in [(InsetKind::Revised, palette.up), (InsetKind::Technical, palette.second)] {
                let paint = palette.inset_paint(kind);
                assert_eq!(paint.bar, bar, "the bar is the palette's own colour for this kind");
                assert_ne!(paint.fill, palette.bg, "the fill is a visible tint");
                assert_eq!(paint.label, None);
                let (bg, fill, bar) = (luminance(palette.bg), luminance(paint.fill), luminance(paint.bar));
                assert!((fill - bg).abs() <= (bar - bg).abs(), "the fill stays closer to the page than the bar does");
            }
        }
    }

    /// Revised text is set like the body; a technical addition at the callout
    /// size with its line scaled from the body line.
    #[test]
    fn inset_sizes_follow_the_kind() {
        let press = Press::new(Format::Doc, super::example_doc_palette(), super::example_doc_preset(), std::path::PathBuf::new());
        let revised = press.inset_sizes(InsetKind::Revised);
        assert_eq!((revised.text, revised.item), (11.0, 10.0));
        let technical = press.inset_sizes(InsetKind::Technical);
        assert_eq!((technical.text, technical.item), (9.5, 9.5));
        assert!((technical.text_leading - 11.0 * 1.2 * 1.15 * 9.5 / 11.0).abs() < 1e-9);
        assert!((technical.item_leading.expect("lists have an exact line") - 12.0 * 9.5 / 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_block_id_names_its_kind_and_no_other_id_does() {
        assert_eq!(inset_kind(inset_id(InsetKind::Revised, 7)), Some(InsetKind::Revised));
        assert_eq!(inset_kind(inset_id(InsetKind::Technical, 7)), Some(InsetKind::Technical));
        assert_ne!(inset_id(InsetKind::Revised, 7), inset_id(InsetKind::Technical, 7));
        assert_eq!(inset_kind(uzor_typeset::BlockId::structural(7)), None);
        assert_eq!(inset_kind(uzor_typeset::BlockId(7)), None);
    }

    fn inset_pages(kind_marker: &str, paragraphs: usize) -> (Press, Vec<uzor_typeset::Page<'static>>) {
        let mut source = String::from("---\ntitle: T\n---\n\nОригинальный абзац.\n\n> Комментарий.\n\n");
        for paragraph in 0..paragraphs {
            if paragraph > 0 {
                source.push_str(&format!("{kind_marker}\n"));
            }
            source.push_str(&format!("{kind_marker} Revised paragraph number one, a line long enough to wrap onto the next line of the page and continue there.\n"));
        }
        source.push_str("\nAfter the block.\n");
        let document = crate::parse::parse(&source).expect("fixture parses");
        let press = Press::new(Format::Doc, super::example_doc_palette(), super::example_doc_preset(), std::path::PathBuf::new());
        let output = press.build(&document).expect("press builds");
        (press, output.pages)
    }

    /// A block that runs over a page: every page that holds a part of it
    /// gets one backdrop ahead of that part, wide enough to hang into both
    /// margins, and the text keeps the body column. The technical kind sets
    /// its text at the callout size.
    #[test]
    fn a_block_gets_a_backdrop_on_every_page_it_touches() {
        for (marker, kind, text_size) in [(">>", InsetKind::Revised, 11.0), (">>>", InsetKind::Technical, 9.5)] {
            let (press, pages) = inset_pages(marker, 40);
            assert!(pages.len() >= 3, "the block must cross a page break, got {} pages", pages.len());

            let margins = press.margins();
            let (page_width, _) = Format::Doc.page_size();
            let mut pages_with_block = 0;
            for page in &pages {
                let blocks = &page.frame.blocks;
                let owned: Vec<usize> = (0..blocks.len()).filter(|&i| inset_kind(blocks[i].id) == Some(kind)).collect();
                if owned.is_empty() {
                    continue;
                }
                pages_with_block += 1;
                let backdrops: Vec<usize> = owned.iter().copied().filter(|&i| matches!(blocks[i].kind, SceneBlock::Figure(_))).collect();
                assert_eq!(backdrops.len(), 1, "one backdrop per page for one block");
                let first = owned[0];
                assert_eq!(backdrops[0], first, "the backdrop is drawn ahead of the text it sits behind");
                let backdrop = blocks[first].rect;
                let last = blocks[*owned.last().expect("non-empty")].rect;
                assert!((backdrop.x - (margins.left - INSET_HANG_LEFT)).abs() < 1e-6);
                assert!((backdrop.width - (page_width - margins.left - margins.right + INSET_HANG_LEFT + INSET_HANG_RIGHT)).abs() < 1e-6);
                assert!((backdrop.y - (blocks[first + 1].rect.y - INSET_PAD_V)).abs() < 1e-6);
                assert!((backdrop.y + backdrop.height - (last.y + last.height + INSET_PAD_V)).abs() < 1e-6);
                for text in &owned[1..] {
                    assert!((blocks[*text].rect.x - margins.left).abs() < 1e-6, "text keeps the body column");
                }
                // The first placed paragraph after the label is the block's text.
                let sized = owned[1..].iter().filter_map(|&i| blocks[i].paragraph_layout.as_ref()).filter_map(|l| l.glyphs.first()).map(|g| g.font.size_px);
                assert!(sized.clone().count() > 0);
                assert!(sized.skip(1).all(|size| (size - text_size).abs() < 1e-9), "text size of the {kind:?} kind");
            }
            assert!(pages_with_block >= 2, "the block continues on a second page");
        }
    }

    /// A document with no `>>` block is untouched: no backdrop, no id.
    #[test]
    fn a_document_without_a_block_gets_no_backdrop() {
        let document = crate::parse::parse("---\ntitle: T\n---\n\nАбзац.\n\n> Комментарий.\n").expect("parses");
        let press = Press::new(Format::Doc, Palette::light(), crate::preset::Preset::long(), std::path::PathBuf::new());
        let output = press.build(&document).expect("press builds");
        assert!(output.pages.iter().all(|p| p.frame.blocks.iter().all(|b| inset_kind(b.id).is_none())));
    }

    /// A picture right after a `>>>` block keeps with its last node; after a
    /// `>>` block, or when the node has a choice of its own, nothing changes.
    #[test]
    fn a_picture_after_a_technical_block_keeps_with_it() {
        let technical = AstBlock::Inset { kind: InsetKind::Technical, parts: vec![InsetPart::Paragraph(vec![InlineRun::Plain("x".to_owned())])] };
        let revised = AstBlock::Inset { kind: InsetKind::Revised, parts: vec![InsetPart::Paragraph(vec![InlineRun::Plain("x".to_owned())])] };
        let picture = AstBlock::Image(ImageSpec { path: std::path::PathBuf::from("p.png"), height: None, island: None, width: None, caption: None });
        let paragraph = AstBlock::Paragraph(vec![InlineRun::Plain("x".to_owned())]);
        let node = || vec![BlockNode::new(SceneBlock::Spacer(1.0))];

        let mut flow = node();
        keep_with_following_figure(&mut flow, &technical, Some(&picture));
        assert_eq!(flow[0].break_control, BreakControl::AvoidAfter);

        for (block, next) in [(&technical, Some(&paragraph)), (&technical, None), (&revised, Some(&picture))] {
            let mut flow = node();
            keep_with_following_figure(&mut flow, block, next);
            assert_eq!(flow[0].break_control, BreakControl::Auto);
        }

        let mut flow = vec![BlockNode::new(SceneBlock::Spacer(1.0)).with_break_control(BreakControl::ForceBefore)];
        keep_with_following_figure(&mut flow, &technical, Some(&picture));
        assert_eq!(flow[0].break_control, BreakControl::ForceBefore, "an explicit choice stays");
    }

    #[test]
    fn split_arrows_cuts_text_at_arrow_boundaries() {
        assert_eq!(split_arrows("a → b"), vec![("a ", false), ("→", true), (" b", false)]);
        assert_eq!(split_arrows("→→x"), vec![("→→", true), ("x", false)]);
        assert_eq!(split_arrows("plain"), vec![("plain", false)]);
    }

    #[test]
    fn total_rows_are_found_by_their_first_non_empty_cell() {
        let cell = |t: &str| vec![InlineRun::Plain(t.to_owned())];
        let prefixes = vec!["Total".to_owned(), "Sum".to_owned()];
        assert!(is_total_row(&[cell(""), cell("Total"), cell("5")], &prefixes));
        assert!(is_total_row(&[cell("SUM of section"), cell("5")], &prefixes));
        assert!(is_total_row(&[cell("Total for section"), cell("5")], &prefixes));
        assert!(!is_total_row(&[cell("Service"), cell("Total")], &prefixes));
        assert!(!is_total_row(&[cell("Total"), cell("5")], &[]));
    }
}
