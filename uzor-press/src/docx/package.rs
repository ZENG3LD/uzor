//! Zip assembly: `[Content_Types].xml`, every `_rels/*.rels` part,
//! `docProps/*.xml`, and `word/media/*.png` — the ONLY place this crate
//! touches the `zip` crate directly.
//!
//! Determinism: Stored (no Deflate) compression, a fixed 1980-01-01
//! per-entry timestamp, and one hardcoded, sorted write order — never a
//! `HashMap`-iteration-derived order — so two `write_package` calls over
//! the same [`DocxPackage`] produce byte-identical zip bytes.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::error::DocxError;
use super::media::MediaAsset;

/// `word/_rels/document.xml.rels`' own relationship id for `styles.xml`.
const STYLES_REL_ID: &str = "rIdStyles";
/// `word/_rels/document.xml.rels`' own relationship id for `numbering.xml`.
const NUMBERING_REL_ID: &str = "rIdNumbering";
/// `word/_rels/document.xml.rels`' own relationship id for `footer1.xml` —
/// ALSO the `r:id` the document's own `<w:sectPr><w:footerReference>`
/// points at (`docx::build` threads this same constant into
/// `section::sect_pr_xml`).
pub(crate) const FOOTER_REL_ID: &str = "rIdFooter1";

pub(crate) struct DocxPackage {
    pub document_xml: String,
    pub styles_xml: String,
    pub numbering_xml: String,
    pub footer_xml: String,
    /// `word/document.xml`'s own images (cover logo/image, inline/island
    /// images, diagram rasters) — registered via `docx::body::DocxCtx`'s
    /// own `MediaRegistry`.
    pub media: Vec<MediaAsset>,
    /// `word/footer1.xml`'s own images (the footer brand mark) — a
    /// SEPARATE `MediaRegistry` (OOXML has no cross-part dedup, so a logo
    /// used on both the cover and the footer is embedded twice).
    pub footer_media: Vec<MediaAsset>,
    pub core_props_xml: String,
    pub app_props_xml: String,
    pub hyphenate: bool,
}

/// The DOS-zip epoch minimum (1980-01-01 00:00:00) — in-range (unlike an
/// all-zero date some tools reject) and a well-known "no real timestamp"
/// convention. This exact input is always valid (it is the epoch's own
/// minimum boundary), so the only way `from_date_and_time` could fail here
/// is a `zip` crate regression, not a real runtime condition — hence
/// `expect` rather than propagating an error nothing meaningful could
/// recover from.
fn fixed_timestamp() -> DateTime {
    DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).expect("1980-01-01 00:00:00 is exactly the zip DOS-date epoch minimum, always valid")
}

