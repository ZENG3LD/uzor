//! Custom `TypesetFigure` implementations: `Heading`, `KpiRow`, `Diagram`,
//! `Cover`, `Closing`, `FooterMark`, `StrokeMarkFigure`, `BandCover`,
//! `PageMark`, `CalloutBox`, `InsetBackdrop` — every one paints only through
//! `RenderContext` (`SPEC.md`'s "Custom figures" section), never a bespoke
//! drawing path.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;
use uzor_figures::FigureTheme;
use uzor_text::{
    draw_paragraph, layout_paragraph, layout_text, paragraph_intrinsic_size, CosmicShaper, FontSpec, Paragraph, ParagraphAlign,
    ParagraphLayout, StyledRun,
};
use uzor_typeset::{BlockSizing, ImageFit, TypesetFigure};

use crate::parse::DiagramBox;
use crate::press::Palette;

/// Gap between an H1's own text and its 64px accent rule.
pub const HEADING_RULE_GAP: f64 = 6.0;
/// The rule's own height.
pub const HEADING_RULE_H: f64 = 2.0;
/// Gap left BELOW the rule, before whatever flow content follows — part of
/// this figure's own `BlockSizing::FixedHeight`, not painted.
pub const HEADING_TRAILING_GAP: f64 = 8.0;
const HEADING_RULE_W: f64 = 64.0;

/// An H1 heading + its own 2px x 64px accent rule painted as ONE atomic
/// figure (not two separate flow nodes) — see `press.rs`'s own
/// `push_heading` doc comment for why: `BreakControl::AvoidAfter` only
/// peeks at the SINGLE next flow node, so a separate heading-paragraph +
/// rule-figure pair let the (always-fits) rule satisfy that peek and
/// strand the pair at a page bottom while the real next content (the
/// following paragraph) didn't fit. Folding heading+rule into one figure
/// makes the paragraph after it the peeked-at node instead.
pub struct Heading {
    pub layout: ParagraphLayout,
    pub palette: Palette,
}

impl TypesetFigure for Heading {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        draw_paragraph(ctx, (rect.x, rect.y), &self.layout, &Palette::hex(self.palette.ink), false);
        ctx.set_fill_color(&Palette::hex(self.palette.accent));
        ctx.fill_rect(rect.x, rect.y + self.layout.height + HEADING_RULE_GAP, HEADING_RULE_W, HEADING_RULE_H);
    }
}

/// One `:::kpi` tile's own already-resolved content — `value_font` is
/// ALREADY auto-fit (shrunk in `press.rs`'s own `fit_kpi_value_font` until
/// the value string fits the tile's own inner width, or the size floor is
/// hit), so [`KpiRow::render`] never re-fits anything, only paints.
pub struct KpiTile {
    pub value: String,
    pub value_font: FontSpec,
    pub label: String,
    pub label_font: FontSpec,
}

/// N value/label tiles across the full width — `:::kpi` blocks.
/// `value_baseline_y`/`label_y_offset` are both derived from the NOMINAL
/// (never-shrunk) value font's own line metrics, computed ONCE in
/// `press.rs` and shared across every tile — a tile whose own value was
/// auto-shrunk to fit still paints on the SAME baseline as its siblings
/// (never floating above the row just because its glyphs are smaller),
/// and every label starts at the same y regardless of its own tile's
/// value size.
pub struct KpiRow {
    pub tiles: Vec<KpiTile>,
    pub palette: Palette,
    pub value_baseline_y: f64,
    pub label_y_offset: f64,
}

pub const KPI_GAP: f64 = 18.0;
pub const KPI_PAD_X: f64 = 14.0;
pub const KPI_TOP_PAD: f64 = 10.0;
pub const KPI_VALUE_LABEL_GAP: f64 = 6.0;
pub const KPI_BOTTOM_PAD: f64 = 10.0;
/// Extra blank space at the bottom of this figure's own `BlockSizing::
/// FixedHeight`, left unpainted (the tile panel/border/accent-bar fill
/// stops at `rect.height - KPI_EXTRA_BOTTOM`) — a visible buffer between
/// the tiles and whatever flow content follows.
pub const KPI_EXTRA_BOTTOM: f64 = 12.0;

