//! `word/styles.xml` — the named paragraph styles `docx::body` applies to
//! every block-level element (`Normal`, `Heading1/2/3`, `ListParagraph`,
//! `Caption`, `PressCoverTitle`, `PressCoverSubtitle`, `PressCoverMeta`).
//! Table/KPI/diagram content uses direct run formatting instead (no named
//! style) — same "small dedicated tool, no unnecessary indirection"
//! doctrine this crate already follows in `figures.rs`.

use super::units::{docx_font_name, ooxml_hex, pt_to_half_points};
use crate::press::{Palette, Sizes};

fn rpr_xml(font: &str, size_pt: f64, color: u32, bold: bool, italic: bool) -> String {
    let mut xml = String::new();
    xml.push_str(&format!(r#"<w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:cs="{font}"/>"#));
    if bold {
        xml.push_str("<w:b/><w:bCs/>");
    }
    if italic {
        xml.push_str("<w:i/><w:iCs/>");
    }
    xml.push_str(&format!(r#"<w:color w:val="{}"/>"#, ooxml_hex(color)));
    let half_points = pt_to_half_points(size_pt);
    xml.push_str(&format!(r#"<w:sz w:val="{half_points}"/><w:szCs w:val="{half_points}"/>"#));
    xml
}

fn paragraph_style_xml(style_id: &str, name: &str, based_on: &str, extra_ppr: &str, rpr: &str) -> String {
    format!(
        r#"<w:style w:type="paragraph" w:styleId="{style_id}"><w:name w:val="{name}"/><w:basedOn w:val="{based_on}"/><w:pPr>{extra_ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>"#
    )
}

/// The whole `word/styles.xml` body — theme colors + the plan's font
/// declaration baked directly into each named style's own `w:rPr`/`w:pPr`.
pub(crate) fn build_styles_xml(palette: &Palette, sizes: &Sizes, h1_rule: bool, lang: &str) -> String {
    let face = docx_font_name(palette, palette.font_display);
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );

    // `docDefaults` — every unstyled run falls back to the display font at
    // body size, ink color.
    // The proofing language sits last in `rPr` (after `szCs`).
    xml.push_str(&format!(
        r#"<w:docDefaults><w:rPrDefault><w:rPr>{}<w:lang w:val="{lang}"/></w:rPr></w:rPrDefault></w:docDefaults>"#,
        rpr_xml(&face, sizes.body, palette.ink, false, false)
    ));

    // `Normal` — the base paragraph style every other style is `basedOn`.
    xml.push_str(&format!(
        r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/><w:rPr>{}</w:rPr></w:style>"#,
        rpr_xml(&face, sizes.body, palette.ink, false, false)
    ));

    // `Heading1` — bold, H1 size, accent bottom border spanning the
    // paragraph's own width (Divergence 1: full-width, not the PDF's own
    // 64pt stub), keep-with-next.
    let h1_border = if h1_rule {
        format!(r#"<w:pBdr><w:bottom w:val="single" w:sz="24" w:space="4" w:color="{}"/></w:pBdr><w:keepNext/>"#, ooxml_hex(palette.accent))
    } else {
        "<w:keepNext/>".to_owned()
    };
    xml.push_str(&paragraph_style_xml("Heading1", "heading 1", "Normal", &h1_border, &rpr_xml(&face, sizes.h1, palette.ink, true, false)));

    // `Heading2` — bold, H2 size, keep-with-next.
    xml.push_str(&paragraph_style_xml("Heading2", "heading 2", "Normal", "<w:keepNext/>", &rpr_xml(&face, sizes.h2, palette.ink, true, false)));

    // `Heading3` — bold, H3 size, keep-with-next (as in the PDF).
    xml.push_str(&paragraph_style_xml("Heading3", "heading 3", "Normal", "<w:keepNext/>", &rpr_xml(&face, sizes.h3, palette.ink, true, false)));

    // `ListParagraph` — body size, standard Word list-paragraph name (so
    // Word's own list-paragraph affordances recognize it).
    xml.push_str(&paragraph_style_xml("ListParagraph", "List Paragraph", "Normal", "", &rpr_xml(&face, sizes.list_item, palette.ink, false, false)));

    // `Caption` — italic, caption size, muted, centered.
    xml.push_str(&paragraph_style_xml("Caption", "caption", "Normal", r#"<w:jc w:val="center"/>"#, &rpr_xml(&face, sizes.caption, palette.muted, false, true)));

    // `PressCoverTitle` — bold, cover title size.
    xml.push_str(&paragraph_style_xml("PressCoverTitle", "Press Cover Title", "Normal", "", &rpr_xml(&face, sizes.cover_title, palette.ink, true, false)));

    // `PressCoverSubtitle` — regular, cover subtitle size.
    xml.push_str(&paragraph_style_xml("PressCoverSubtitle", "Press Cover Subtitle", "Normal", "", &rpr_xml(&face, sizes.cover_sub, palette.ink, false, false)));

    // `PressCoverMeta` — regular, cover meta size, muted (also reused for
    // `site`/footer-style small print inside the cover/closing figures).
    xml.push_str(&paragraph_style_xml("PressCoverMeta", "Press Cover Meta", "Normal", "", &rpr_xml(&face, sizes.cover_meta, palette.muted, false, false)));

    xml.push_str("</w:styles>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::press::Palette;

    fn test_sizes() -> Sizes {
        // Mirrors `Sizes::doc()` — `Sizes`'s own fields are `pub`, so a
        // literal test fixture needs every field named once.
        Sizes {
            h1: 24.0,
            h2: 18.0,
            h3: 15.0,
            body: 13.0,
            list_item: 13.0,
            table_header: 11.0,
            table_cell: 11.0,
            caption: 10.5,
            footer: 9.0,
            callout: 13.0,
            kpi_value: 32.0,
            kpi_label: 10.5,
            cover_title: 44.0,
            cover_sub: 16.0,
            cover_meta: 11.0,
            diagram_title: 12.0,
            diagram_subtitle: 10.5,
        }
    }

    #[test]
    fn heading1_bottom_border_color_matches_the_palette_accent_exactly() {
        let palette = Palette::light();
        let xml = build_styles_xml(&palette, &test_sizes(), true, "ru-RU");
        let needle = format!(r#"w:bottom w:val="single" w:sz="24" w:space="4" w:color="{}""#, ooxml_hex(palette.accent));
        assert!(xml.contains(&needle), "Heading1 pBdr color must be the palette accent hex, xml={xml}");
    }

    #[test]
    fn doc_defaults_carry_the_proofing_language_and_the_theme_docx_font() {
        let mut palette = Palette::light();
        palette.doc.docx_font = Some("Example Face".to_owned());
        let xml = build_styles_xml(&palette, &test_sizes(), false, "ru-RU");
        assert!(xml.contains(r#"<w:lang w:val="ru-RU"/></w:rPr></w:rPrDefault>"#));
        assert!(xml.contains(r#"w:ascii="Example Face""#));
        assert!(!xml.contains("Roboto"));
    }

    #[test]
    fn every_heading_style_keeps_with_next() {
        let xml = build_styles_xml(&Palette::light(), &test_sizes(), false, "ru-RU");
        for id in ["Heading1", "Heading2", "Heading3"] {
            let start = xml.find(&format!(r#"w:styleId="{id}""#)).expect("heading style is declared");
            let end = xml[start..].find("</w:style>").expect("style closes") + start;
            assert!(xml[start..end].contains("<w:keepNext/>"), "{id} must keep with next");
        }
    }

    #[test]
    fn every_named_style_referenced_by_body_is_declared() {
        let xml = build_styles_xml(&Palette::light(), &test_sizes(), true, "ru-RU");
        for style_id in ["Normal", "Heading1", "Heading2", "Heading3", "ListParagraph", "Caption", "PressCoverTitle", "PressCoverSubtitle", "PressCoverMeta"] {
            assert!(xml.contains(&format!(r#"w:styleId="{style_id}""#)), "missing style {style_id}");
        }
    }
}
