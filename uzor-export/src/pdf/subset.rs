//! Per-font glyph-set collection + subsetting (research doc §1/§3,
//! `nemo/docs/uzor-engines/research_export_sota_2026.md`). `subsetter`
//! (`typst/subsetter`, same author/lineage as `pdf-writer` itself) takes an
//! ALREADY-KNOWN glyph-id set — [`super::ttf::TtfMetrics`] is what actually
//! discovers that set from the document's own text (its `cmap` reader now
//! covers every codepoint, not a fixed WinAnsi byte range, see that
//! module's own doc comment). This module is the seam between the two:
//! [`build_font_data`] walks every page a font was used on, collects the
//! distinct glyph ids it referenced, subsets the font down to exactly
//! those, and builds this font's own `/W` widths + `/ToUnicode` CMap +
//! `original-gid -> new-CID` remap table FROM THE SAME `(gid, char)` pairs
//! the CIDs themselves come from — never independently. That "never
//! independently" rule exists because of a real, documented bug class:
//! [typst/typst#3416](https://github.com/typst/typst/issues/3416) shipped a
//! CJK CID font whose `ToUnicode` was built from a different source than
//! its CIDs — glyphs rendered fine, copy-paste text was silently wrong.

use std::collections::{BTreeSet, HashMap};

use pdf_writer::types::{SystemInfo, UnicodeCmap};
use pdf_writer::{Name, Str};

use super::render_context::PdfOp;
use super::ttf::TtfMetrics;
use super::PageRecord;

/// The `/CIDSystemInfo` this crate's own `/ToUnicode` CMaps declare —
/// `Adobe-Identity-UCS` is the conventional ordering for a CMap that maps
/// arbitrary CIDs to arbitrary Unicode scalars (no predefined character
/// collection), matching every other CID-font writer's own convention.
pub(super) const ADOBE_IDENTITY_UCS: SystemInfo<'static> = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"UCS"), supplement: 0 };

/// Everything [`super::write_font`] needs to embed ONE font as a
/// Type0/CIDFontType2 resource.
pub(super) struct FontData {
    /// The embedded font program bytes — a real `subsetter` subset when
    /// subsetting succeeded, or the full, unmodified font as a documented
    /// fallback (see [`build_font_data`]'s own doc comment).
    pub subset_bytes: Vec<u8>,
    /// `(cid, width_1000)` pairs. NOT assumed contiguous by
    /// [`super::write_font`] (a per-pair `/W` entry is always correct
    /// regardless of gaps) — the subsetting-succeeded path happens to
    /// produce a contiguous `0..n` CID space (`subsetter::GlyphRemapper`'s
    /// own guarantee), the fallback path does not.
    pub widths: Vec<(u16, f32)>,
    /// A finished `/ToUnicode` CMap stream (uncompressed — the caller
    /// Flate-compresses it, same as every other stream this crate writes).
    pub to_unicode: Vec<u8>,
    /// Original (pre-subsetting) glyph id -> the CID a page's own content
    /// stream must show for it.
    pub orig_to_new: HashMap<u16, u16>,
    /// `Some(tag)` when subsetting actually happened — the PDF spec's own
    /// six-uppercase-letter `+` `/BaseFont` prefix convention (§9.6.4) is
    /// only meaningful for a genuine subset, never for the degraded
    /// full-font fallback.
    pub subset_tag: Option<String>,
}

/// Collect every `(original gid, source char)` font `font_index`'s own
/// runs use across the WHOLE document's already-added `pages`, subset the
/// font to exactly that glyph set via `subsetter::subset`, and build the
/// widths/ToUnicode/remap tables from those SAME pairs.
///
/// Subsetting failure (an exotic/malformed font `subsetter` can't handle,
/// e.g. a CFF2 program) degrades to the pre-P5-SOTA-pass v1 behavior —
/// embed the FULL font unmodified, address glyphs by their own original
/// gid as the CID directly (still a valid Identity-H/`CIDToGIDMap
/// /Identity` CID font, just unsubsetted) — never a panic, never a
/// silently broken PDF, only a larger file (this crate's own "never panic,
/// degrade to a documented fallback" convention, same as every
/// `TtfMetrics` reader gap).
pub(super) fn build_font_data(font_index: u32, ttf_bytes: &[u8], metrics: &TtfMetrics, pages: &[PageRecord]) -> FontData {
    let mut used_gids: BTreeSet<u16> = BTreeSet::new();
    let mut gid_to_char: HashMap<u16, char> = HashMap::new();
    for page in pages {
        for run in &page.runs {
            if run.font_id.0 != font_index {
                continue;
            }
            for &(gid, ch) in &run.glyphs {
                used_gids.insert(gid);
                if gid != 0 {
                    gid_to_char.entry(gid).or_insert(ch);
                }
            }
        }
        // Figures/tables/chrome text (typography-gap WAVE 1) resolves its
        // own glyphs the SAME way `run.glyphs` above already does (see
        // `render_context::PdfRenderContext::fill_text`'s own doc
        // comment) — walked here too so a font used ONLY by a figure axis
        // label (never by any paragraph run) still gets subsetted
        // correctly rather than silently keeping an empty glyph set.
        for op in &page.content_ops {
            if let PdfOp::Text { font, glyphs, .. } = op {
                if font.0 != font_index {
                    continue;
                }
                for &(gid, ch) in glyphs {
                    used_gids.insert(gid);
                    if gid != 0 {
                        gid_to_char.entry(gid).or_insert(ch);
                    }
                }
            }
        }
    }

    let mut remapper = subsetter::GlyphRemapper::new();
    for &gid in &used_gids {
        remapper.remap(gid);
    }

    match subsetter::subset(ttf_bytes, 0, &remapper) {
        Ok(subset_bytes) => {
            let mut widths = Vec::new();
            let mut orig_to_new = HashMap::new();
            let mut cmap = UnicodeCmap::new(Name(b"Adobe-Identity-UCS"), ADOBE_IDENTITY_UCS);
            for (new_cid, old_gid) in remapper.remapped_gids().enumerate() {
                let new_cid = new_cid as u16;
                orig_to_new.insert(old_gid, new_cid);
                widths.push((new_cid, metrics.advance_1000_for_gid(old_gid) as f32));
                if let Some(&ch) = gid_to_char.get(&old_gid) {
                    cmap.pair(new_cid, ch);
                }
            }
            FontData {
                subset_bytes,
                widths,
                to_unicode: cmap.finish().into_vec(),
                orig_to_new,
                subset_tag: Some(subset_tag(font_index)),
            }
        }
        Err(_) => {
            let mut widths = Vec::new();
            let mut orig_to_new = HashMap::new();
            let mut cmap = UnicodeCmap::new(Name(b"Adobe-Identity-UCS"), ADOBE_IDENTITY_UCS);
            for &gid in &used_gids {
                orig_to_new.insert(gid, gid);
                widths.push((gid, metrics.advance_1000_for_gid(gid) as f32));
                if let Some(&ch) = gid_to_char.get(&gid) {
                    cmap.pair(gid, ch);
                }
            }
            FontData { subset_bytes: ttf_bytes.to_vec(), widths, to_unicode: cmap.finish().into_vec(), orig_to_new, subset_tag: None }
        }
    }
}

