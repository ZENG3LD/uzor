//! `word/footer1.xml` + the document's own `<w:sectPr>` — page geometry,
//! the footer flow (brand mark + text + `PAGE`/`NUMPAGES` fields), and the
//! `<w:titlePg/>` cover-has-no-footer mechanism.

use uzor_typeset::{Margins, PageNumberFormat};

use crate::press::{Format, Palette};

use super::body::inline_drawing_xml;
use super::units::{docx_font_name, escape_xml_text, ooxml_hex, pt_to_half_points, pt_to_twips};

/// The whole `word/footer1.xml` body: `[logo?][footer text][tab][PAGE
/// "/" NUMPAGES]`, one paragraph with a right tab stop at `body_width_pt`.
/// `logo` is `(rel_id, width_pt, height_pt)` — `None` keeps the ORIGINAL
/// plain-text-only footer, byte-identical in spirit to the PDF path's own
/// `logo: None -> plain Paragraph` rule (`press.rs::footer_master`).
///
/// A theme with `doc.page_mark` set gets `PAGE ▰ NUMPAGES` instead: the
/// U+25B0 parallelogram in that colour, set in a symbol font that carries it.
/// An empty `footer_text` writes no text run.
pub(crate) fn build_footer_xml(
    footer_text: &str,
    logo: Option<(&str, f64, f64)>,
    palette: &Palette,
    footer_size_pt: f64,
    body_width_pt: f64,
    number: PageNumberFormat,
) -> String {
    let tab_pos = pt_to_twips(body_width_pt);
    let font = docx_font_name(palette, palette.font_display);
    let half_points = pt_to_half_points(footer_size_pt);
    let color = ooxml_hex(if palette.doc.page_mark.is_some() { palette.ink } else { palette.muted });
    let run = |text: &str| {
        format!(
            r#"<w:r><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:cs="{font}"/><w:color w:val="{color}"/><w:sz w:val="{half_points}"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
            escape_xml_text(text)
        )
    };
    let field = |instr: &str| {
        format!(
            r#"<w:fldSimple w:instr=" {instr} "><w:r><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:cs="{font}"/><w:color w:val="{color}"/><w:sz w:val="{half_points}"/></w:rPr><w:t>1</w:t></w:r></w:fldSimple>"#
        )
    };

    let mut run_xml = String::new();
    if let Some((rel_id, w, h)) = logo {
        run_xml.push_str(&inline_drawing_xml(rel_id, w, h));
    }
    if !footer_text.is_empty() {
        run_xml.push_str(&run(footer_text));
    }
    run_xml.push_str("<w:r><w:tab/></w:r>");
    run_xml.push_str(&field("PAGE"));
    if let Some(mark) = palette.doc.page_mark {
        run_xml.push_str(&run(" "));
        run_xml.push_str(&format!(
            r#"<w:r><w:rPr><w:rFonts w:ascii="{SYMBOL_FONT}" w:hAnsi="{SYMBOL_FONT}" w:cs="{SYMBOL_FONT}"/><w:color w:val="{}"/><w:sz w:val="{half_points}"/></w:rPr><w:t>{PARALLELOGRAM_GLYPH}</w:t></w:r>"#,
            ooxml_hex(mark)
        ));
        run_xml.push_str(&run(" "));
        run_xml.push_str(&field("NUMPAGES"));
    } else if number == PageNumberFormat::OfTotal {
        run_xml.push_str(&run("/"));
        run_xml.push_str(&field("NUMPAGES"));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:p><w:pPr><w:tabs><w:tab w:val="right" w:pos="{tab_pos}"/></w:tabs></w:pPr>{run_xml}</w:p></w:ftr>"#
    )
}

/// The font the folio parallelogram is set in: it ships with Windows and
/// carries U+25B0, which the body face does not.
const SYMBOL_FONT: &str = "Segoe UI Symbol";
/// BLACK PARALLELOGRAM.
const PARALLELOGRAM_GLYPH: char = '\u{25B0}';