impl TypesetFigure for KpiRow {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let shaper = CosmicShaper::headless();
        let n = self.tiles.len().max(1);
        let tile_w = ((rect.width - KPI_GAP * (n as f64 - 1.0)) / n as f64).max(1.0);
        let visible_h = (rect.height - KPI_EXTRA_BOTTOM).max(1.0);

        for (i, tile) in self.tiles.iter().enumerate() {
            let x = rect.x + i as f64 * (tile_w + KPI_GAP);

            ctx.set_fill_color(&Palette::hex(self.palette.panel));
            ctx.fill_rect(x, rect.y, tile_w, visible_h);
            ctx.set_stroke_color(&Palette::hex(self.palette.line));
            ctx.set_stroke_width(1.0);
            ctx.stroke_rect(x + 0.5, rect.y + 0.5, (tile_w - 1.0).max(0.0), (visible_h - 1.0).max(0.0));
            ctx.set_fill_color(&Palette::hex(self.palette.accent));
            ctx.fill_rect(x, rect.y, 3.0, visible_h);

            ctx.set_font(&tile.value_font.to_css_font());
            ctx.set_fill_color(&Palette::hex(self.palette.accent));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Alphabetic);
            ctx.fill_text(&tile.value, x + KPI_PAD_X, rect.y + self.value_baseline_y);

            let label_w = (tile_w - 2.0 * KPI_PAD_X).max(1.0);
            let label_layout = layout_text(&tile.label, &tile.label_font, label_w, &shaper);
            let label_y = rect.y + self.label_y_offset;
            draw_paragraph(ctx, (x + KPI_PAD_X, label_y), &label_layout, &Palette::hex(self.palette.muted), false);
        }
    }
}

/// Rows of connected boxes — `:::diagram` blocks.
pub struct Diagram {
    pub rows: Vec<Vec<DiagramBox>>,
    pub palette: Palette,
    pub title_font: FontSpec,
    pub subtitle_font: FontSpec,
    pub row_h: f64,
    pub row_gap: f64,
    pub box_gap: f64,
}

const DIAGRAM_BOX_RADIUS: f64 = 6.0;
const DIAGRAM_BOX_PAD: f64 = 8.0;
const DIAGRAM_TITLE_SUBTITLE_GAP: f64 = 4.0;

impl TypesetFigure for Diagram {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let shaper = CosmicShaper::headless();

        let mut row_rects: Vec<Vec<Rect>> = Vec::with_capacity(self.rows.len());
        let mut y = rect.y;
        for row in &self.rows {
            let n = row.len().max(1);
            let w = ((rect.width - self.box_gap * (n as f64 - 1.0)) / n as f64).max(1.0);
            let boxes = (0..row.len()).map(|i| Rect::new(rect.x + i as f64 * (w + self.box_gap), y, w, self.row_h)).collect();
            row_rects.push(boxes);
            y += self.row_h + self.row_gap;
        }

        // Connectors: a horizontal bus under the row above, plus one
        // vertical stem per box in the row below, hanging off the bus.
        ctx.set_stroke_color(&Palette::hex(self.palette.line));
        ctx.set_stroke_width(1.0);
        for r in 1..row_rects.len() {
            let (Some(prev_first), Some(prev_last)) = (row_rects[r - 1].first(), row_rects[r - 1].last()) else { continue };
            let bus_y = prev_first.y + self.row_h;
            ctx.begin_path();
            ctx.move_to(prev_first.x, bus_y);
            ctx.line_to(prev_last.x + prev_last.width, bus_y);
            ctx.stroke();
            for box_rect in &row_rects[r] {
                let cx = box_rect.x + box_rect.width / 2.0;
                ctx.begin_path();
                ctx.move_to(cx, bus_y);
                ctx.line_to(cx, box_rect.y);
                ctx.stroke();
            }
        }