/// The PDF spec's own subset-font `/BaseFont` convention (§9.6.4): six
/// uppercase letters + `+`. Deterministic per font index (base-26,
/// zero-padded) — this crate has no RNG dependency and no reason to want
/// one here; the tag only needs to exist, not to be globally unique
/// across documents.
fn subset_tag(mut index: u32) -> String {
    let mut letters = [b'A'; 6];
    for slot in letters.iter_mut().rev() {
        *slot = b'A' + (index % 26) as u8;
        index /= 26;
    }
    String::from_utf8(letters.to_vec()).unwrap_or_else(|_| "AAAAAA".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{FontId, PageTextRun};

    const ROBOTO_REGULAR: &[u8] = include_bytes!("../../../uzor-fonts/fonts/Roboto-Regular.ttf");

    fn page_with_glyphs(font_index: u32, glyphs: Vec<(u16, char)>) -> PageRecord {
        PageRecord {
            width_pt: 100.0,
            height_pt: 100.0,
            raster_rgb: None,
            runs: vec![PageTextRun { font_id: FontId(font_index), size_pt: 12.0, x_pt: 0.0, y_pt: 0.0, rgb: 0, glyphs, real_advances_pt: None }],
            links: Vec::new(),
            content_ops: Vec::new(),
        }
    }

    #[test]
    fn subsetting_a_small_glyph_set_shrinks_the_embedded_font() {
        let metrics = TtfMetrics::parse(ROBOTO_REGULAR);
        let gid_a = metrics.gid_for_char('A').expect("Roboto must have a glyph for 'A'");
        let pages = vec![page_with_glyphs(0, vec![(gid_a, 'A')])];

        let data = build_font_data(0, ROBOTO_REGULAR, &metrics, &pages);
        assert!(data.subset_tag.is_some(), "subsetting a well-formed TrueType font must succeed");
        assert!(
            data.subset_bytes.len() < ROBOTO_REGULAR.len(),
            "a 1-glyph subset ({} bytes) must be far smaller than the full font ({} bytes)",
            data.subset_bytes.len(),
            ROBOTO_REGULAR.len()
        );
    }

    #[test]
    fn tounicode_and_cid_remap_agree_on_the_same_glyph() {
        let metrics = TtfMetrics::parse(ROBOTO_REGULAR);
        let gid_a = metrics.gid_for_char('A').expect("Roboto must have a glyph for 'A'");
        let pages = vec![page_with_glyphs(0, vec![(gid_a, 'A')])];

        let data = build_font_data(0, ROBOTO_REGULAR, &metrics, &pages);
        let new_cid = *data.orig_to_new.get(&gid_a).expect("the used glyph must be present in the remap table");
        // The ToUnicode CMap's own PostScript body embeds `<newcid> <utf16be>`
        // pairs as hex literals — a cheap, honest way to confirm the SAME
        // cid appears next to `A`'s own UTF-16BE code (0041) without a
        // second CMap parser.
        let cmap_text = String::from_utf8_lossy(&data.to_unicode);
        let cid_hex = format!("{new_cid:04x}");
        assert!(
            cmap_text.to_lowercase().contains(&format!("<{cid_hex}> <0041>")),
            "ToUnicode must map the SAME cid {new_cid} used for the CID font to U+0041, got: {cmap_text}"
        );
    }

    #[test]
    fn font_index_filters_out_other_fonts_own_runs() {
        let metrics = TtfMetrics::parse(ROBOTO_REGULAR);
        let gid_a = metrics.gid_for_char('A').expect("Roboto must have a glyph for 'A'");
        let gid_b = metrics.gid_for_char('B').expect("Roboto must have a glyph for 'B'");
        let pages = vec![page_with_glyphs(0, vec![(gid_a, 'A')]), page_with_glyphs(1, vec![(gid_b, 'B')])];

        let data = build_font_data(0, ROBOTO_REGULAR, &metrics, &pages);
        assert!(data.orig_to_new.contains_key(&gid_a), "font 0's own glyph must be remapped");
        assert!(!data.orig_to_new.contains_key(&gid_b), "font 1's own glyph must never leak into font 0's remap table");
    }
}
