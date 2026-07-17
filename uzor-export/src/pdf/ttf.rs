//! A minimal, hand-rolled TrueType/OpenType table reader — extracts exactly
//! the four things [`super`] needs to embed a font as a PDF **simple**
//! TrueType font under `WinAnsiEncoding`: a `char -> glyph id` lookup
//! (`cmap`, format 4 only), per-glyph advance widths (`hmtx`), the
//! em-square scale (`head`'s `unitsPerEm`), and a handful of best-effort
//! `FontDescriptor` metrics (`hhea`/`OS/2`).
//!
//! **Why hand-rolled, not a font-parsing crate (`ttf-parser` et al.):**
//! this phase's dependency law scopes new `uzor-export` dependencies to
//! `pdf-writer` (+ `subsetter`, not adopted — see `super`'s own doc comment
//! for why full-font embedding was chosen instead) only. Getting
//! `subsetter` working at all would ALSO have needed a real font parser
//! (its own API takes already-known glyph ids — it has no `cmap`/`hmtx`
//! reader of its own), so that dependency-law constraint is what actually
//! forced the "full font, no subsetting" v1 fallback the design doc's own
//! §6.3 explicitly sanctions, not a scope preference. This module is
//! narrow ON PURPOSE — no CFF, no cmap format 12/14, no variable-font
//! instancing, and it never panics: anything it can't parse degrades to a
//! documented, conservative fallback constant instead.
//!
//! **Why this is still exact where it matters**: this crate's own
//! shaper-computed glyph positions are NOT available at PDF-assembly time
//! (`PdfTextRun` — deliberately neutral, see `super`'s own doc comment —
//! carries only a text string, no per-glyph pixel data), so the `/Widths`
//! array driving a PDF viewer's OWN cursor advance between glyphs of one
//! `Tj` string must come from the EMBEDDED font's real metrics, not an
//! approximation — this reader gets those real metrics directly from the
//! same bytes that get embedded, so the widths a viewer uses always match
//! the glyphs it's actually drawing.

use std::collections::HashMap;

const TAG_HEAD: &[u8; 4] = b"head";
const TAG_HHEA: &[u8; 4] = b"hhea";
const TAG_HMTX: &[u8; 4] = b"hmtx";
const TAG_CMAP: &[u8; 4] = b"cmap";
const TAG_OS2: &[u8; 4] = b"OS/2";

/// Best-effort metrics extracted from one TrueType/OpenType font program.
/// Every field has a conservative fallback (see [`TtfMetrics::parse`]'s own
/// per-table doc comments) — a malformed or unusual font never panics,
/// just degrades to sane, documented constants.
#[derive(Debug, Clone)]
pub(super) struct TtfMetrics {
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub cap_height: i16,
    /// `[xMin, yMin, xMax, yMax]`, font units.
    pub bbox: [i16; 4],
    /// `WinAnsiEncoding` byte code -> glyph id, resolved via this font's own
    /// `cmap` at parse time (never re-parsed per lookup).
    glyph_for_byte: [Option<u16>; 256],
    /// Glyph id -> advance width, font units (`hmtx`) — only populated for
    /// glyphs [`Self::glyph_for_byte`] actually reached.
    advance_for_glyph: HashMap<u16, u16>,
}

/// Used only when a byte code's glyph/advance genuinely couldn't be
/// resolved (missing `cmap` coverage, malformed font, or `bullet` used as
/// the undefined-slot decode fallback for a glyph a real font may not
/// contain) — a plausible average Latin glyph width, never a hard error;
/// the WORST case is that one glyph's own trailing space is a few percent
/// off, never a missing or wrongly-shaped glyph (the embedded outline is
/// used verbatim regardless of this fallback).
const FALLBACK_WIDTH_1000: f64 = 600.0;

