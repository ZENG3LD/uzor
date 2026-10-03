//! `parse::Document` -> `word/document.xml` body content — the AST -> OOXML
//! mapping table from the DOCX plan. Every helper here builds a fragment of
//! `<w:body>` inner XML as a `String`; [`build_document_body`] is the only
//! `pub(crate)` entry point.

use uzor::fonts::FontFamily;

use uzor::types::Rect;

use crate::figures::BandCover;
use crate::parse::{
    Block as AstBlock, ColumnAlign, CoverLike, DiagramBox, Document, FrontMatter, ImageSpec, InlineRun, InsetKind, InsetPart, IslandSide,
};
use crate::press::{
    is_total_row, CalloutStyle, InsetPaint, Press, INSET_HANG_LEFT, INSET_HANG_RIGHT, INSET_PAD_V,
};

use super::error::DocxError;
use super::media::{self, MediaRegistry};
use super::numbering::{NumberingRegistry, LIST_MARKER_OFFSET_PT};
use super::units::{docx_font_name, escape_xml_text, ooxml_hex, pt_to_emu, pt_to_half_points, pt_to_twips};

/// Shared mutable state threaded through one `build_document_body` call —
/// the image/diagram-logo relationship registry and the list numId
/// allocator, plus the one geometry number (`body_width_pt`) every block
/// kind needs.
pub(crate) struct DocxCtx<'a> {
    press: &'a Press,
    media: MediaRegistry,
    numbering: NumberingRegistry,
    body_width_pt: f64,
    /// `true` while nothing but a page break (or the start of the document)
    /// precedes the next block, so a heading that asks for a page break
    /// before itself must not add a second one.
    at_page_top: bool,
}

/// The font name Word is asked for: the theme's `docx_font`, else the
/// bundled family's name.
fn face(press: &Press, family: FontFamily) -> String {
    docx_font_name(press.palette(), family)
}

impl<'a> DocxCtx<'a> {
    pub(crate) fn new(press: &'a Press) -> Self {
        Self { press, media: MediaRegistry::new(), numbering: NumberingRegistry::new(), body_width_pt: press.body_width(), at_page_top: true }
    }

    /// Consumes the context, handing its own media registry (document.xml's
    /// own images) and numbering registry (`word/numbering.xml`) to the
    /// caller for final assembly.
    pub(crate) fn into_media_and_numbering(self) -> (MediaRegistry, NumberingRegistry) {
        (self.media, self.numbering)
    }
}

/// The whole `<w:body>` inner content: the optional cover title page (its
/// own page, ended by an explicit page break), then every `---`-separated
/// section's blocks (a page break inserted between sections, mirroring
/// `press.rs::build_doc`'s own `BreakControl::ForceBefore` at every
/// section boundary after the first).
pub(crate) fn build_document_body(document: &Document, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let mut xml = String::new();
    if let Some(cover) = ctx.press.band_cover_data(document) {
        xml.push_str(&band_cover_xml(&cover, &document.front_matter, ctx)?);
        xml.push_str(page_break_xml());
    } else if let Some(cover) = &document.cover {
        xml.push_str(&cover_title_page_xml(cover, &document.front_matter, ctx)?);
        xml.push_str(page_break_xml());
    } else if let Some(label) = document.front_matter.badge.as_deref() {
        let palette = ctx.press.palette();
        let sizes = ctx.press.sizes();
        let run = runs_xml(&[InlineRun::Plain(label.to_owned())], &face(ctx.press, palette.font_mono), sizes.footer, palette.accent, true, false);
        xml.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr>{run}</w:p>"#));
    }
    ctx.at_page_top = true;
    for (section_idx, section) in document.sections.iter().enumerate() {
        if section_idx > 0 {
            xml.push_str(page_break_xml());
            ctx.at_page_top = true;
        }
        for (block_idx, block) in section.blocks.iter().enumerate() {
            let block_xml = block_to_xml(block, section.blocks.get(block_idx + 1), ctx)?;
            if !block_xml.is_empty() {
                ctx.at_page_top = false;
            }
            xml.push_str(&block_xml);
        }
    }
    Ok(xml)
}

/// A page break in a paragraph of its own. The paragraph mark that follows
/// the break lands at the top of the next page, so the paragraph is one
/// point tall rather than a whole empty line.
fn page_break_xml() -> &'static str {
    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/></w:pPr><w:r><w:br w:type="page"/></w:r></w:p>"#
}

/// `next` is the block that follows in the same section: a picture after a
/// `>>>` block keeps with it.
fn block_to_xml(block: &AstBlock, next: Option<&AstBlock>, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    match block {
        AstBlock::Heading { level, runs } => Ok(heading_xml(*level, runs, ctx)),
        AstBlock::Paragraph(runs) => Ok(paragraph_xml(runs, ctx)),
        AstBlock::List { ordered, items } => Ok(list_xml(*ordered, items, ctx)),
        AstBlock::Callout(runs) => Ok(callout_xml(runs, ctx)),
        AstBlock::Inset { kind, parts } => {
            let keep_with_next = *kind == InsetKind::Technical && matches!(next, Some(AstBlock::Image(_)));
            Ok(inset_xml(*kind, parts, keep_with_next, ctx))
        }
        AstBlock::Table { header, rows, fractions, aligns } => Ok(table_xml(header, rows, fractions, aligns, ctx)),
        AstBlock::Image(spec) => image_xml(spec, ctx),
        AstBlock::Kpi(items) => Ok(kpi_table_xml(items, ctx)),
        AstBlock::Diagram(rows) => diagram_xml(rows, ctx),
        AstBlock::Closing(cover) => closing_xml(cover, ctx),
        AstBlock::Spacer(n) => Ok(spacer_xml(*n)),
        AstBlock::Clear => Ok(String::new()),
    }
}

/// `Plain` runs at `size`/`color`; `Bold` runs (or `force_bold`) additionally
/// get `<w:b/>`. Empty `runs` still emits one empty, `xml:space="preserve"`
/// run so a block never collapses to a completely childless paragraph.
fn runs_xml(runs: &[InlineRun], font: &str, size_pt: f64, color: u32, force_bold: bool, italic: bool) -> String {
    if runs.is_empty() {
        return one_run_xml("", font, size_pt, color, force_bold, italic);
    }
    let mut xml = String::new();
    for run in runs {
        let (text, is_bold) = match run {
            InlineRun::Plain(t) => (t.as_str(), false),
            InlineRun::Bold(t) => (t.as_str(), true),
        };
        xml.push_str(&one_run_xml(text, font, size_pt, color, force_bold || is_bold, italic));
    }
    xml
}

