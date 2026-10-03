//! Unit conversions (pt -> twips/half-points/EMU) + XML text escaping —
//! every OOXML-emitting module in `docx/` goes through these, never a
//! second ad-hoc conversion.

use uzor::fonts::FontFamily;

use crate::press::Palette;

/// The installed-font name Word should ask the OS for — bare name, no
/// weight/style suffix (`<w:b/>`/`<w:i/>` carry those separately). Every
/// current `FontFamily` variant ships in `uzor-fonts` under exactly this
/// name.
pub(crate) fn font_name(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Roboto => "Roboto",
        FontFamily::PtRootUi => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
        FontFamily::SourceSerif4 => "Source Serif 4",
    }
}

/// The font name written into DOCX runs: the theme's `docx_font` when it
/// names one (a face the reader has installed but the press does not bundle),
/// otherwise the bundled family's own name.
pub(crate) fn docx_font_name(palette: &Palette, family: FontFamily) -> String {
    palette.doc.docx_font.clone().unwrap_or_else(|| font_name(family).to_owned())
}

/// Points -> twips (1 pt = 20 twips) — `<w:sz>`'s own sibling measurements
/// (`<w:ind>`, `<w:pgSz>`, `<w:pgMar>`, table grid columns, ...) all use
/// twips.
pub(crate) fn pt_to_twips(pt: f64) -> i64 {
    (pt * 20.0).round() as i64
}

/// Points -> half-points (1 pt = 2 half-points) — `<w:sz>`/`<w:szCs>`'s own
/// unit for font sizes and border widths.
pub(crate) fn pt_to_half_points(pt: f64) -> i64 {
    (pt * 2.0).round() as i64
}

/// Points -> EMU (1 pt = 12700 EMU) — `<wp:extent>`'s own unit for
/// DrawingML image sizing.
pub(crate) fn pt_to_emu(pt: f64) -> i64 {
    (pt * 12700.0).round() as i64
}

/// `0xRRGGBB` -> `"RRGGBB"` (uppercase, no `#`) — the bare hex OOXML color
/// attributes (`w:color w:val`, `w:fill`, ...) expect.
pub(crate) fn ooxml_hex(color: u32) -> String {
    format!("{:06X}", color & 0xff_ffff)
}

/// Escape `&`/`<`/`>` for safe placement inside XML text content or
/// attribute values — every other UTF-8 byte (including Cyrillic) passes
/// through untouched. Callers add `xml:space="preserve"` themselves when a
/// run's own leading/trailing space must survive Word's default
/// whitespace collapsing.
pub(crate) fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_font_name_prefers_the_theme_override_and_falls_back_to_the_family() {
        assert_eq!(docx_font_name(&Palette::light(), FontFamily::Roboto), "Roboto");
        let mut palette = Palette::light();
        palette.doc.docx_font = Some("Example Face".to_owned());
        assert_eq!(docx_font_name(&palette, FontFamily::Roboto), "Example Face");
    }

    #[test]
    fn pt_to_twips_scales_by_20() {
        assert_eq!(pt_to_twips(1.0), 20);
        assert_eq!(pt_to_twips(12.0), 240);
        assert_eq!(pt_to_twips(0.0), 0);
        // Rounds to the nearest twip rather than truncating.
        assert_eq!(pt_to_twips(1.026), 21);
    }

    #[test]
    fn pt_to_half_points_scales_by_2() {
        assert_eq!(pt_to_half_points(11.0), 22);
        assert_eq!(pt_to_half_points(10.5), 21);
        assert_eq!(pt_to_half_points(0.0), 0);
    }

    #[test]
    fn pt_to_emu_scales_by_12700() {
        assert_eq!(pt_to_emu(1.0), 12700);
        assert_eq!(pt_to_emu(72.0), 914_400); // 1 inch, the well-known EMU constant
    }

    #[test]
    fn ooxml_hex_formats_uppercase_no_hash() {
        assert_eq!(ooxml_hex(0x2E74B5), "2E74B5");
        assert_eq!(ooxml_hex(0x00ff00), "00FF00");
    }

    #[test]
    fn escape_xml_text_escapes_reserved_characters_only() {
        assert_eq!(escape_xml_text("a & b < c > d"), "a &amp; b &lt; c &gt; d");
        // Cyrillic and other UTF-8 pass through untouched.
        assert_eq!(escape_xml_text("Отчёт «пример»"), "Отчёт «пример»");
        assert_eq!(escape_xml_text("no special chars"), "no special chars");
    }
}