impl TtfMetrics {
    /// Parse `data` (a full, unmodified OpenType/TrueType font program).
    /// Never panics — every table is optional from this function's own
    /// point of view; a missing/malformed one just leaves its fields at
    /// the conservative defaults below.
    pub(super) fn parse(data: &[u8]) -> Self {
        let mut metrics = TtfMetrics {
            units_per_em: 1000,
            ascent: 800,
            descent: -200,
            cap_height: 700,
            bbox: [0, 0, 0, 0],
            glyph_for_byte: [None; 256],
            advance_for_glyph: HashMap::new(),
        };

        let Some(tables) = read_table_directory(data) else { return metrics };

        if let Some(head) = tables.get(TAG_HEAD).copied() {
            if let Some(v) = read_u16(data, head.0 + 18) {
                metrics.units_per_em = v.max(1);
            }
            if let (Some(x0), Some(y0), Some(x1), Some(y1)) =
                (read_i16(data, head.0 + 36), read_i16(data, head.0 + 38), read_i16(data, head.0 + 40), read_i16(data, head.0 + 42))
            {
                metrics.bbox = [x0, y0, x1, y1];
            }
        }

        let mut num_h_metrics = 0u16;
        if let Some(hhea) = tables.get(TAG_HHEA).copied() {
            if let Some(a) = read_i16(data, hhea.0 + 4) {
                metrics.ascent = a;
            }
            if let Some(d) = read_i16(data, hhea.0 + 6) {
                metrics.descent = d;
            }
            if let Some(n) = read_u16(data, hhea.0 + 34) {
                num_h_metrics = n;
            }
        }

        // `sCapHeight` only exists in OS/2 version 2+; older/absent tables
        // keep the proportional-to-em fallback set below.
        let mut cap_height_resolved = false;
        if let Some(os2) = tables.get(TAG_OS2).copied() {
            if let Some(version) = read_u16(data, os2.0) {
                if version >= 2 {
                    if let Some(ch) = read_i16(data, os2.0 + 88) {
                        metrics.cap_height = ch;
                        cap_height_resolved = true;
                    }
                }
            }
        }
        if !cap_height_resolved {
            metrics.cap_height = (0.7 * f64::from(metrics.units_per_em)).round() as i16;
        }

        let hmtx = tables.get(TAG_HMTX).copied();
        if let Some(cmap) = tables.get(TAG_CMAP).copied().and_then(|(o, l)| data.get(o..o + l)) {
            for byte in 0u16..=255 {
                let Some(codepoint) = super::winansi::decode(byte as u8) else { continue };
                let Some(glyph_id) = find_cmap_glyph(cmap, codepoint) else { continue };
                metrics.glyph_for_byte[byte as usize] = Some(glyph_id);
                if let Some((hmtx_off, hmtx_len)) = hmtx {
                    if let Some(hmtx_bytes) = data.get(hmtx_off..hmtx_off + hmtx_len) {
                        if let Some(advance) = advance_for_glyph(hmtx_bytes, num_h_metrics, glyph_id) {
                            metrics.advance_for_glyph.insert(glyph_id, advance);
                        }
                    }
                }
            }
        }

        metrics
    }

    /// This byte code's advance width, in `/1000`-em `/Widths` units (the
    /// convention every simple PDF font uses regardless of point size —
    /// see this module's own doc comment for why that's exactly what a
    /// per-glyph, size-INDEPENDENT font metric must be).
    pub(super) fn width_1000(&self, byte: u8) -> f64 {
        let Some(glyph_id) = self.glyph_for_byte[byte as usize] else { return FALLBACK_WIDTH_1000 };
        let Some(&advance_units) = self.advance_for_glyph.get(&glyph_id) else { return FALLBACK_WIDTH_1000 };
        f64::from(advance_units) * 1000.0 / f64::from(self.units_per_em.max(1))
    }

    pub(super) fn ascent_1000(&self) -> f64 {
        f64::from(self.ascent) * 1000.0 / f64::from(self.units_per_em.max(1))
    }

    pub(super) fn descent_1000(&self) -> f64 {
        f64::from(self.descent) * 1000.0 / f64::from(self.units_per_em.max(1))
    }