fn one_run_xml(text: &str, font: &str, size_pt: f64, color: u32, bold: bool, italic: bool) -> String {
    let half_points = pt_to_half_points(size_pt);
    let mut rpr = format!(r#"<w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:cs="{font}"/>"#);
    if bold {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if italic {
        rpr.push_str("<w:i/><w:iCs/>");
    }
    rpr.push_str(&format!(r#"<w:color w:val="{}"/><w:sz w:val="{half_points}"/><w:szCs w:val="{half_points}"/>"#, ooxml_hex(color)));
    let escaped = escape_xml_text(text);
    let t = if text.is_empty() || text.starts_with(' ') || text.ends_with(' ') {
        format!(r#"<w:t xml:space="preserve">{escaped}</w:t>"#)
    } else {
        format!("<w:t>{escaped}</w:t>")
    };
    format!("<w:r><w:rPr>{rpr}</w:rPr>{t}</w:r>")
}

fn heading_xml(level: u8, runs: &[InlineRun], ctx: &DocxCtx) -> String {
    let style_id = match level {
        1 => "Heading1",
        2 => "Heading2",
        _ => "Heading3",
    };
    let sizes = ctx.press.sizes();
    let size = match level {
        1 => sizes.h1,
        2 => sizes.h2,
        _ => sizes.h3,
    };
    let palette = ctx.press.palette();
    let extras = &ctx.press.sheet().extras;
    let space = extras.heading_space[usize::from(level.clamp(1, 3) - 1)];

    // `pPr` children in schema order: pStyle, pageBreakBefore, spacing.
    let mut ppr = format!(r#"<w:pStyle w:val="{style_id}"/>"#);
    if level == 1 && extras.h1_page_break && !ctx.at_page_top {
        ppr.push_str("<w:pageBreakBefore/>");
    }
    let mut spacing = String::new();
    if space.before > 0.0 || space.after > 0.0 {
        spacing.push_str(&format!(r#" w:before="{}" w:after="{}""#, pt_to_twips(space.before), pt_to_twips(space.after)));
    }
    if let Some(leading) = extras.heading_leading {
        spacing.push_str(&format!(r#" w:line="{}" w:lineRule="exact""#, pt_to_twips(leading)));
    }
    if !spacing.is_empty() {
        ppr.push_str(&format!("<w:spacing{spacing}/>"));
    }
    let body = runs_xml(runs, &face(ctx.press, palette.font_display), size, palette.ink, true, false);
    format!(r#"<w:p><w:pPr>{ppr}</w:pPr>{body}</w:p>"#)
}

/// Doc/report body paragraphs justify (Word's own `<w:jc w:val="both"/>`,
/// the native analogue of the PDF path's own `ParagraphAlign::Justify` +
/// Russian hyphenation); `deck` stays left-aligned, matching
/// `press.rs::push_paragraph`. `pPr` children are written in schema order
/// (spacing, ind, jc).
fn paragraph_xml(runs: &[InlineRun], ctx: &DocxCtx) -> String {
    let sheet = ctx.press.sheet();
    let look = ParaLook {
        frame: "",
        size: ctx.press.sizes().body,
        leading: Some(sheet.body_leading),
        after: Some(pt_to_twips(sheet.paragraph_spacing)),
        keep_next: false,
    };
    paragraph_props_xml(runs, ctx, &look)
}

/// How one paragraph, or one list, is set. `frame` is border and shading XML
/// (empty outside a `>>` / `>>>` block); it goes where the schema puts
/// `pBdr`/`shd`.
struct ParaLook<'a> {
    frame: &'a str,
    /// Font size, pt.
    size: f64,
    /// Exact line, pt. A list on a preset with no list line sets none.
    leading: Option<f64>,
    /// Space after, twips. For a list: on the last item only, the sheet's
    /// paragraph gap elsewhere.
    after: Option<i64>,
    /// Keep with the next paragraph (the last item, for a list).
    keep_next: bool,
}

fn paragraph_props_xml(runs: &[InlineRun], ctx: &DocxCtx, look: &ParaLook) -> String {
    let palette = ctx.press.palette();
    let sheet = ctx.press.sheet();
    let jc = if sheet.justify { r#"<w:jc w:val="both"/>"# } else { "" };
    let after = look.after.unwrap_or_else(|| pt_to_twips(sheet.paragraph_spacing));
    let line = pt_to_twips(look.leading.unwrap_or(sheet.body_leading));
    let indent = sheet.first_indent;
    let ind = if indent > 0.0 { format!(r#"<w:ind w:firstLine="{}"/>"#, pt_to_twips(indent)) } else { String::new() };
    let keep = if look.keep_next { "<w:keepNext/>" } else { "" };
    let frame = look.frame;
    let body = runs_xml(runs, &face(ctx.press, ctx.press.text_family()), look.size, palette.ink, false, false);
    format!(r#"<w:p><w:pPr>{keep}{frame}<w:spacing w:after="{after}" w:line="{line}" w:lineRule="exact"/>{ind}{jc}</w:pPr>{body}</w:p>"#)
}

fn list_xml(ordered: bool, items: &[Vec<InlineRun>], ctx: &mut DocxCtx) -> String {
    let look = ParaLook { frame: "", size: ctx.press.sizes().list_item, leading: ctx.press.sheet().extras.list_leading, after: None, keep_next: false };
    list_props_xml(ordered, items, ctx, &look)
}

fn list_props_xml(ordered: bool, items: &[Vec<InlineRun>], ctx: &mut DocxCtx, look: &ParaLook) -> String {
    let num_id = ctx.numbering.allocate(ordered);
    let palette = ctx.press.palette();
    let sheet = ctx.press.sheet();
    // Refined lists: exact leading, the paragraph gap only after the last
    // item (contextual spacing), justified like the body.
    let spacing = |after: i64, overridden: bool| match look.leading {
        Some(leading) => {
            format!(r#"<w:spacing w:after="{after}" w:line="{}" w:lineRule="exact"/><w:contextualSpacing/>"#, pt_to_twips(leading))
        }
        None if overridden => format!(r#"<w:spacing w:after="{after}"/>"#),
        None => String::new(),
    };
    let jc = if sheet.extras.justify_lists && sheet.justify { r#"<w:jc w:val="both"/>"# } else { "" };
    let frame = look.frame;
    let mut xml = String::new();
    for (index, item) in items.iter().enumerate() {
        let is_last = index + 1 == items.len();
        let after_numpr = match look.after {
            Some(after) if is_last => spacing(after, true),
            _ => spacing(pt_to_twips(sheet.paragraph_spacing), false),
        };
        let keep = if look.keep_next && is_last { "<w:keepNext/>" } else { "" };
        let body = runs_xml(item, &face(ctx.press, ctx.press.text_family()), look.size, palette.ink, false, false);
        xml.push_str(&format!(
            r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/>{keep}<w:numPr><w:ilvl w:val="0"/><w:numId w:val="{num_id}"/></w:numPr>{frame}{after_numpr}{jc}</w:pPr>{body}</w:p>"#
        ));
    }
    xml
}

/// Border and shading of a paragraph inside a `>>` / `>>>` block: the fill,
/// a bar on the left and thin fill-coloured borders on the other sides, which
/// pad the block and reach the fill past the text column. Paragraphs with the
/// same frame join into one box in Word, so the fill runs unbroken between
/// them. `extra_left` moves the bar further out for a paragraph whose border
/// Word draws at an indent (list items: at the marker), so every bar of a
/// block stands on one line.
fn inset_frame_xml(paint: &InsetPaint, extra_left: f64) -> String {
    let fill = ooxml_hex(paint.fill);
    let bar = ooxml_hex(paint.bar);
    let bar_eighths = (paint.bar_width * 8.0).round() as i64;
    // `w:space` is whole points, 0..=31.
    let space = |pt: f64| pt.round().clamp(0.0, 31.0) as i64;
    let (left, right, vertical) = (space(INSET_HANG_LEFT + extra_left), space(INSET_HANG_RIGHT), space(INSET_PAD_V));
    format!(
        r#"<w:pBdr><w:top w:val="single" w:sz="4" w:space="{vertical}" w:color="{fill}"/><w:left w:val="single" w:sz="{bar_eighths}" w:space="{left}" w:color="{bar}"/><w:bottom w:val="single" w:sz="4" w:space="{vertical}" w:color="{fill}"/><w:right w:val="single" w:sz="4" w:space="{right}" w:color="{fill}"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="{fill}"/>"#
    )
}

/// `>>` / `>>>` block: an optional label paragraph (kept with the first
/// part), then the parts as ordinary paragraphs, list items and tables, each
/// shaded with a bar on the left. Revised text is set like the body, a
/// technical addition at the callout size. A paragraph gap sits between two
/// paragraphs (inside the shaded box) and after the block; where a paragraph
/// meets a list or a table the gap is dropped and the borders' own padding
/// separates the two, so the fill has no white seam. `keep_with_next` keeps
/// the block's last paragraph with the next one (a picture that follows).
fn inset_xml(kind: InsetKind, parts: &[InsetPart], keep_with_next: bool, ctx: &mut DocxCtx) -> String {
    let press = ctx.press;
    let palette = press.palette();
    let sheet = press.sheet();
    let paint = palette.inset_paint(kind);
    let sizes = press.inset_sizes(kind);
    let gap = pt_to_twips(sheet.paragraph_spacing);
    let gap_before = |next: Option<&InsetPart>| match next {
        Some(InsetPart::List { .. } | InsetPart::Table { .. }) => 0,
        Some(InsetPart::Paragraph(_)) | None => gap,
    };
    let text_frame = inset_frame_xml(&paint, 0.0);
    let list_frame = inset_frame_xml(&paint, LIST_MARKER_OFFSET_PT);

    let mut xml = String::new();
    if let Some(label) = paint.label.as_ref() {
        let leading = pt_to_twips(sheet.body_leading * label.size / press.sizes().body);
        let run = runs_xml(&[InlineRun::Plain(label.text.clone())], &face(press, palette.font_display), label.size, paint.bar, true, false);
        xml.push_str(&format!(
            r#"<w:p><w:pPr><w:keepNext/>{text_frame}<w:spacing w:before="0" w:after="{}" w:line="{leading}" w:lineRule="exact"/></w:pPr>{run}</w:p>"#,
            gap_before(parts.first())
        ));
    }
    for (index, part) in parts.iter().enumerate() {
        let after = Some(gap_before(parts.get(index + 1)));
        let keep_next = keep_with_next && index + 1 == parts.len();
        match part {
            InsetPart::Paragraph(runs) => {
                let look = ParaLook { frame: &text_frame, size: sizes.text, leading: Some(sizes.text_leading), after, keep_next };
                xml.push_str(&paragraph_props_xml(runs, ctx, &look));
            }
            InsetPart::List { ordered, items } => {
                let look = ParaLook { frame: &list_frame, size: sizes.item, leading: sizes.item_leading, after, keep_next };
                xml.push_str(&list_props_xml(*ordered, items, ctx, &look));
            }
            InsetPart::Table { header, rows, fractions, aligns } => {
                xml.push_str(&table_xml_with(header, rows, fractions, aligns, ctx, Some(&paint)));
            }
        }
    }
    xml
}

/// `CalloutStyle::Boxed`: a 1x1 borderless-except-accent `<w:tbl>` — the
/// closest native equivalent of `push_callout`'s own single-cell `TableBlock`
/// (accent gridlines, `panel` fill). `CalloutStyle::Tinted`: one paragraph with
/// a light shading, a thick left border in the bar colour, and thin
/// fill-coloured top/bottom borders that pad the text block.
fn callout_xml(runs: &[InlineRun], ctx: &DocxCtx) -> String {
    let palette = ctx.press.palette();
    let sizes = ctx.press.sizes();
    let font = face(ctx.press, palette.font_display);
    match palette.doc.callout {
        CalloutStyle::Boxed => {
            let body_twips = pt_to_twips(ctx.body_width_pt);
            let body = runs_xml(runs, &font, sizes.callout, palette.ink, false, false);
            let border = ooxml_hex(palette.accent);
            let fill = ooxml_hex(palette.panel);
            format!(
                r#"<w:tbl><w:tblPr><w:tblW w:w="{body_twips}" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="8" w:color="{border}"/><w:left w:val="single" w:sz="8" w:color="{border}"/><w:bottom w:val="single" w:sz="8" w:color="{border}"/><w:right w:val="single" w:sz="8" w:color="{border}"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="{body_twips}"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="{body_twips}" w:type="dxa"/><w:shd w:val="clear" w:fill="{fill}"/><w:tcMar><w:top w:w="200" w:type="dxa"/><w:left w:w="240" w:type="dxa"/><w:bottom w:w="200" w:type="dxa"/><w:right w:w="240" w:type="dxa"/></w:tcMar></w:tcPr><w:p>{body}</w:p></w:tc></w:tr></w:tbl>"#
            )
        }
        CalloutStyle::Tinted { fill, bar, bar_width } => {
            let sheet = ctx.press.sheet();
            let leading = pt_to_twips(sheet.body_leading * sizes.callout / sizes.body);
            let after = pt_to_twips(sheet.paragraph_spacing);
            let body = runs_xml(runs, &font, sizes.callout, palette.ink, false, false);
            let (fill, bar) = (ooxml_hex(fill), ooxml_hex(bar));
            let bar_eighths = (bar_width * 8.0).round() as i64;
            format!(
                r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="4" w:color="{fill}"/><w:left w:val="single" w:sz="{bar_eighths}" w:space="6" w:color="{bar}"/><w:bottom w:val="single" w:sz="4" w:space="4" w:color="{fill}"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="{fill}"/><w:spacing w:before="0" w:after="{after}" w:line="{leading}" w:lineRule="exact"/><w:ind w:left="160" w:right="160"/></w:pPr>{body}</w:p>"#
            )
        }
    }
}

/// `fractions` scaled to twips summing to EXACTLY `total_twips` — the last
/// column absorbs the flooring remainder, so a table's own `tblGrid` can
/// never declare a width wider (or narrower) than the page.
fn column_widths_twips(fractions: &[f64], total_twips: i64) -> Vec<i64> {
    let sum: f64 = fractions.iter().sum::<f64>().max(1e-9);
    let mut widths: Vec<i64> = fractions.iter().map(|f| ((f / sum) * total_twips as f64).floor() as i64).collect();
    let assigned: i64 = widths.iter().sum();
    if let Some(last) = widths.last_mut() {
        *last += total_twips - assigned;
    }
    widths
}

fn column_align_xml(align: Option<&ColumnAlign>) -> &'static str {
    match align {
        Some(ColumnAlign::Center) => r#"<w:jc w:val="center"/>"#,
        Some(ColumnAlign::Right) => r#"<w:jc w:val="right"/>"#,
        _ => r#"<w:jc w:val="left"/>"#,
    }
}

/// `sz` is the line width in eighths of a point (4 = 0.5 pt). `left_bar`
/// (colour, eighths of a point) replaces the left edge with a bar.
fn table_borders_xml(color: u32, rules: uzor_typeset::TableRules, sz: i64, left_bar: Option<(u32, i64)>) -> String {
    let hex = ooxml_hex(color);
    let side = |name: &str, on: bool| {
        if on {
            format!(r#"<w:{name} w:val="single" w:sz="{sz}" w:color="{hex}"/>"#)
        } else {
            format!(r#"<w:{name} w:val="nil"/>"#)
        }
    };
    let vertical = matches!(rules, uzor_typeset::TableRules::Box);
    ["top", "left", "bottom", "right", "insideH", "insideV"]
        .into_iter()
        .map(|name| match (name, left_bar) {
            ("left", Some((bar, bar_sz))) => format!(r#"<w:left w:val="single" w:sz="{bar_sz}" w:color="{}"/>"#, ooxml_hex(bar)),
            _ => {
                let on = match name {
                    "left" | "right" | "insideV" => vertical,
                    _ => true,
                };
                side(name, on)
            }
        })
        .collect()
}

/// Header row repeats on every page (`w:tblHeader`), keeps with the first body
/// row (`w:keepNext` in its paragraphs) and no row splits across pages
/// (`w:cantSplit`), as in the PDF. Rows the theme recognises as totals
/// get its total fill and bold text.
fn table_xml(header: &[Vec<InlineRun>], rows: &[Vec<Vec<InlineRun>>], fractions: &[f64], aligns: &[ColumnAlign], ctx: &DocxCtx) -> String {
    table_xml_with(header, rows, fractions, aligns, ctx, None)
}

/// [`table_xml`] inside a `>>` / `>>>` block when `revised` is set: body
/// cells that carry no fill of their own take the block's fill, and the left
/// edge is the block's bar.
fn table_xml_with(
    header: &[Vec<InlineRun>],
    rows: &[Vec<Vec<InlineRun>>],
    fractions: &[f64],
    aligns: &[ColumnAlign],
    ctx: &DocxCtx,
    revised: Option<&InsetPaint>,
) -> String {
    let body_twips = pt_to_twips(ctx.body_width_pt);
    let widths = column_widths_twips(fractions, body_twips);
    let palette = ctx.press.palette();
    let sizes = ctx.press.sizes();
    let sheet = ctx.press.sheet();
    let doc = &palette.doc;
    let font = face(ctx.press, palette.font_mono);
    let rule_color = doc.table_rule.map_or(palette.line, |rule| rule.color);
    let rule_eighths = doc.table_rule.map_or(4, |rule| (rule.width * 8.0).round() as i64);
    let head_fill = ooxml_hex(doc.table_head_fill.unwrap_or(palette.panel));

    let mut xml = String::new();
    // `tblPr` children in schema order: tblW, tblBorders, tblLayout, tblCellMar.
    let mut table_props = format!(
        r#"<w:tblW w:w="{body_twips}" w:type="dxa"/><w:tblBorders>{}</w:tblBorders>"#,
        table_borders_xml(rule_color, sheet.table_rules, rule_eighths, revised.map(|r| (r.bar, (r.bar_width * 8.0).round() as i64)))
    );
    if sheet.extras.docx_cell_margins {
        let (h, v) = (pt_to_twips(sheet.cell_padding.h), pt_to_twips(sheet.cell_padding.v));
        table_props.push_str(&format!(
            r#"<w:tblLayout w:type="fixed"/><w:tblCellMar><w:top w:w="{v}" w:type="dxa"/><w:left w:w="{h}" w:type="dxa"/><w:bottom w:w="{v}" w:type="dxa"/><w:right w:w="{h}" w:type="dxa"/></w:tblCellMar>"#
        ));
    }
    xml.push_str(&format!("<w:tbl><w:tblPr>{table_props}</w:tblPr><w:tblGrid>"));
    for w in &widths {
        xml.push_str(&format!(r#"<w:gridCol w:w="{w}"/>"#));
    }
    xml.push_str("</w:tblGrid>");

    xml.push_str("<w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>");
    for (i, cell_runs) in header.iter().enumerate() {
        let align = column_align_xml(aligns.get(i));
        let body = runs_xml(cell_runs, &font, sizes.table_header, palette.ink, true, false);
        xml.push_str(&format!(
            r#"<w:tc><w:tcPr><w:tcW w:w="{}" w:type="dxa"/><w:shd w:val="clear" w:fill="{head_fill}"/></w:tcPr><w:p><w:pPr><w:keepNext/>{align}</w:pPr>{body}</w:p></w:tc>"#,
            widths.get(i).copied().unwrap_or(0)
        ));
    }
    xml.push_str("</w:tr>");

    for row in rows {
        let total_fill = doc.table_total_fill.filter(|_| is_total_row(row, &doc.total_prefixes));
        xml.push_str("<w:tr><w:trPr><w:cantSplit/></w:trPr>");
        for (i, cell_runs) in row.iter().enumerate() {
            let align = column_align_xml(aligns.get(i));
            let body = runs_xml(cell_runs, &font, sizes.table_cell, palette.ink, total_fill.is_some(), false);
            let cell_fill = total_fill.or(revised.map(|r| r.fill));
            let shd = cell_fill.map_or_else(String::new, |fill| format!(r#"<w:shd w:val="clear" w:fill="{}"/>"#, ooxml_hex(fill)));
            xml.push_str(&format!(
                r#"<w:tc><w:tcPr><w:tcW w:w="{}" w:type="dxa"/>{shd}</w:tcPr><w:p><w:pPr>{align}</w:pPr>{body}</w:p></w:tc>"#,
                widths.get(i).copied().unwrap_or(0)
            ));
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

/// `:::kpi` -> a bordered `<w:tbl>` — 2 columns for 1-2 items, else 3;
/// tiles fill row-major, a short last row padded with empty (still
/// bordered) cells so the grid stays rectangular.
fn kpi_table_xml(items: &[(String, String)], ctx: &DocxCtx) -> String {
    // 1 item -> 1 column, 2 items -> 2 columns, 3+ items -> 3 columns.
    let cols = items.len().clamp(1, 3);
    let palette = ctx.press.palette();
    let sizes = ctx.press.sizes();
    let body_twips = pt_to_twips(ctx.body_width_pt);
    let widths = column_widths_twips(&vec![1.0; cols], body_twips);

    let mut xml = String::new();
    xml.push_str(&format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="{body_twips}" w:type="dxa"/><w:tblBorders>{}</w:tblBorders></w:tblPr><w:tblGrid>"#,
        table_borders_xml(palette.line, uzor_typeset::TableRules::Box, 4, None)
    ));
    for w in &widths {
        xml.push_str(&format!(r#"<w:gridCol w:w="{w}"/>"#));
    }
    xml.push_str("</w:tblGrid>");

    for chunk in items.chunks(cols) {
        xml.push_str("<w:tr>");
        for (i, width) in widths.iter().enumerate() {
            match chunk.get(i) {
                Some((value, label)) => {
                    let value_xml = runs_xml(&[InlineRun::Plain(value.clone())], &face(ctx.press, palette.font_mono), sizes.kpi_value, palette.accent, true, false);
                    let label_xml = runs_xml(&[InlineRun::Plain(label.clone())], &face(ctx.press, palette.font_mono), sizes.kpi_label, palette.muted, false, false);
                    xml.push_str(&format!(
                        r#"<w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/><w:shd w:val="clear" w:fill="{}"/></w:tcPr><w:p><w:pPr><w:jc w:val="center"/></w:pPr>{value_xml}</w:p><w:p><w:pPr><w:jc w:val="center"/></w:pPr>{label_xml}</w:p></w:tc>"#,
                        ooxml_hex(palette.panel)
                    ));
                }
                None => xml.push_str(&format!(r#"<w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/></w:tcPr><w:p/></w:tc>"#)),
            }
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

/// `pub(crate)` — [`super::section::build_footer_xml`] embeds the footer
/// brand mark through the exact same inline-picture XML shape.
pub(crate) fn inline_drawing_xml(rel_id: &str, width_pt: f64, height_pt: f64) -> String {
    let cx = pt_to_emu(width_pt);
    let cy = pt_to_emu(height_pt);
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="1" name="Picture"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="Picture"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

/// `wrap_text_side`: which side TEXT flows on (opposite the image's own
/// `align_side`) — an `island=left` image sits at the left margin and
/// text wraps on its RIGHT; `island=right` is the mirror.
fn anchored_drawing_xml(rel_id: &str, width_pt: f64, height_pt: f64, align_side: &str, wrap_text_side: &str) -> String {
    let cx = pt_to_emu(width_pt);
    let cy = pt_to_emu(height_pt);
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="1" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="margin"><wp:align>{align_side}</wp:align></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:wrapSquare wrapText="{wrap_text_side}"/><wp:docPr id="2" name="Picture"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="Picture"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

fn caption_xml(text: &str, press: &Press) -> String {
    let palette = press.palette();
    let sizes = press.sizes();
    let body = runs_xml(&[InlineRun::Plain(text.to_owned())], &face(press, palette.font_display), sizes.caption, palette.muted, false, true);
    format!(r#"<w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{body}</w:p>"#)
}

/// Non-island: inline, contain-fit within `(body_width_pt, spec.height or
/// the format default)`, never exceeding `body_width_pt`. Island: native
/// Word text-wrap-around-image via `<wp:anchor>` — the FOLLOWING
/// paragraphs simply flow in the body text stream and Word wraps them
/// automatically (`:::clear`/`Block::Clear` is a documented no-op here,
/// same as the PDF path).
fn image_xml(spec: &ImageSpec, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let (pixel_w, pixel_h) = ctx.press.image_dimensions(&spec.path).map_err(|source| DocxError::Image { path: spec.path.clone(), source })?;
    let bytes = ctx.press.read_image_bytes(&spec.path).map_err(|source| DocxError::Image { path: spec.path.clone(), source })?;
    let rel_id = ctx.media.register_png(bytes);
    let aspect = (pixel_w as f64 / (pixel_h.max(1) as f64)).max(0.0001);

    let (drawing, centered) = if let Some(side) = spec.island {
        let width_pt = ctx.body_width_pt * spec.width.unwrap_or(0.45);
        let height_pt = width_pt / aspect;
        let (align_side, wrap_text_side) = match side {
            IslandSide::Left => ("left", "right"),
            IslandSide::Right => ("right", "left"),
        };
        (anchored_drawing_xml(&rel_id, width_pt, height_pt, align_side, wrap_text_side), false)
    } else {
        let target_h = spec.height.unwrap_or_else(|| ctx.press.format().image_default_height());
        let mut width_pt = target_h * aspect;
        let mut height_pt = target_h;
        if width_pt > ctx.body_width_pt {
            width_pt = ctx.body_width_pt;
            height_pt = width_pt / aspect;
        }
        (inline_drawing_xml(&rel_id, width_pt, height_pt), true)
    };

    let jc = if centered { r#"<w:jc w:val="center"/>"# } else { "" };
    // A captioned picture keeps with its caption.
    let keep = if centered && spec.caption.is_some() { "<w:keepNext/>" } else { "" };
    let mut out = format!("<w:p><w:pPr>{keep}{jc}</w:pPr>{drawing}</w:p>");
    if let Some(caption) = &spec.caption {
        out.push_str(&caption_xml(caption, ctx.press));
    }
    Ok(out)
}

fn diagram_xml(rows: &[Vec<DiagramBox>], ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let (bytes, width_pt, height_pt) = media::diagram_png(ctx.press, rows)?;
    let rel_id = ctx.media.register_png(bytes);
    Ok(format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#, inline_drawing_xml(&rel_id, width_pt, height_pt)))
}

fn spacer_xml(n: f64) -> String {
    format!(r#"<w:p><w:pPr><w:spacing w:after="{}"/></w:pPr></w:p>"#, pt_to_twips(n))
}

/// `:::closing` — centered `PressCoverTitle`(H1-sized)/`PressCoverSubtitle`/
/// site paragraphs, optional image above (Closing is already single-column
/// centered in the PDF path, unlike the 2-column cover).
fn closing_xml(cover: &CoverLike, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let palette = ctx.press.palette();
    let sizes = ctx.press.sizes();
    let mut xml = String::new();

    if let Some(image_path) = &cover.image {
        let (pixel_w, pixel_h) = ctx.press.image_dimensions(image_path).map_err(|source| DocxError::Image { path: image_path.clone(), source })?;
        let bytes = ctx.press.read_image_bytes(image_path).map_err(|source| DocxError::Image { path: image_path.clone(), source })?;
        let rel_id = ctx.media.register_png(bytes);
        let aspect = (pixel_w as f64 / (pixel_h.max(1) as f64)).max(0.0001);
        let target_h = ctx.press.format().image_default_height();
        let mut width_pt = target_h * aspect;
        let mut height_pt = target_h;
        if width_pt > ctx.body_width_pt {
            width_pt = ctx.body_width_pt;
            height_pt = width_pt / aspect;
        }
        xml.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#, inline_drawing_xml(&rel_id, width_pt, height_pt)));
    }

    let title = runs_xml(&[InlineRun::Plain(cover.title.clone())], &face(ctx.press, palette.font_display), sizes.h1, palette.ink, true, false);
    xml.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverTitle"/><w:jc w:val="center"/></w:pPr>{title}</w:p>"#));
    let subtitle = runs_xml(&[InlineRun::Plain(cover.subtitle.clone())], &face(ctx.press, palette.font_display), sizes.cover_sub, palette.ink, false, false);
    xml.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverSubtitle"/><w:jc w:val="center"/></w:pPr>{subtitle}</w:p>"#));
    let site = runs_xml(&[InlineRun::Plain(cover.site.clone())], &face(ctx.press, palette.font_display), sizes.footer, palette.muted, false, false);
    xml.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverMeta"/><w:jc w:val="center"/></w:pPr>{site}</w:p>"#));
    Ok(xml)
}

/// `Document.cover` -> one borderless, 1-row, 2-column `<w:tbl>` title
/// page. Left cell: corner `logo` (if any) + `PressCoverTitle` +
/// `PressCoverSubtitle` + an accent bottom-border rule (a full-cell-width
/// empty paragraph border — Divergence 1: not the PDF's own 96pt stub) +
/// `PressCoverMeta` + `site`. Right cell: `badge` (bold accent text,
/// right-aligned, top) then `cover.image`, vertically centered by
/// `<w:vAlign w:val="center"/>` on the cell.
fn cover_title_page_xml(cover: &CoverLike, front_matter: &FrontMatter, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let palette = ctx.press.palette().clone();
    let sizes = *ctx.press.sizes();
    let body_twips = pt_to_twips(ctx.body_width_pt);
    let left_twips = (body_twips as f64 * 0.58).round() as i64;
    let right_twips = body_twips - left_twips;

    let mut left = String::new();
    if let Some(logo_path) = front_matter.logo.as_deref() {
        let (bytes, w, h) = media::resolve_logo_png(ctx.press, logo_path)?;
        let rel_id = ctx.media.register_png(bytes);
        let logo_h_pt = ctx.press.format().logo_height();
        let logo_w_pt = logo_h_pt * (w as f64 / (h.max(1) as f64));
        left.push_str(&format!("<w:p>{}</w:p>", inline_drawing_xml(&rel_id, logo_w_pt, logo_h_pt)));
    }
    let title = runs_xml(&[InlineRun::Plain(cover.title.clone())], &face(ctx.press, palette.font_display), sizes.cover_title, palette.ink, true, false);
    left.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverTitle"/></w:pPr>{title}</w:p>"#));
    let subtitle = runs_xml(&[InlineRun::Plain(cover.subtitle.clone())], &face(ctx.press, palette.font_display), sizes.cover_sub, palette.ink, false, false);
    left.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverSubtitle"/></w:pPr>{subtitle}</w:p>"#));
    left.push_str(&format!(
        r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="18" w:space="1" w:color="{}"/></w:pBdr><w:spacing w:after="120"/></w:pPr></w:p>"#,
        ooxml_hex(palette.accent)
    ));
    let meta = runs_xml(&[InlineRun::Plain(cover.meta.clone())], &face(ctx.press, palette.font_display), sizes.cover_meta, palette.muted, false, false);
    left.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverMeta"/></w:pPr>{meta}</w:p>"#));
    let site = runs_xml(&[InlineRun::Plain(cover.site.clone())], &face(ctx.press, palette.font_display), sizes.footer, palette.muted, false, false);
    left.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="PressCoverMeta"/></w:pPr>{site}</w:p>"#));

    let mut right = String::new();
    if let Some(badge) = &front_matter.badge {
        let badge_xml = runs_xml(&[InlineRun::Plain(badge.to_uppercase())], &face(ctx.press, palette.font_mono), sizes.footer, palette.accent, true, false);
        right.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr>{badge_xml}</w:p>"#));
    }
    if let Some(image_path) = &cover.image {
        let (pixel_w, pixel_h) = ctx.press.image_dimensions(image_path).map_err(|source| DocxError::Image { path: image_path.clone(), source })?;
        let bytes = ctx.press.read_image_bytes(image_path).map_err(|source| DocxError::Image { path: image_path.clone(), source })?;
        let rel_id = ctx.media.register_png(bytes);
        let aspect = (pixel_w as f64 / (pixel_h.max(1) as f64)).max(0.0001);
        let right_width_pt = right_twips as f64 / 20.0;
        // A sane visual cap (matches the cover's own generous-whitespace
        // right-side picture, `figures::Cover::render`'s own right column)
        // so a tall/narrow source image can never blow out the title page.
        const MAX_HEIGHT_PT: f64 = 320.0;
        let mut w = right_width_pt;
        let mut h = w / aspect;
        if h > MAX_HEIGHT_PT {
            h = MAX_HEIGHT_PT;
            w = h * aspect;
        }
        right.push_str(&format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#, inline_drawing_xml(&rel_id, w, h)));
    } else if right.is_empty() {
        right.push_str("<w:p/>");
    }

    Ok(format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="{body_twips}" w:type="dxa"/><w:tblBorders><w:top w:val="none"/><w:left w:val="none"/><w:bottom w:val="none"/><w:right w:val="none"/><w:insideH w:val="none"/><w:insideV w:val="none"/></w:tblBorders><w:tblLook w:val="0000"/></w:tblPr><w:tblGrid><w:gridCol w:w="{left_twips}"/><w:gridCol w:w="{right_twips}"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="{left_twips}" w:type="dxa"/></w:tcPr>{left}</w:tc><w:tc><w:tcPr><w:tcW w:w="{right_twips}" w:type="dxa"/><w:vAlign w:val="center"/></w:tcPr>{right}</w:tc></w:tr></w:tbl>"#
    ))
}

/// Band cover, laid out by [`BandCover::plan`] (the geometry the PDF cover
/// paints from): the logo inline at the margin, a vector parallelogram
/// anchored to the page, and the title, author and date blocks as
/// page-anchored frames so no run of empty paragraphs is needed to push them
/// into place.
fn band_cover_xml(cover: &BandCover, front_matter: &FrontMatter, ctx: &mut DocxCtx) -> Result<String, DocxError> {
    let plan = cover.plan();
    let palette = ctx.press.palette().clone();
    let sizes = *ctx.press.sizes();
    let font = face(ctx.press, palette.font_display);
    let text = |value: &str, size: f64, bold: bool| runs_xml(&[InlineRun::Plain(value.to_owned())], &font, size, palette.ink, bold, false);

    let mut first = String::new();
    if let Some(logo_path) = front_matter.logo.as_deref() {
        let (bytes, px_w, px_h) = media::resolve_logo_png(ctx.press, logo_path)?;
        let rel_id = ctx.media.register_png(bytes);
        let aspect = px_w as f64 / px_h.max(1) as f64;
        let (mut w, mut h) = (plan.logo.width, plan.logo.height);
        if w / h > aspect {
            w = h * aspect;
        } else {
            h = w / aspect;
        }
        first.push_str(&inline_drawing_xml(&rel_id, w, h));
    }
    first.push_str(&parallelogram_xml(plan.shape, cover.shape_color, cover.slant));
    let mut xml = format!(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr>{first}</w:p>"#);

    let title_frame = frame_pr_xml(plan.title_x, plan.title_width, FrameY::Page(plan.title_y));
    xml.push_str(&frame_paragraph(&title_frame, cover.title_leading, &text(&cover.title, sizes.cover_title, true)));
    if let Some(subtitle) = &cover.subtitle {
        xml.push_str(&frame_paragraph(&title_frame, cover.title_leading, &text(subtitle, sizes.cover_sub, true)));
    }
    let authors_frame = frame_pr_xml(plan.authors_x, plan.authors_width, FrameY::MarginBottom);
    for author in &cover.authors {
        xml.push_str(&frame_paragraph(&authors_frame, cover.authors_leading, &text(author, sizes.cover_meta, false)));
    }
    if let Some(date) = &cover.date {
        let width = (cover.page_width - cover.margin_right - plan.date_x).max(1.0);
        let date_frame = frame_pr_xml(plan.date_x, width, FrameY::Page(plan.date_y));
        xml.push_str(&frame_paragraph(&date_frame, cover.authors_leading, &text(date, sizes.cover_meta, true)));
    }
    Ok(xml)
}

/// Vertical anchor of a text frame.
enum FrameY {
    /// This many points from the top of the page.
    Page(f64),
    /// Flush with the bottom margin.
    MarginBottom,
}

/// `w:framePr`: a frame `width_pt` wide whose left edge is `x_pt` from the
/// page's left edge. Consecutive paragraphs with the same `framePr` share
/// one frame.
fn frame_pr_xml(x_pt: f64, width_pt: f64, y: FrameY) -> String {
    let vertical = match y {
        FrameY::Page(top) => format!(r#"w:vAnchor="page" w:y="{}""#, pt_to_twips(top)),
        FrameY::MarginBottom => r#"w:vAnchor="margin" w:yAlign="bottom""#.to_owned(),
    };
    format!(
        r#"<w:framePr w:w="{}" w:hRule="auto" w:hSpace="0" w:vSpace="0" w:wrap="around" w:hAnchor="page" w:x="{}" {vertical}/>"#,
        pt_to_twips(width_pt),
        pt_to_twips(x_pt)
    )
}

fn frame_paragraph(frame_pr: &str, leading_pt: f64, runs: &str) -> String {
    format!(
        r#"<w:p><w:pPr>{frame_pr}<w:spacing w:before="0" w:after="0" w:line="{}" w:lineRule="exact"/></w:pPr>{runs}</w:p>"#,
        pt_to_twips(leading_pt)
    )
}

/// The cover parallelogram as a page-anchored DrawingML shape (`wps:wsp`
/// with a custom path, like the original): filled `color`, no outline.
fn parallelogram_xml(shape: Rect, color: u32, slant: f64) -> String {
    let (x, y) = (pt_to_emu(shape.x), pt_to_emu(shape.y));
    let (cx, cy) = (pt_to_emu(shape.width), pt_to_emu(shape.height));
    let offset = (cx as f64 * slant).round() as i64;
    let foot = cx - offset;
    let fill = ooxml_hex(color);
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="9001" name="Cover parallelogram"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="0" b="0"/><a:pathLst><a:path w="{cx}" h="{cy}"><a:moveTo><a:pt x="{offset}" y="0"/></a:moveTo><a:lnTo><a:pt x="{cx}" y="0"/></a:lnTo><a:lnTo><a:pt x="{foot}" y="{cy}"/></a:lnTo><a:lnTo><a:pt x="0" y="{cy}"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom><a:solidFill><a:srgbClr val="{fill}"/></a:solidFill><a:ln><a:noFill/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::press::Palette;

    fn test_press() -> Press {
        Press::new(crate::press::Format::Doc, Palette::light(), crate::preset::Preset::column(), std::path::PathBuf::new())
    }

    #[test]
    fn table_xml_column_widths_sum_to_exactly_body_width_twips() {
        let press = test_press();
        let ctx = DocxCtx::new(&press);
        let body_twips = pt_to_twips(ctx.body_width_pt);
        // 3 equal fractions over a width not evenly divisible by 3 — proves
        // the remainder-to-last-column rule.
        let fractions = vec![1.0, 1.0, 1.0];
        let widths = column_widths_twips(&fractions, body_twips);
        assert_eq!(widths.iter().sum::<i64>(), body_twips);
        assert_eq!(widths.len(), 3);

        let header = vec![vec![InlineRun::Plain("A".to_owned())], vec![InlineRun::Plain("B".to_owned())], vec![InlineRun::Plain("C".to_owned())]];
        let xml = table_xml(&header, &[], &fractions, &[], &ctx);
        for w in &widths {
            assert!(xml.contains(&format!(r#"w:w="{w}""#)));
        }
    }

    #[test]
    fn kpi_table_xml_pads_a_partial_last_row_to_the_declared_column_count() {
        let press = test_press();
        let ctx = DocxCtx::new(&press);
        // 4 items over 3 columns -> last row has 1 real cell + 2 padding cells.
        let items = vec![
            ("1".to_owned(), "one".to_owned()),
            ("2".to_owned(), "two".to_owned()),
            ("3".to_owned(), "three".to_owned()),
            ("4".to_owned(), "four".to_owned()),
        ];
        let xml = kpi_table_xml(&items, &ctx);
        assert_eq!(xml.matches("<w:tr>").count(), 2);
        assert_eq!(xml.matches("<w:tc>").count(), 6);
        assert_eq!(xml.matches("<w:p/>").count(), 2);
    }

    #[test]
    fn a_bold_inline_run_emits_wb() {
        let runs = vec![InlineRun::Plain("plain ".to_owned()), InlineRun::Bold("bold".to_owned())];
        let xml = runs_xml(&runs, "Roboto", 11.0, 0x000000, false, false);
        assert!(xml.contains("<w:b/>"));
        // The plain run's own <w:r> must NOT carry <w:b/>.
        let plain_run_start = xml.find("<w:r>").expect("at least one run");
        let plain_run_end = xml[plain_run_start..].find("</w:r>").expect("run must close") + plain_run_start;
        assert!(!xml[plain_run_start..plain_run_end].contains("<w:b/>"));
    }

    fn example_press() -> Press {
        Press::new(crate::press::Format::Doc, crate::press::example_doc_palette(), crate::press::example_doc_preset(), std::path::PathBuf::new())
    }

    fn cells(texts: &[&str]) -> Vec<Vec<InlineRun>> {
        texts.iter().map(|t| vec![InlineRun::Plain((*t).to_owned())]).collect()
    }

    #[test]
    fn a_table_marks_its_header_row_to_repeat_and_keeps_rows_whole() {
        let press = example_press();
        let ctx = DocxCtx::new(&press);
        let rows = vec![cells(&["Rate", "10"]), cells(&["Total", "20"])];
        let xml = table_xml(&cells(&["Название", "Сумма"]), &rows, &[1.0, 1.0], &[ColumnAlign::Left, ColumnAlign::Right], &ctx);
        assert_eq!(xml.matches("<w:tblHeader/>").count(), 1, "only the first row repeats");
        assert_eq!(xml.matches("<w:keepNext/>").count(), 2, "both header paragraphs keep with the first body row");
        assert_eq!(xml.matches("<w:cantSplit/>").count(), 3, "every row stays whole");
        assert!(xml.contains(r#"w:fill="D0D7DE""#), "header fill");
        assert!(xml.contains(r#"w:color="666666""#) && xml.contains(r#"w:sz="4""#), "0.5 pt grey grid");
        assert_eq!(xml.matches(r#"w:fill="FFF4CC""#).count(), 2, "both cells of the totals row are filled");
        assert!(xml.contains("<w:tblCellMar><w:top w:w=\"60\""), "3 pt vertical cell margin");
        assert!(xml.contains(r#"<w:left w:w="120""#), "6 pt horizontal cell margin");
    }

    #[test]
    fn a_tinted_callout_is_one_shaded_paragraph_with_a_left_bar() {
        let press = example_press();
        let ctx = DocxCtx::new(&press);
        let xml = callout_xml(&[InlineRun::Plain("Примечание".to_owned())], &ctx);
        assert!(!xml.contains("<w:tbl>"));
        assert!(xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="FFF4E5"/>"#));
        assert!(xml.contains(r#"<w:left w:val="single" w:sz="16" w:space="6" w:color="112233"/>"#));
        assert!(xml.contains(r#"<w:sz w:val="19"/>"#), "9.5 pt text");
    }

    #[test]
    fn the_boxed_callout_of_the_other_themes_stays_a_table() {
        let press = test_press();
        let ctx = DocxCtx::new(&press);
        let xml = callout_xml(&[InlineRun::Plain("x".to_owned())], &ctx);
        assert!(xml.starts_with("<w:tbl>"));
    }

    fn inset_parts(body: &str) -> (InsetKind, Vec<InsetPart>) {
        let document = crate::parse::parse(&format!("---\ntitle: T\n---\n\n{body}\n")).expect("inset fixture parses");
        match document.sections.into_iter().next().and_then(|s| s.blocks.into_iter().next()) {
            Some(AstBlock::Inset { kind, parts }) => (kind, parts),
            other => panic!("expected a `>>` block, got {other:?}"),
        }
    }

    #[test]
    fn a_revised_block_is_shaded_paragraphs_with_a_bar_and_a_label_kept_with_the_first() {
        let press = example_press();
        let mut ctx = DocxCtx::new(&press);
        let (kind, parts) = inset_parts(">> Новый **текст**.\n>>\n>> Второй абзац.\n>>\n>> - пункт один\n>> - пункт два\n>>\n>> Итог.");
        let xml = inset_xml(kind, &parts, false, &mut ctx);
        assert!(!xml.contains("<w:tbl>"));
        assert!(xml.contains("Revised"), "the label line");
        assert_eq!(xml.matches("<w:keepNext/>").count(), 1, "only the label keeps with the next paragraph");
        assert_eq!(xml.matches("<w:p>").count(), 1 + 2 + 2 + 1, "label, two paragraphs, two items, one paragraph");
        assert_eq!(xml.matches(r#"<w:shd w:val="clear" w:color="auto" w:fill="E5F6D8"/>"#).count(), 6, "every paragraph is shaded");
        assert_eq!(xml.matches(r#"w:sz="16" w:space="8" w:color="3D7A1A""#).count(), 4, "2 pt bar, 8 pt out, on the text paragraphs");
        assert_eq!(xml.matches(r#"w:sz="16" w:space="26" w:color="3D7A1A""#).count(), 2, "list items: the border sits at the marker, 18 pt in");
        assert!(xml.contains(r#"<w:right w:val="single" w:sz="4" w:space="6" w:color="E5F6D8"/>"#), "the fill reaches 6 pt past the column");
        assert!(xml.contains(r#"<w:color w:val="3D7A1A"/><w:sz w:val="18"/>"#), "the label is 9 pt");
        assert!(xml.contains(r#"<w:sz w:val="22"/>"#), "the text stays at the 11 pt body size");
        assert!(xml.contains(r#"<w:sz w:val="20"/>"#), "list items at their 10 pt");
        // The second paragraph is followed by a list: no paragraph gap there.
        assert!(xml.contains(r#"<w:spacing w:after="0" w:line="304" w:lineRule="exact"/>"#));
    }

    #[test]
    fn a_technical_block_is_blue_at_the_callout_size_and_keeps_with_a_following_picture() {
        let press = example_press();
        let mut ctx = DocxCtx::new(&press);
        let (kind, parts) = inset_parts(">>> A technical **note**.\n>>>\n>>> - point\n>>> - another point");
        assert_eq!(kind, InsetKind::Technical);
        let alone = inset_xml(kind, &parts, false, &mut ctx);
        assert!(alone.contains("Note"), "the label line");
        assert_eq!(alone.matches("<w:keepNext/>").count(), 1, "only the label");
        assert_eq!(alone.matches(r#"<w:shd w:val="clear" w:color="auto" w:fill="E5F0FA"/>"#).count(), 4, "label, paragraph, two items");
        assert!(alone.contains(r#"w:sz="16" w:space="8" w:color="1A527A""#) && alone.contains(r#"w:sz="16" w:space="26" w:color="1A527A""#));
        assert!(alone.contains(r#"<w:color w:val="1A527A"/><w:sz w:val="18"/>"#), "the label is 9 pt");
        assert!(alone.contains(r#"<w:sz w:val="19"/>"#) && !alone.contains(r#"<w:sz w:val="22"/>"#), "text at the 9.5 pt callout size");
        // 9.5 pt text on a 15.18 * 9.5 / 11 line; list items on 12 * 9.5 / 10.
        assert!(alone.contains(r#"w:line="262""#) && alone.contains(r#"w:line="228" w:lineRule="exact"/><w:contextualSpacing/>"#));

        let kept = inset_xml(kind, &parts, true, &mut ctx);
        assert_eq!(kept.matches("<w:keepNext/>").count(), 2, "the label and the last item keep with the next paragraph");
        let last_item = kept.rfind("<w:p>").expect("has paragraphs");
        assert!(kept[last_item..].contains("<w:keepNext/>"), "the last item is the one that keeps");
    }

    #[test]
    fn a_picture_after_a_technical_block_is_kept_with_it_in_the_body() {
        let press = example_press();
        let mut ctx = DocxCtx::new(&press);
        let (kind, parts) = inset_parts(">>> A note.");
        let block = AstBlock::Inset { kind, parts };
        let picture = AstBlock::Image(ImageSpec { path: std::path::PathBuf::from("p.png"), height: None, island: None, width: None, caption: None });
        let paragraph = AstBlock::Paragraph(vec![InlineRun::Plain("x".to_owned())]);
        let followed = block_to_xml(&block, Some(&picture), &mut ctx).expect("block builds");
        assert_eq!(followed.matches("<w:keepNext/>").count(), 2, "label and the paragraph before the picture");
        let alone = block_to_xml(&block, Some(&paragraph), &mut ctx).expect("block builds");
        assert_eq!(alone.matches("<w:keepNext/>").count(), 1, "the label only");
    }

    #[test]
    fn a_revised_table_takes_the_fill_and_a_bar_on_its_left_edge() {
        let press = example_press();
        let mut ctx = DocxCtx::new(&press);
        let (kind, parts) = inset_parts(">> Before the table.\n>>\n>> | Name | Amount |\n>> |---|--:|\n>> | Rate | 10 |\n>> | Total | 20 |");
        let xml = inset_xml(kind, &parts, false, &mut ctx);
        assert!(xml.contains("<w:tbl>"));
        assert!(xml.contains(r#"<w:left w:val="single" w:sz="16" w:color="3D7A1A"/>"#), "the bar is the table's left edge");
        assert_eq!(xml.matches(r#"w:fill="E5F6D8""#).count(), 4, "label and paragraph, plus the two cells of the plain body row");
        assert!(xml.contains(r#"w:fill="D0D7DE""#) && xml.contains(r#"w:fill="FFF4CC""#));
    }

    #[test]
    fn blocks_of_another_theme_have_no_label_and_take_the_palette_colours() {
        let press = test_press();
        let mut ctx = DocxCtx::new(&press);
        for (marker, kind) in [(">>", InsetKind::Revised), (">>>", InsetKind::Technical)] {
            let (_, parts) = inset_parts(&format!("{marker} Текст."));
            let xml = inset_xml(kind, &parts, false, &mut ctx);
            let paint = press.palette().inset_paint(kind);
            assert!(!xml.contains("<w:keepNext/>"));
            assert_eq!(xml.matches("<w:p>").count(), 1);
            assert!(xml.contains(&format!(r#"w:fill="{}""#, ooxml_hex(paint.fill))));
            assert!(xml.contains(&format!(r#"w:color="{}""#, ooxml_hex(paint.bar))));
        }
    }

    #[test]
    fn an_h1_starts_a_page_except_at_the_top_of_one() {
        let press = example_press();
        let mut ctx = DocxCtx::new(&press);
        let runs = [InlineRun::Plain("1. ОГОВОРКИ".to_owned())];
        assert!(!heading_xml(1, &runs, &ctx).contains("<w:pageBreakBefore/>"), "first block of the body");
        ctx.at_page_top = false;
        let xml = heading_xml(1, &runs, &ctx);
        assert!(xml.contains("<w:pageBreakBefore/>"));
        assert!(xml.contains(r#"<w:spacing w:before="280" w:after="200""#), "14 pt above, 10 pt below");
        let h3 = heading_xml(3, &runs, &ctx);
        assert!(!h3.contains("<w:pageBreakBefore/>"));
        assert!(h3.contains(r#"w:before="160" w:after="120""#), "8 pt above, 6 pt below");
    }

    #[test]
    fn the_band_cover_carries_a_vector_parallelogram_and_page_anchored_frames() {
        let press = example_press();
        let document = crate::parse::parse("---\ntitle: T\nsubtitle: S\nauthors: A / B\ndate: 18.09.2026\n---\n\nText\n").expect("fixture parses");
        let cover = press.band_cover_data(&document).expect("the band cover is active");
        let mut ctx = DocxCtx::new(&press);
        let xml = band_cover_xml(&cover, &document.front_matter, &mut ctx).expect("cover builds");
        assert!(xml.contains("wordprocessingShape") && xml.contains(r#"<a:srgbClr val="336699"/>"#));
        let cx = pt_to_emu(cover.plan().shape.width);
        let offset = (cx as f64 * 0.5).round() as i64;
        assert!(xml.contains(&format!(r#"<a:pt x="{offset}" y="0"/>"#)), "slant is half the shape width");
        assert_eq!(xml.matches(r#"w:vAnchor="margin" w:yAlign="bottom""#).count(), 2, "two author lines in one bottom frame");
        assert!(xml.contains("18.09.2026"));
    }

    #[test]
    fn an_island_image_emits_an_anchor_a_non_island_image_emits_inline() {
        let anchor = anchored_drawing_xml("rId1", 100.0, 50.0, "left", "right");
        assert!(anchor.contains("<wp:anchor"));
        assert!(anchor.contains(r#"wrapText="right""#));
        let inline = inline_drawing_xml("rId1", 100.0, 50.0);
        assert!(inline.contains("<wp:inline"));
        assert!(!inline.contains("<wp:anchor"));
    }
}
