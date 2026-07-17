//! `WinAnsiEncoding` (PDF spec Appendix D, == Windows-1252) char <-> byte
//! conversion — the single named `/Encoding` this module's simple TrueType
//! fonts declare (`pdf::mod`'s own doc comment explains why a simple font,
//! not a Type0/CID font, is this phase's chosen shape).
//!
//! Both directions derive from the SAME `HIGH_TABLE` (the one block of
//! `WinAnsiEncoding` that differs from a plain Latin-1 identity mapping,
//! bytes `0x80..=0x9F`) so encode/decode can never drift apart — verified
//! against `lopdf`'s own `WIN_ANSI_ENCODING` decode table (the crate this
//! module's own round-trip test parses output PDFs with) byte-for-byte at
//! every byte this module's tests exercise.

/// The `0x80..=0x9F` block where `WinAnsiEncoding` diverges from a plain
/// Latin-1 identity byte-equals-codepoint mapping. `(byte, codepoint)`.
const HIGH_TABLE: &[(u8, u32)] = &[
    (0x80, 0x20AC), // Euro
    (0x82, 0x201A), // single low-9 quotation mark
    (0x83, 0x0192), // florin
    (0x84, 0x201E), // double low-9 quotation mark
    (0x85, 0x2026), // horizontal ellipsis
    (0x86, 0x2020), // dagger
    (0x87, 0x2021), // double dagger
    (0x88, 0x02C6), // modifier letter circumflex accent
    (0x89, 0x2030), // per mille sign
    (0x8A, 0x0160), // Scaron
    (0x8B, 0x2039), // single left-pointing angle quotation mark
    (0x8C, 0x0152), // OE
    (0x8E, 0x017D), // Zcaron
    (0x91, 0x2018), // left single quotation mark
    (0x92, 0x2019), // right single quotation mark
    (0x93, 0x201C), // left double quotation mark
    (0x94, 0x201D), // right double quotation mark
    (0x95, 0x2022), // bullet
    (0x96, 0x2013), // en dash
    (0x97, 0x2014), // em dash
    (0x98, 0x02DC), // small tilde
    (0x99, 0x2122), // trademark sign
    (0x9A, 0x0161), // scaron
    (0x9B, 0x203A), // single right-pointing angle quotation mark
    (0x9C, 0x0153), // oe
    (0x9E, 0x017E), // zcaron
    (0x9F, 0x0178), // Ydieresis
];

/// Byte codes `WinAnsiEncoding` leaves undefined in `0x80..=0x9F`; the PDF
/// spec's own Appendix D fills every one of them with `bullet` on decode
/// (matches `lopdf`'s `WIN_ANSI_ENCODING` table exactly at these six
/// positions) — never used as an ENCODE target (bullet's own canonical
/// byte is `0x95`, see [`encode`]), only relevant for [`decode`].
const UNDEFINED_TO_BULLET: [u8; 6] = [0x7F, 0x81, 0x8D, 0x8F, 0x90, 0x9D];

/// Decode a `WinAnsiEncoding` byte code to its Unicode codepoint. Used only
/// by [`super::ttf`] to look up a font's `cmap` per byte code (never part
/// of this crate's public surface — text ENCODING, [`encode`], is the only
/// direction a caller needs).
pub(super) fn decode(byte: u8) -> Option<u32> {
    match byte {
        0x20..=0x7E | 0xA0..=0xFF => Some(byte as u32),
        b if UNDEFINED_TO_BULLET.contains(&b) => Some(0x2022),
        b => HIGH_TABLE.iter().find(|&&(hb, _)| hb == b).map(|&(_, cp)| cp),
    }
}

/// Encode `ch` as its `WinAnsiEncoding` byte code, or `None` if `ch` has no
/// slot in the table at all (CJK, emoji, anything outside Latin-1 plus the
/// `0x80..=0x9F` punctuation block above) — [`super::text_to_winansi_bytes`]
/// substitutes `'?'` for an unencodable character rather than dropping it
/// silently.
pub(super) fn encode(ch: char) -> Option<u8> {
    let cp = ch as u32;
    match cp {
        0x20..=0x7E | 0xA0..=0xFF => Some(cp as u8),
        _ => HIGH_TABLE.iter().find(|&&(_, hcp)| hcp == cp).map(|&(hb, _)| hb),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_identity_both_ways() {
        for b in 0x20u8..=0x7E {
            assert_eq!(decode(b), Some(b as u32));
            assert_eq!(encode(b as char), Some(b));
        }
    }

    #[test]
    fn em_dash_and_bullet_round_trip_through_their_canonical_bytes() {
        assert_eq!(encode('\u{2014}'), Some(0x97));
        assert_eq!(decode(0x97), Some(0x2014));
        assert_eq!(encode('\u{2022}'), Some(0x95));
        assert_eq!(decode(0x95), Some(0x2022));
    }

    #[test]
    fn undefined_high_bytes_decode_to_bullet_but_bullet_encodes_to_its_own_byte_only() {
        for &b in &UNDEFINED_TO_BULLET {
            assert_eq!(decode(b), Some(0x2022));
        }
        // Encoding must be unambiguous: bullet always picks 0x95, never one
        // of the undefined fallback bytes.
        assert_eq!(encode('\u{2022}'), Some(0x95));
    }

    #[test]
    fn latin1_supplement_is_identity_both_ways() {
        assert_eq!(decode(0xE9), Some(0xE9)); // 'é'
        assert_eq!(encode('\u{00e9}'), Some(0xE9));
    }

    #[test]
    fn unencodable_characters_return_none() {
        assert_eq!(encode('\u{4e2d}'), None); // CJK
        assert_eq!(encode('\u{1F600}'), None); // emoji (astral)
    }
}