        for (row, boxes) in self.rows.iter().zip(&row_rects) {
            for (b, box_rect) in row.iter().zip(boxes) {
                ctx.set_fill_color(&Palette::hex(self.palette.panel));
                ctx.fill_rounded_rect(box_rect.x, box_rect.y, box_rect.width, box_rect.height, DIAGRAM_BOX_RADIUS);
                ctx.set_stroke_color(&Palette::hex(self.palette.second));
                ctx.set_stroke_width(1.0);
                ctx.stroke_rounded_rect(
                    box_rect.x + 0.5,
                    box_rect.y + 0.5,
                    (box_rect.width - 1.0).max(0.0),
                    (box_rect.height - 1.0).max(0.0),
                    DIAGRAM_BOX_RADIUS,
                );
                self.draw_box_text(ctx, &shaper, b, *box_rect);
            }
        }
    }
}

impl Diagram {
    fn draw_box_text(&self, ctx: &mut dyn RenderContext, shaper: &CosmicShaper, b: &DiagramBox, box_rect: Rect) {
        let interior_w = (box_rect.width - 2.0 * DIAGRAM_BOX_PAD).max(1.0);
        let subtitle_layout = b.subtitle.as_ref().map(|s| {
            let run = [StyledRun::new(s.as_str(), self.subtitle_font)];
            layout_paragraph(&Paragraph::new(&run, interior_w).with_align(ParagraphAlign::Center), shaper)
        });
        let title_h = self.title_font.size_px * 1.2;
        let subtitle_h = subtitle_layout.as_ref().map_or(0.0, |l| l.height);
        let content_h = title_h + if subtitle_layout.is_some() { DIAGRAM_TITLE_SUBTITLE_GAP + subtitle_h } else { 0.0 };
        let start_y = box_rect.y + ((box_rect.height - content_h) / 2.0).max(0.0);

        ctx.set_font(&self.title_font.to_css_font());
        ctx.set_fill_color(&Palette::hex(self.palette.ink));
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(&b.title, box_rect.x + box_rect.width / 2.0, start_y);

        if let Some(layout) = &subtitle_layout {
            let ox = box_rect.x + (box_rect.width - interior_w) / 2.0;
            draw_paragraph(ctx, (ox, start_y + title_h + DIAGRAM_TITLE_SUBTITLE_GAP), layout, &Palette::hex(self.palette.muted), false);
        }
    }
}

/// A decoded image handed to [`Cover`]/[`Closing`] — RGBA8 pixels plus
/// their own intrinsic size.
#[derive(Clone, Copy)]
pub struct CoverImage {
    pub rgba: &'static [u8],
    pub width: u32,
    pub height: u32,
}

/// Paint `image` contain-fit into `rect`, reusing
/// [`uzor_typeset::ImageBlock::content_rect`]'s own fit-rect geometry
/// (design law 1 — no second position formula) instead of re-deriving the
/// CSS `object-fit: contain` math here.
fn draw_contain_image(ctx: &mut dyn RenderContext, image: &CoverImage, rect: Rect) {
    let block = uzor_typeset::ImageBlock::new(image.rgba, image.width, image.height, BlockSizing::FixedHeight(rect.height), ImageFit::Contain);
    let (cx, cy, cw, ch) = block.content_rect(rect.width, rect.height);
    if cw <= 0.0 || ch <= 0.0 {
        return;
    }
    if let Some(painter) = ctx.image_painter() {
        painter.draw_image_rgba(image.rgba, image.width, image.height, rect.x + cx, rect.y + cy, cw, ch);
    }
}

/// A mark handed to [`Cover`]/[`FooterMark`] — either a decoded raster PNG
/// (the pre-existing `logo: <path>` mechanism) or a stroke mark from the
/// theme file, selected when the logo file name equals the theme's logo
/// sentinel (`press.rs`'s own `Press::resolve_logo`).
#[derive(Clone)]
pub enum LogoAsset {
    Raster(CoverImage),
    Mark { viewbox: f64, stroke_width: f64, polylines: Vec<Vec<(f64, f64)>>, wordmark: Option<String> },
}

