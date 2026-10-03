//! Orchestration only — resolves the footer's own logo, assembles
//! `word/document.xml`, and hands everything to [`super::package::
//! write_package`]. No OOXML fragment building of its own beyond the
//! document/docProps root wrappers.

use crate::parse::{Document, FrontMatter};
use crate::press::{Format, Press};

use super::body::{build_document_body, DocxCtx};
use super::error::DocxError;
use super::media::{resolve_logo_png, MediaRegistry};
use super::package::{write_package, DocxPackage, FOOTER_REL_ID};
use super::section::{build_footer_xml, sect_pr_xml};
use super::styles::build_styles_xml;
use super::units::escape_xml_text;

/// Builds a complete `.docx` byte stream from `document`/`press`. Rejects
/// `Format::Deck` up front — belt-and-suspenders, since `main.rs` already
/// refuses `--format deck --emit docx|both` before ever calling this.
pub(crate) fn build_docx(document: &Document, press: &Press) -> Result<Vec<u8>, DocxError> {
    if press.format() == Format::Deck {
        return Err(DocxError::UnsupportedFormat(Format::Deck));
    }

    let mut ctx = DocxCtx::new(press);
    let body_xml = build_document_body(document, &mut ctx)?;
    let (media, numbering) = ctx.into_media_and_numbering();

    let has_cover = document.cover.is_some() || press.band_cover_active();
    let (footer_media, footer_logo) = resolve_footer_logo(press, &document.front_matter)?;
    let footer_logo_ref = footer_logo.as_ref().map(|(rel_id, w, h)| (rel_id.as_str(), *w, *h));
    let footer_xml = build_footer_xml(
        &document.front_matter.footer,
        footer_logo_ref,
        press.palette(),
        press.sizes().footer,
        press.body_width(),
        press.sheet().page_number.format,
    );
    // A cover page normally has no footer (`titlePg` with no first-page
    // footer). A theme with a page mark numbers the cover too, so it keeps
    // the ordinary footer there.
    let cover_without_footer = has_cover && press.palette().doc.page_mark.is_none();
    let sect_pr = sect_pr_xml(press.format(), press.margins(), press.sheet().footer_from_edge, cover_without_footer, FOOTER_REL_ID);

    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body>{body_xml}{sect_pr}</w:body></w:document>"#
    );
    let styles_xml = build_styles_xml(press.palette(), press.sizes(), press.sheet().h1_rule, &word_language(&document.front_matter.lang));
    let numbering_xml = numbering.to_numbering_xml();

    let pkg = DocxPackage {
        document_xml,
        styles_xml,
        numbering_xml,
        footer_xml,
        media: media.assets().to_vec(),
        footer_media: footer_media.assets().to_vec(),
        core_props_xml: core_props_xml(&document.front_matter),
        app_props_xml: app_props_xml(),
        hyphenate: press.sheet().hyphenate,
    };
    write_package(pkg)
}

/// Resolves the footer's own brand mark (front matter's `logo:`, the SAME
/// key the cover reads) into a dedicated `MediaRegistry` (OOXML has no
/// cross-part dedup — a logo used on both the cover and the footer is
/// embedded TWICE, once per part) and the `(rel_id, width_pt, height_pt)`
/// triple `section::build_footer_xml` needs. `None` when no `logo:` is
/// set, matching `press.rs::footer_master`'s own "byte-identical when
/// absent" rule.
fn resolve_footer_logo(press: &Press, front_matter: &FrontMatter) -> Result<(MediaRegistry, Option<(String, f64, f64)>), DocxError> {
    let mut registry = MediaRegistry::with_prefix("footer-image");
    let Some(logo_path) = front_matter.logo.clone().filter(|_| press.palette().doc.footer_logo) else {
        return Ok((registry, None));
    };
    let (bytes, pixel_w, pixel_h) = resolve_logo_png(press, &logo_path)?;
    let rel_id = registry.register_png(bytes);
    // `SPEC.md`'s own footer mark height: "a small mark (~18 pt) at the
    // left of the footer line" (`figures::FOOTER_MARK_H`).
    const FOOTER_MARK_HEIGHT_PT: f64 = 18.0;
    let width_pt = FOOTER_MARK_HEIGHT_PT * (pixel_w as f64 / (pixel_h.max(1) as f64));
    Ok((registry, Some((rel_id, width_pt, FOOTER_MARK_HEIGHT_PT))))
}