pub(crate) fn write_package(pkg: DocxPackage) -> Result<Vec<u8>, DocxError> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored).last_modified_time(fixed_timestamp());

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));

    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types_xml(pkg.hyphenate).as_bytes())?;

    zip.start_file("_rels/.rels", options)?;
    zip.write_all(root_rels_xml().as_bytes())?;

    zip.start_file("docProps/core.xml", options)?;
    zip.write_all(pkg.core_props_xml.as_bytes())?;

    zip.start_file("docProps/app.xml", options)?;
    zip.write_all(pkg.app_props_xml.as_bytes())?;

    zip.start_file("word/document.xml", options)?;
    zip.write_all(pkg.document_xml.as_bytes())?;

    zip.start_file("word/_rels/document.xml.rels", options)?;
    zip.write_all(document_rels_xml(&pkg.media, pkg.hyphenate).as_bytes())?;

    zip.start_file("word/styles.xml", options)?;
    zip.write_all(pkg.styles_xml.as_bytes())?;

    zip.start_file("word/numbering.xml", options)?;
    zip.write_all(pkg.numbering_xml.as_bytes())?;

    zip.start_file("word/footer1.xml", options)?;
    zip.write_all(pkg.footer_xml.as_bytes())?;

    if pkg.hyphenate {
        zip.start_file("word/settings.xml", options)?;
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:autoHyphenation w:val="true"/></w:settings>"#)?;
    }

    if !pkg.footer_media.is_empty() {
        zip.start_file("word/_rels/footer1.xml.rels", options)?;
        zip.write_all(footer_rels_xml(&pkg.footer_media).as_bytes())?;
    }

    for asset in &pkg.media {
        zip.start_file(format!("word/media/{}", asset.file_name), options)?;
        zip.write_all(&asset.bytes)?;
    }
    for asset in &pkg.footer_media {
        zip.start_file(format!("word/media/{}", asset.file_name), options)?;
        zip.write_all(&asset.bytes)?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn content_types_xml(hyphenate: bool) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#);
    xml.push_str(r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#);
    xml.push_str(r#"<Default Extension="xml" ContentType="application/xml"/>"#);
    xml.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);
    xml.push_str(r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"#);
    if hyphenate {
        xml.push_str(r#"<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#);
    }
    xml.push_str(r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#);
    xml.push_str(r#"<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#);
    xml.push_str("</Types>");
    xml
}

fn root_rels_xml() -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    xml.push_str(r#"<Relationship Id="rIdDocument" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>"#);
    xml.push_str(r#"<Relationship Id="rIdCoreProps" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#);
    xml.push_str(r#"<Relationship Id="rIdAppProps" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>"#);
    xml.push_str("</Relationships>");
    xml
}

fn document_rels_xml(media: &[MediaAsset], hyphenate: bool) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    xml.push_str(&format!(
        r#"<Relationship Id="{STYLES_REL_ID}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    ));
    xml.push_str(&format!(
        r#"<Relationship Id="{NUMBERING_REL_ID}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#
    ));
    xml.push_str(&format!(
        r#"<Relationship Id="{FOOTER_REL_ID}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#
    ));
    if hyphenate {
        xml.push_str(
            r#"<Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#,
        );
    }
    for asset in media {
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{}"/>"#,
            asset.rel_id, asset.file_name
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn footer_rels_xml(footer_media: &[MediaAsset]) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    xml.push_str(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for asset in footer_media {
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{}"/>"#,
            asset.rel_id, asset.file_name
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Read;

    fn test_package() -> DocxPackage {
        DocxPackage {
            document_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:drawing><w:blah r:embed="rId1"/></w:drawing></w:r></w:p><w:sectPr><w:footerReference w:type="default" r:id="rIdFooter1"/></w:sectPr></w:body></w:document>"#.to_owned(),
            styles_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#.to_owned(),
            numbering_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#.to_owned(),
            footer_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p><w:r><w:drawing><w:blah r:embed="rId1"/></w:drawing></w:r></w:p></w:ftr>"#.to_owned(),
            media: vec![MediaAsset { rel_id: "rId1".to_owned(), file_name: "image1.png".to_owned(), bytes: vec![1, 2, 3] }],
            footer_media: vec![MediaAsset { rel_id: "rId1".to_owned(), file_name: "footer-image1.png".to_owned(), bytes: vec![4, 5, 6] }],
            core_props_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></cp:coreProperties>"#.to_owned(),
            app_props_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>mlc-pitch</Application></Properties>"#.to_owned(),
            hyphenate: false,
        }
    }

    #[test]
    fn write_package_output_contains_every_required_part() {
        let bytes = write_package(test_package()).expect("packaging must succeed");
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("must be a valid zip");
        let names: HashSet<String> = archive.file_names().map(|s| s.to_owned()).collect();
        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "docProps/core.xml",
            "docProps/app.xml",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
            "word/footer1.xml",
            "word/_rels/footer1.xml.rels",
            "word/media/image1.png",
            "word/media/footer-image1.png",
        ] {
            assert!(names.contains(required), "missing part {required}");
        }
        // Every XML/rels part must parse as well-formed XML (no attempt at
        // full schema validation — see the python validator for that).
        for name in names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            let mut file = archive.by_name(name).expect("named file must open");
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("part must be valid UTF-8");
            let mut reader = quick_xml::Reader::from_str(&contents);
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Eof) => break,
                    Ok(_) => {}
                    Err(e) => panic!("{name} is not well-formed XML: {e}"),
                }
            }
        }
    }

    #[test]
    fn every_relationship_id_referenced_in_document_and_footer_resolves_in_its_own_rels() {
        let bytes = write_package(test_package()).expect("packaging must succeed");
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("must be a valid zip");

        let read_part = |archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str| -> String {
            let mut file = archive.by_name(name).expect("named file must open");
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("part must be valid UTF-8");
            contents
        };

        let document_xml = read_part(&mut archive, "word/document.xml");
        let document_rels = read_part(&mut archive, "word/_rels/document.xml.rels");
        // `rIdStyles`/`rIdNumbering` are relationship-TYPE discovered (Word
        // finds `styles.xml`/`numbering.xml` by their own `Type=".../
        // styles"`/`".../numbering"`, never via an `r:id` scan of the body)
        // — legitimately declared but never `r:id`-referenced, unlike an
        // image relationship, which WOULD be a real orphan bug.
        assert_relationship_ids_resolve(&document_xml, &document_rels, &[STYLES_REL_ID, NUMBERING_REL_ID]);

        let footer_xml = read_part(&mut archive, "word/footer1.xml");
        let footer_rels = read_part(&mut archive, "word/_rels/footer1.xml.rels");
        assert_relationship_ids_resolve(&footer_xml, &footer_rels, &[]);
    }

    /// Scans every `r:id="..."`/`r:embed="..."` attribute value in `part_xml`
    /// and asserts each resolves to a matching `Id="..."` in `rels_xml`;
    /// and that every OTHER `Id` declared in `rels_xml` (excluding
    /// `type_discovered_ids`, relationships Word resolves by `Type=`, never
    /// by scanning the body for `r:id`) is referenced at least once (no
    /// orphaned IMAGE/footer relationship).
    fn assert_relationship_ids_resolve(part_xml: &str, rels_xml: &str, type_discovered_ids: &[&str]) {
        let referenced = extract_attr_values(part_xml, &["r:id", "r:embed"]);
        let declared = extract_attr_values(rels_xml, &["Id"]);
        for id in &referenced {
            assert!(declared.contains(id), "{id} referenced in the part but not declared in its own .rels");
        }
        for id in &declared {
            if type_discovered_ids.contains(&id.as_str()) {
                continue;
            }
            assert!(referenced.contains(id), "{id} declared in .rels but never referenced by the part");
        }
    }

    fn extract_attr_values(xml: &str, attr_names: &[&str]) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut reader = quick_xml::Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(quick_xml::events::Event::Start(e)) | Ok(quick_xml::events::Event::Empty(e)) => {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                        if attr_names.contains(&key.as_str()) {
                            out.insert(String::from_utf8_lossy(&attr.value).into_owned());
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => panic!("malformed XML while scanning relationship ids: {e}"),
            }
        }
        out
    }
}