impl LogoAsset {
    /// `width / height` of the mark's own natural shape — the decoded
    /// PNG's own intrinsic size, or `1.0` (square view box) for a stroke mark.
    fn aspect(&self) -> f64 {
        match self {
            LogoAsset::Raster(image) => image.width as f64 / image.height.max(1) as f64,
            LogoAsset::Mark { .. } => 1.0,
        }
    }

    fn wordmark(&self) -> Option<&str> {
        match self {
            LogoAsset::Mark { wordmark, .. } => wordmark.as_deref(),
            LogoAsset::Raster(_) => None,
        }
    }
}

/// Paint the polylines of a stroke mark, contain-fit and centered in `rect`,
/// in `color_hex`. Shared by [`draw_logo_asset`] and [`StrokeMarkFigure`].
fn paint_mark(ctx: &mut dyn RenderContext, rect: Rect, viewbox: f64, stroke_width: f64, polylines: &[Vec<(f64, f64)>], color_hex: &str) {
    if viewbox <= 0.0 {
        return;
    }
    let size = rect.width.min(rect.height);
    if size <= 0.0 {
        return;
    }
    let scale = size / viewbox;
    let ox = rect.x + (rect.width - size) / 2.0;
    let oy = rect.y + (rect.height - size) / 2.0;
    ctx.set_stroke_color(color_hex);
    ctx.set_stroke_width((stroke_width * scale).max(0.5));
    for line in polylines {
        let Some((first, rest)) = line.split_first() else { continue };
        ctx.begin_path();
        ctx.move_to(ox + first.0 * scale, oy + first.1 * scale);
        for &(x, y) in rest {
            ctx.line_to(ox + x * scale, oy + y * scale);
        }
        ctx.stroke();
    }
}

/// Paint `logo` contain-fit into `rect` — a raster logo through
/// [`draw_contain_image`] (its own baked-in colors, `accent_hex` unused),
/// a stroke mark through [`paint_mark`] at `accent_hex`.
fn draw_logo_asset(ctx: &mut dyn RenderContext, logo: &LogoAsset, rect: Rect, accent_hex: &str) {
    match logo {
        LogoAsset::Raster(image) => draw_contain_image(ctx, image, rect),
        LogoAsset::Mark { viewbox, stroke_width, polylines, .. } => paint_mark(ctx, rect, *viewbox, *stroke_width, polylines, accent_hex),
    }
}

/// Raster target for a stroke mark. The wordmark is not part of the raster;
/// the cover draws it beside the mark.
pub struct StrokeMarkFigure {
    pub viewbox: f64,
    pub stroke_width: f64,
    pub polylines: Vec<Vec<(f64, f64)>>,
    pub color_hex: String,
}

impl TypesetFigure for StrokeMarkFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        paint_mark(ctx, rect, self.viewbox, self.stroke_width, &self.polylines, &self.color_hex);
    }
}

/// `:::cover` — the opening slide/page. Left 58%: title/subtitle/rule/
/// meta, vertically centered as a group. Right 42%: the image, contain-fit,
/// vertically centered. Bottom-left: `site`. Background = the page's own
/// background (nothing extra painted here). Two theme-independent,
/// front-matter-driven extras, both no-ops (byte-identical rendering)
/// when absent: `logo` (top-left corner mark, contain-fit within
/// `logo_height` — plus the mark's wordmark to its right when that mark
/// carries one) and `badge` (a rounded accent pill, `bg`-colored uppercase
/// text, top-right corner).
pub struct Cover {
    pub title: String,
    pub subtitle: String,
    pub meta: String,
    pub site: String,
    pub image: Option<CoverImage>,
    pub logo: Option<LogoAsset>,
    pub logo_height: f64,
    pub badge: Option<String>,
    pub badge_font: FontSpec,
    pub palette: Palette,
    pub title_font: FontSpec,
    pub subtitle_font: FontSpec,
    pub meta_font: FontSpec,
    pub site_font: FontSpec,
    /// Wordmark face beside a [`LogoAsset::Mark`] that carries a wordmark.
    /// Unread when `logo` is `None` or a raster (no wordmark is drawn then).
    pub wordmark_font: FontSpec,
}