/// The document's own single `<w:sectPr>` — `doc` -> A4 portrait, `report`
/// -> A4 landscape (`Format::page_size`/`margins`, unchanged from the PDF
/// path's own numbers). `has_cover` sets `<w:titlePg/>` with NO `type="first"`
/// footer reference, so Word gives page 1 no footer at all (the cover's own
/// "no footer, no page number" rule, `press.rs::cover_master`) while every
/// other page uses `footer_r_id`'s `type="default"` footer. Header/footer
/// distance-from-edge is a fixed 720 twips (0.5in, Word's own default) —
/// `PageMaster`'s own pixel-exact footer band has no Word analogue
/// (Divergence 3).
pub(crate) fn sect_pr_xml(format: Format, margins: Margins, footer_from_edge: f64, has_cover: bool, footer_r_id: &str) -> String {
    let (w, h) = format.page_size();
    let orient = if matches!(format, Format::Report) { r#" w:orient="landscape""# } else { "" };
    let title_pg = if has_cover { "<w:titlePg/>" } else { "" };
    let footer = pt_to_twips(footer_from_edge);
    format!(
        r#"<w:sectPr><w:footerReference w:type="default" r:id="{footer_r_id}"/><w:pgSz w:w="{}" w:h="{}"{orient}/><w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="{footer}" w:footer="{footer}" w:gutter="0"/>{title_pg}</w:sectPr>"#,
        pt_to_twips(w),
        pt_to_twips(h),
        pt_to_twips(margins.top),
        pt_to_twips(margins.right),
        pt_to_twips(margins.bottom),
        pt_to_twips(margins.left),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_without_a_logo_carries_the_footer_text_and_page_fields() {
        let palette = Palette::light();
        let xml = build_footer_xml("Some Footer", None, &palette, 9.0, 483.0, PageNumberFormat::OfTotal);
        assert!(xml.contains("Some Footer"));
        assert!(xml.contains(r#"w:instr=" PAGE ""#));
        assert!(xml.contains(r#"w:instr=" NUMPAGES ""#));
        assert!(!xml.contains("<wp:inline"));
    }

    #[test]
    fn a_page_mark_theme_writes_page_parallelogram_total_with_the_mark_colour() {
        let palette = crate::press::example_doc_palette();
        let xml = build_footer_xml("", None, &palette, 9.0, 467.0, PageNumberFormat::Bare);
        assert!(xml.contains(r#"w:instr=" PAGE ""#));
        assert!(xml.contains(r#"w:instr=" NUMPAGES ""#));
        assert!(xml.contains('\u{25B0}'));
        assert!(xml.contains(r#"<w:color w:val="445566"/>"#));
        assert!(xml.contains("Segoe UI Symbol"));
        assert!(!xml.contains("<w:t xml:space=\"preserve\"></w:t>"), "an empty footer text writes no run");
        assert!(!xml.contains("/</w:t>"), "no `n / N` slash for the mark");
    }

    #[test]
    fn footer_with_a_logo_embeds_an_inline_drawing() {
        let palette = Palette::light();
        let xml = build_footer_xml("Some Footer", Some(("rId1", 18.0, 18.0)), &palette, 9.0, 483.0, PageNumberFormat::OfTotal);
        assert!(xml.contains("<wp:inline"));
        assert!(xml.contains(r#"r:embed="rId1""#));
    }

    #[test]
    fn report_sect_pr_is_landscape_doc_is_portrait() {
        let margins = Margins::new(64.0, 64.0, 72.0, 64.0);
        let doc_xml = sect_pr_xml(Format::Doc, margins, 18.0, false, "rIdFooter1");
        assert!(!doc_xml.contains("landscape"));
        let report_xml = sect_pr_xml(Format::Report, margins, 18.0, false, "rIdFooter1");
        assert!(report_xml.contains("landscape"));
    }

    #[test]
    fn a_cover_gets_title_pg_and_no_cover_omits_it() {
        let margins = Margins::new(64.0, 64.0, 72.0, 64.0);
        let with_cover = sect_pr_xml(Format::Doc, margins, 18.0, true, "rIdFooter1");
        assert!(with_cover.contains("<w:titlePg/>"));
        let without_cover = sect_pr_xml(Format::Doc, margins, 18.0, false, "rIdFooter1");
        assert!(!without_cover.contains("<w:titlePg/>"));
    }
}