    pub(super) fn cap_height_1000(&self) -> f64 {
        f64::from(self.cap_height) * 1000.0 / f64::from(self.units_per_em.max(1))
    }

    pub(super) fn bbox_1000(&self) -> [f64; 4] {
        self.bbox.map(|v| f64::from(v) * 1000.0 / f64::from(self.units_per_em.max(1)))
    }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    read_u16(data, offset).map(|v| v as i16)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// `tag -> (byte_offset, byte_len)` for every table this font's directory
/// declares. `None` only when `data` is too short to even hold a table
/// directory header — every other malformed-table case degrades per-table
/// inside [`TtfMetrics::parse`] instead.
fn read_table_directory(data: &[u8]) -> Option<HashMap<[u8; 4], (usize, usize)>> {
    let num_tables = read_u16(data, 4)?;
    let mut tables = HashMap::with_capacity(num_tables as usize);
    for i in 0..num_tables as usize {
        let record_off = 12 + i * 16;
        let tag = data.get(record_off..record_off + 4)?;
        let offset = read_u32(data, record_off + 8)? as usize;
        let length = read_u32(data, record_off + 12)? as usize;
        let tag: [u8; 4] = tag.try_into().ok()?;
        tables.insert(tag, (offset, length));
    }
    Some(tables)
}

/// Platform/encoding preference order for a Unicode `cmap` subtable —
/// Windows BMP (3,1) first (what every font this crate actually embeds
/// ships), then the two Unicode platform-0 variants, then Windows-full
/// (3,10) — the last one is accepted only if it happens to also be format
/// 4 (this reader's only supported format; format 12's full-Unicode range
/// is out of scope, see this module's own doc comment).
const CMAP_PREFERENCE: [(u16, u16); 4] = [(3, 1), (0, 3), (0, 4), (3, 10)];

/// Look up `codepoint`'s glyph id in `cmap` (the whole `cmap` table's own
/// bytes, offset 0 = table start) via the first format-4 subtable found at
/// a preferred platform/encoding combination.
fn find_cmap_glyph(cmap: &[u8], codepoint: u32) -> Option<u16> {
    let num_tables = read_u16(cmap, 2)?;
    let mut best_offset: Option<usize> = None;
    for &(want_platform, want_encoding) in &CMAP_PREFERENCE {
        for i in 0..num_tables as usize {
            let record_off = 4 + i * 8;
            let platform = read_u16(cmap, record_off)?;
            let encoding = read_u16(cmap, record_off + 2)?;
            if platform == want_platform && encoding == want_encoding {
                let sub_offset = read_u32(cmap, record_off + 4)? as usize;
                if read_u16(cmap, sub_offset) == Some(4) {
                    best_offset = Some(sub_offset);
                    break;
                }
            }
        }
        if best_offset.is_some() {
            break;
        }
    }
    let sub_offset = best_offset?;
    let sub = cmap.get(sub_offset..)?;
    lookup_format4(sub, codepoint)
}

/// Standard cmap format-4 (segment mapping to delta values) lookup —
/// `sub` starts at the subtable's own `format` field (offset 0 = `4`).
fn lookup_format4(sub: &[u8], codepoint: u32) -> Option<u16> {
    if codepoint > 0xFFFF {
        return None; // format 4 only covers the BMP.
    }
    let c = codepoint as u16;

    let seg_count_x2 = read_u16(sub, 6)?;
    let seg_count = seg_count_x2 as usize / 2;

    let end_code_off = 14;
    let start_code_off = end_code_off + seg_count * 2 + 2; // +2 skips reservedPad
    let id_delta_off = start_code_off + seg_count * 2;
    let id_range_off_off = id_delta_off + seg_count * 2;

    for i in 0..seg_count {
        let end = read_u16(sub, end_code_off + i * 2)?;
        if c > end {
            continue;
        }
        let start = read_u16(sub, start_code_off + i * 2)?;
        if c < start {
            return None; // segments are sorted by end code; not covered.
        }
        let delta = read_i16(sub, id_delta_off + i * 2)?;
        let range_offset = read_u16(sub, id_range_off_off + i * 2)?;
        if range_offset == 0 {
            return Some(((c as i32 + delta as i32) & 0xFFFF) as u16);
        }
        let glyph_index_addr = id_range_off_off + i * 2 + range_offset as usize + 2 * (c - start) as usize;
        let raw = read_u16(sub, glyph_index_addr)?;
        if raw == 0 {
            return Some(0);
        }
        return Some(((raw as i32 + delta as i32) & 0xFFFF) as u16);
    }
    None
}

/// `hmtx`'s own reuse-last-advance rule: glyphs at or beyond
/// `num_h_metrics` share the LAST explicit `(advanceWidth, lsb)` entry.
fn advance_for_glyph(hmtx: &[u8], num_h_metrics: u16, glyph_id: u16) -> Option<u16> {
    if num_h_metrics == 0 {
        return None;
    }
    let idx = (glyph_id as usize).min(num_h_metrics as usize - 1);
    read_u16(hmtx, idx * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real, production font (Roboto Regular, read straight off disk via
    /// a relative `include_bytes!` — NOT a new Cargo dependency on
    /// `uzor-fonts`, see this crate's own dependency law) — ground truth
    /// for this hand-rolled reader against an actual shipped TrueType font.
    const ROBOTO_REGULAR: &[u8] = include_bytes!("../../../uzor-fonts/fonts/Roboto-Regular.ttf");

    #[test]
    fn parses_sane_units_per_em_and_bbox_from_a_real_font() {
        let m = TtfMetrics::parse(ROBOTO_REGULAR);
        assert!(m.units_per_em >= 500, "real fonts use a several-hundred-plus unit em square, got {}", m.units_per_em);
        let bbox = m.bbox_1000();
        assert!(bbox[2] > bbox[0], "bbox xMax must exceed xMin");
        assert!(bbox[3] > bbox[1], "bbox yMax must exceed yMin");
    }

    #[test]
    fn resolves_a_glyph_and_a_positive_width_for_ordinary_ascii_letters() {
        let m = TtfMetrics::parse(ROBOTO_REGULAR);
        for byte in b'A'..=b'Z' {
            assert!(m.glyph_for_byte[byte as usize].is_some(), "byte {byte:#x} ({}) must resolve a glyph", byte as char);
            assert!(m.width_1000(byte) > 0.0, "byte {byte:#x} must have a positive advance width");
        }
    }

    #[test]
    fn resolves_the_em_dash_glyph_used_by_this_crates_own_report_fixtures() {
        let m = TtfMetrics::parse(ROBOTO_REGULAR);
        assert!(m.glyph_for_byte[0x97].is_some(), "em-dash (WinAnsi byte 0x97) must resolve to a real glyph in Roboto");
    }

    #[test]
    fn distinct_letters_resolve_to_distinct_glyph_ids() {
        let m = TtfMetrics::parse(ROBOTO_REGULAR);
        let gid_a = m.glyph_for_byte[b'A' as usize];
        let gid_b = m.glyph_for_byte[b'B' as usize];
        assert!(gid_a.is_some() && gid_b.is_some());
        assert_ne!(gid_a, gid_b);
    }

    #[test]
    fn malformed_font_bytes_degrade_to_fallback_constants_without_panicking() {
        let m = TtfMetrics::parse(&[0u8; 4]); // far too short for any real table
        assert_eq!(m.units_per_em, 1000);
        assert_eq!(m.width_1000(b'A'), FALLBACK_WIDTH_1000);
        assert!(m.ascent_1000() > 0.0);
    }

    #[test]
    fn empty_font_bytes_do_not_panic() {
        let m = TtfMetrics::parse(&[]);
        assert_eq!(m.width_1000(b'A'), FALLBACK_WIDTH_1000);
    }
}