/// The BCP 47 tag Word wants for `w:lang` from a front-matter `lang` code.
fn word_language(lang: &str) -> String {
    match lang.trim().to_lowercase().as_str() {
        "ru" => "ru-RU".to_owned(),
        "en" => "en-US".to_owned(),
        _ => lang.trim().to_owned(),
    }
}

fn core_props_xml(front_matter: &FrontMatter) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(
        r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
    );
    xml.push_str(&format!("<dc:title>{}</dc:title>", escape_xml_text(&front_matter.title)));
    if let Some(author) = &front_matter.author {
        xml.push_str(&format!("<dc:creator>{}</dc:creator>", escape_xml_text(author)));
    }
    if let Some(subject) = &front_matter.subject {
        xml.push_str(&format!("<dc:subject>{}</dc:subject>", escape_xml_text(subject)));
    }
    xml.push_str(&format!(r#"<dc:language>{}</dc:language>"#, escape_xml_text(&front_matter.lang)));
    xml.push_str("</cp:coreProperties>");
    xml
}

fn app_props_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>mlc-pitch</Application></Properties>"#
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use crate::press::Palette;

    fn fixture_document() -> Document {
        let source = r#"---
title: Test Document
footer: Test Footer
---

# Heading One

A paragraph with **bold** text.

- item one
- item two

| a | b |
|---|---|
| 1 | 2 |

:::kpi
42 | the answer
:::

:::diagram
row: Alpha | first
row: Beta | second
:::
"#;
        parse::parse(source).expect("fixture document must parse")
    }

    fn fixture_press() -> Press {
        Press::new(Format::Doc, Palette::light(), crate::preset::Preset::column(), std::path::PathBuf::new())
    }

    #[test]
    fn build_docx_is_deterministic_byte_for_byte_across_two_builds() {
        let document = fixture_document();
        let press = fixture_press();
        let bytes1 = build_docx(&document, &press).expect("first build must succeed");
        let bytes2 = build_docx(&document, &press).expect("second build must succeed");
        assert_eq!(bytes1, bytes2, "two build_docx calls over the same Document/Press must be byte-identical");
    }

    #[test]
    fn word_language_maps_the_short_codes_and_passes_others_through() {
        assert_eq!(word_language("ru"), "ru-RU");
        assert_eq!(word_language("EN"), "en-US");
        assert_eq!(word_language("de-DE"), "de-DE");
    }

    #[test]
    fn a_band_cover_document_builds_with_page_mark_footer_and_no_title_page_switch() {
        let source = "---\ntitle: Заключение\nsubtitle: Подзаголовок\nauthors: Иванов / Петров\ndate: 18.09.2026\n---\n\n# 1. ОГОВОРКИ\n\nТекст.\n";
        let document = parse::parse(source).expect("fixture document must parse");
        let press = Press::new(Format::Doc, crate::press::example_doc_palette(), crate::press::example_doc_preset(), std::path::PathBuf::new());
        let bytes = build_docx(&document, &press).expect("docx must build");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("must be a valid zip");
        let mut read = |name: &str| {
            let mut out = String::new();
            std::io::Read::read_to_string(&mut archive.by_name(name).expect("part exists"), &mut out).expect("utf-8 part");
            out
        };
        let document_xml = read("word/document.xml");
        assert!(!document_xml.contains("<w:titlePg/>"), "the cover keeps the footer");
        assert!(document_xml.contains("Заключение") && document_xml.contains("18.09.2026"));
        assert!(read("word/footer1.xml").contains('\u{25B0}'));
        assert!(read("word/styles.xml").contains(r#"<w:lang w:val="ru-RU"/>"#));
    }

    #[test]
    fn build_docx_refuses_the_deck_format() {
        let document = fixture_document();
        let press = Press::new(Format::Deck, Palette::light(), crate::preset::Preset::column(), std::path::PathBuf::new());
        let err = build_docx(&document, &press).expect_err("deck must be refused");
        assert!(matches!(err, DocxError::UnsupportedFormat(Format::Deck)));
    }
}