const COVER_LEFT_FRACTION: f64 = 0.58;
const COVER_RIGHT_FRACTION: f64 = 0.42;
const COVER_LEFT_RIGHT_PAD: f64 = 24.0;
const COVER_RULE_W: f64 = 96.0;
const COVER_RULE_H: f64 = 3.0;
const COVER_BADGE_PAD_X: f64 = 14.0;
const COVER_BADGE_PAD_Y: f64 = 7.0;
const COVER_WORDMARK_GAP: f64 = 12.0;

impl TypesetFigure for Cover {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let shaper = CosmicShaper::headless();
        let left_w = rect.width * COVER_LEFT_FRACTION;
        let right_w = rect.width * COVER_RIGHT_FRACTION;
        let content_w = (left_w - COVER_LEFT_RIGHT_PAD).max(1.0);

        let title_run = [StyledRun::new(self.title.as_str(), self.title_font)];
        let title_layout = layout_paragraph(&Paragraph::new(&title_run, content_w), &shaper);
        let subtitle_run = [StyledRun::new(self.subtitle.as_str(), self.subtitle_font)];
        let subtitle_layout = layout_paragraph(&Paragraph::new(&subtitle_run, content_w), &shaper);
        let meta_run = [StyledRun::new(self.meta.as_str(), self.meta_font)];
        let meta_layout = layout_paragraph(&Paragraph::new(&meta_run, content_w), &shaper);

        let gap_title_subtitle = 16.0;
        let gap_subtitle_rule = 20.0;
        let gap_rule_meta = 18.0;
        let content_h = title_layout.height + gap_title_subtitle + subtitle_layout.height + gap_subtitle_rule + COVER_RULE_H + gap_rule_meta + meta_layout.height;
        let start_y = rect.y + ((rect.height - content_h) / 2.0).max(0.0);
        let left_x = rect.x;

        let mut y = start_y;
        draw_paragraph(ctx, (left_x, y), &title_layout, &Palette::hex(self.palette.ink), false);
        y += title_layout.height + gap_title_subtitle;
        draw_paragraph(ctx, (left_x, y), &subtitle_layout, &Palette::hex(self.palette.ink), false);
        y += subtitle_layout.height + gap_subtitle_rule;
        ctx.set_fill_color(&Palette::hex(self.palette.accent));
        ctx.fill_rect(left_x, y, COVER_RULE_W, COVER_RULE_H);
        y += COVER_RULE_H + gap_rule_meta;
        draw_paragraph(ctx, (left_x, y), &meta_layout, &Palette::hex(self.palette.muted), false);

        if let Some(image) = &self.image {
            let right_x = rect.x + rect.width - right_w;
            draw_contain_image(ctx, image, Rect::new(right_x, rect.y, right_w, rect.height));
        }

        if let Some(logo) = &self.logo {
            let h = self.logo_height.min(rect.height);
            let w = h * logo.aspect();
            let accent_hex = Palette::hex(self.palette.accent);
            draw_logo_asset(ctx, logo, Rect::new(rect.x, rect.y, w, h), &accent_hex);

            if let Some(wordmark) = logo.wordmark() {
                ctx.set_font(&self.wordmark_font.to_css_font());
                ctx.set_fill_color(&accent_hex);
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(wordmark, rect.x + w + COVER_WORDMARK_GAP, rect.y + h / 2.0);
            }
        }

        if let Some(badge) = &self.badge {
            let text = badge.to_uppercase();
            let (text_w, text_h) = paragraph_intrinsic_size(&text, &self.badge_font, f64::MAX, &shaper);
            let pill_w = text_w + 2.0 * COVER_BADGE_PAD_X;
            let pill_h = text_h + 2.0 * COVER_BADGE_PAD_Y;
            let px = rect.x + rect.width - pill_w;
            let py = rect.y;
            ctx.set_fill_color(&Palette::hex(self.palette.accent));
            ctx.fill_rounded_rect(px, py, pill_w, pill_h, pill_h / 2.0);
            ctx.set_font(&self.badge_font.to_css_font());
            ctx.set_fill_color(&Palette::hex(self.palette.bg));
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(&text, px + pill_w / 2.0, py + pill_h / 2.0);
        }

        ctx.set_font(&self.site_font.to_css_font());
        ctx.set_fill_color(&Palette::hex(self.palette.muted));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Bottom);
        ctx.fill_text(&self.site, left_x, rect.y + rect.height);
    }
}

/// `:::closing` — same keys as [`Cover`], smaller (H1-sized) title,
/// centered; the image (if any) sits above the title at 40% of the
/// region's own height.
pub struct Closing {
    pub title: String,
    pub subtitle: String,
    pub site: String,
    pub image: Option<CoverImage>,
    pub palette: Palette,
    pub title_font: FontSpec,
    pub subtitle_font: FontSpec,
    pub site_font: FontSpec,
}

impl TypesetFigure for Closing {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let shaper = CosmicShaper::headless();
        let image_h = rect.height * 0.4;
        let gap_image_title = if self.image.is_some() { 24.0 } else { 0.0 };
        let gap_title_subtitle = 14.0;

        let title_run = [StyledRun::new(self.title.as_str(), self.title_font)];
        let title_layout = layout_paragraph(&Paragraph::new(&title_run, rect.width).with_align(ParagraphAlign::Center), &shaper);
        let subtitle_run = [StyledRun::new(self.subtitle.as_str(), self.subtitle_font)];
        let subtitle_layout = layout_paragraph(&Paragraph::new(&subtitle_run, rect.width).with_align(ParagraphAlign::Center), &shaper);

        let image_block_h = if self.image.is_some() { image_h + gap_image_title } else { 0.0 };
        let content_h = image_block_h + title_layout.height + gap_title_subtitle + subtitle_layout.height;
        let start_y = rect.y + ((rect.height - content_h) / 2.0).max(0.0);

        let mut y = start_y;
        if let Some(image) = &self.image {
            draw_contain_image(ctx, image, Rect::new(rect.x, y, rect.width, image_h));
            y += image_h + gap_image_title;
        }
        draw_paragraph(ctx, (rect.x, y), &title_layout, &Palette::hex(self.palette.ink), false);
        y += title_layout.height + gap_title_subtitle;
        draw_paragraph(ctx, (rect.x, y), &subtitle_layout, &Palette::hex(self.palette.ink), false);

        ctx.set_font(&self.site_font.to_css_font());
        ctx.set_fill_color(&Palette::hex(self.palette.muted));
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Bottom);
        ctx.fill_text(&self.site, rect.x + rect.width / 2.0, rect.y + rect.height);
    }
}

/// The footer flow's own figure when a front-matter `logo:` is
/// configured: a small (`FOOTER_MARK_H`) contain-fit brand mark at the
/// left of the footer line, followed by the ordinary footer text —
/// `press.rs`'s own `footer_master` only builds this figure when a logo
/// is present; every pre-existing content file (no `logo:` key) keeps
/// the original plain-`Paragraph` footer untouched.
pub struct FooterMark {
    pub logo: LogoAsset,
    pub text: String,
    pub font: FontSpec,
    pub palette: Palette,
}

/// The mark's own painted height — `SPEC.md`: "a small mark (~18 pt) at
/// the left of the footer line."
const FOOTER_MARK_H: f64 = 18.0;
const FOOTER_MARK_GAP: f64 = 8.0;

impl TypesetFigure for FooterMark {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let h = FOOTER_MARK_H.min(rect.height);
        let w = h * self.logo.aspect();
        let y = rect.y + (rect.height - h) / 2.0;
        let accent_hex = Palette::hex(self.palette.accent);
        draw_logo_asset(ctx, &self.logo, Rect::new(rect.x, y, w, h), &accent_hex);

        ctx.set_font(&self.font.to_css_font());
        ctx.set_fill_color(&Palette::hex(self.palette.muted));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(&self.text, rect.x + w + FOOTER_MARK_GAP, rect.y + rect.height / 2.0);
    }
}

const MM: f64 = 72.0 / 25.4;

/// Fill a parallelogram (top edge shifted right by `slant` of the width)
/// into the box `(x, y, w, h)` with `color_hex`.
fn fill_parallelogram(ctx: &mut dyn RenderContext, x: f64, y: f64, w: f64, h: f64, color_hex: &str, slant: f64) {
    let offset = w * slant;
    ctx.set_fill_color(color_hex);
    ctx.begin_path();
    ctx.move_to(x + offset, y);
    ctx.line_to(x + w, y);
    ctx.line_to(x + w - offset, y + h);
    ctx.line_to(x, y + h);
    ctx.close_path();
    ctx.fill();
}

/// Band cover, drawn from the front matter: logo top-left,
/// a parallelogram right of the title at 44 % of the page height, the title
/// block from just below the parallelogram's top edge, the author lines
/// bottom-left and the date beside them.
pub struct BandCover {
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub date: Option<String>,
    pub logo: Option<LogoAsset>,
    pub shape_color: u32,
    /// Top-edge shift of the cover parallelogram, as a share of its width.
    pub slant: f64,
    pub palette: Palette,
    pub page_width: f64,
    pub page_height: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub regular_font: FontSpec,
    pub bold_font: FontSpec,
    pub title_leading: f64,
    pub authors_leading: f64,
}

/// Where every element of a [`BandCover`] sits, in page points. The PDF
/// paints from it and the DOCX cover positions its frames from it, so the
/// two covers share one geometry.
pub struct BandCoverPlan {
    pub logo: Rect,
    pub shape: Rect,
    pub title_x: f64,
    pub title_y: f64,
    pub title_width: f64,
    pub authors_x: f64,
    pub authors_width: f64,
    pub authors_bottom: f64,
    pub date_x: f64,
    pub date_y: f64,
}

impl BandCover {
    fn line(&self, text: &str, font: FontSpec, width: f64, leading: f64, shaper: &CosmicShaper) -> ParagraphLayout {
        let run = [StyledRun::new(text, font)];
        layout_paragraph(&Paragraph::new(&run, width).with_line_height(leading), shaper)
    }

    pub fn plan(&self) -> BandCoverPlan {
        let shaper = CosmicShaper::headless();
        let body_width = (self.page_width - self.margin_left - self.margin_right).max(1.0);
        let logo = Rect::new(self.margin_left, self.margin_top, 37.37 * MM, 62.71 * MM);
        let shape_width = 44.65 * MM;
        let shape = Rect::new(self.page_width - self.margin_right - shape_width, self.page_height * 0.44, shape_width, 26.31 * MM);
        let title_y = shape.y + 5.2 * MM;
        let title_width = (shape.x - 3.17 * MM - self.margin_left).max(80.0);
        let authors_width = (106.4 * MM).min(body_width);
        let authors_bottom = self.page_height - self.margin_bottom;
        let authors_height: f64 =
            self.authors.iter().map(|line| self.line(line, self.regular_font, authors_width, self.authors_leading, &shaper).height).sum();
        let date_x = (self.margin_left + 106.5 * MM).min(self.page_width - self.margin_right - 100.0);
        let date_y = if self.authors.is_empty() {
            authors_bottom - self.authors_leading
        } else {
            authors_bottom - authors_height + self.authors_leading
        };
        BandCoverPlan {
            logo,
            shape,
            title_x: self.margin_left,
            title_y,
            title_width,
            authors_x: self.margin_left,
            authors_width,
            authors_bottom,
            date_x,
            date_y,
        }
    }
}

impl TypesetFigure for BandCover {
    fn render(&self, ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {
        let plan = self.plan();
        let shaper = CosmicShaper::headless();
        let ink = Palette::hex(self.palette.ink);

        if let Some(logo) = &self.logo {
            draw_logo_asset(ctx, logo, plan.logo, &Palette::hex(self.palette.accent));
        }
        fill_parallelogram(ctx, plan.shape.x, plan.shape.y, plan.shape.width, plan.shape.height, &Palette::hex(self.shape_color), self.slant);

        let mut y = plan.title_y;
        for text in std::iter::once(self.title.as_str()).chain(self.subtitle.as_deref()) {
            let layout = self.line(text, self.bold_font, plan.title_width, self.title_leading, &shaper);
            draw_paragraph(ctx, (plan.title_x, y), &layout, &ink, false);
            y += layout.height;
        }

        let layouts: Vec<ParagraphLayout> =
            self.authors.iter().map(|line| self.line(line, self.regular_font, plan.authors_width, self.authors_leading, &shaper)).collect();
        let mut y = plan.authors_bottom - layouts.iter().map(|l| l.height).sum::<f64>();
        for layout in &layouts {
            draw_paragraph(ctx, (plan.authors_x, y), layout, &ink, false);
            y += layout.height;
        }

        if let Some(date) = &self.date {
            let layout = self.line(date, self.bold_font, (self.page_width - self.margin_right - plan.date_x).max(1.0), self.authors_leading, &shaper);
            draw_paragraph(ctx, (plan.date_x, plan.date_y), &layout, &ink, false);
        }
    }
}

/// The `N ▰ M` folio: the page number, a small filled parallelogram, and
/// the page count, right-aligned to the figure's rect.
pub struct PageMark {
    pub current: String,
    pub last: String,
    pub font: FontSpec,
    pub ink: u32,
    pub mark: u32,
    /// Top-edge shift of the folio parallelogram, as a share of its width.
    pub slant: f64,
}

impl TypesetFigure for PageMark {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let shaper = CosmicShaper::headless();
        let (last_width, _) = paragraph_intrinsic_size(&self.last, &self.font, f64::MAX, &shaper);
        let mark_width = 4.80 * MM;
        let mark_height = 2.79 * MM;
        let gap = self.font.size_px * 0.45;
        let right = rect.x + rect.width;
        let center_y = rect.y + rect.height / 2.0;
        let mark_x = right - last_width - gap - mark_width;

        ctx.set_font(&self.font.to_css_font());
        ctx.set_text_align(TextAlign::Right);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.set_fill_color(&Palette::hex(self.ink));
        ctx.fill_text(&self.last, right, center_y);
        ctx.fill_text(&self.current, mark_x - gap, center_y);
        fill_parallelogram(ctx, mark_x, center_y - mark_height / 2.0, mark_width, mark_height, &Palette::hex(self.mark), self.slant);
    }
}

/// The fill and the left bar behind one page's worth of a `>>` / `>>>`
/// block. It carries no text: the paragraphs, lists and tables of the block
/// are ordinary flow blocks that the engine breaks across pages, and this
/// figure is placed behind them afterwards (`Press::decorate_insets`).
pub struct InsetBackdrop {
    pub fill: u32,
    pub bar: u32,
    pub bar_width: f64,
}

impl TypesetFigure for InsetBackdrop {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        ctx.set_fill_color(&Palette::hex(self.fill));
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        ctx.set_fill_color(&Palette::hex(self.bar));
        ctx.fill_rect(rect.x, rect.y, self.bar_width, rect.height);
    }
}

/// A `>` note of the tinted kind: a light fill, a bar on the left edge, and
/// the already laid-out paragraph inside.
pub struct CalloutBox {
    pub layout: ParagraphLayout,
    pub fill: u32,
    pub bar: u32,
    pub bar_width: f64,
    pub pad_h: f64,
    pub pad_v: f64,
    pub ink: u32,
}

impl TypesetFigure for CalloutBox {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        ctx.set_fill_color(&Palette::hex(self.fill));
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        ctx.set_fill_color(&Palette::hex(self.bar));
        ctx.fill_rect(rect.x, rect.y, self.bar_width, rect.height);
        draw_paragraph(ctx, (rect.x + self.bar_width + self.pad_h, rect.y + self.pad_v), &self.layout, &Palette::hex(self.ink), false);
    }
}
