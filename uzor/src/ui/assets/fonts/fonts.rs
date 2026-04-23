//! Centralized font catalog.
//!
//! Backends should import font bytes from here rather than embedding their own copies.
//! This ensures all backends share the same font set without duplication.
//!
//! Font byte constants are provided by the `uzor-fonts` crate and re-exported here
//! so that all existing `crate::fonts::*` imports continue to work unchanged.
//!
//! # Available families
//!
//! - **Roboto** — default UI font (regular, bold, italic, bold-italic)
//! - **PT Root UI** — variable-weight sans-serif from Paratype (single VF file)
//! - **JetBrains Mono** — monospace with full Unicode box-drawing coverage
//!   (U+2500–U+259F). Used for terminal output, code blocks, etc.
//! - **Symbols Nerd Font Mono** — Powerline (U+E0B0–U+E0B3), Nerd Font PUA, dev icons
//! - **Noto Sans Symbols2** — wide symbol / mathematical coverage
//! - **Noto Color Emoji** — color emoji (COLRv1/v0 + CBDT bitmaps) for vello-gpu
//! - **Noto Emoji** — color-neutral emoji fallback (legacy bitmap-compatible)

// Re-export all font byte constants from the dedicated assets crate.
pub use uzor_fonts::{
    JETBRAINS_MONO_BOLD,
    JETBRAINS_MONO_REGULAR,
    NOTO_COLOR_EMOJI,
    NOTO_EMOJI,
    NOTO_SANS_SYMBOLS2,
    PT_ROOT_UI_VF,
    ROBOTO_BOLD,
    ROBOTO_BOLD_ITALIC,
    ROBOTO_ITALIC,
    ROBOTO_REGULAR,
    SYMBOLS_NERD_FONT_MONO,
};

// ── Backend-agnostic font family + CSS parser ────────────────────────────────
//
// All backends share one source of truth for CSS font-string parsing and
// family → bytes resolution. Each backend wraps these bytes in its own loader
// (`fontdue::Font`, `peniko::FontData`, `cosmic_text::fontdb::Source`, …) but
// the family detection logic lives here so new families only need to be added
// once.

/// Logical font family used by all uzor backends.
///
/// - **Roboto** — default UI sans-serif. Four style variants bundled.
/// - **PtRootUi** — variable-weight sans-serif from Paratype (single VF file).
///   Italic is not available — italic requests fall back to regular.
/// - **JetBrainsMono** — monospace with full Unicode box-drawing coverage
///   (U+2500–U+259F). Used for terminal output, code blocks, etc. Only
///   regular + bold variants are bundled; italic requests use regular.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Roboto,
    PtRootUi,
    JetBrainsMono,
}

/// Parsed CSS font-shorthand result.
#[derive(Clone, Debug)]
pub struct FontInfo {
    /// Font size in logical pixels.
    pub size:   f32,
    /// Whether a bold weight was requested.
    pub bold:   bool,
    /// Whether an italic style was requested.
    pub italic: bool,
    /// Resolved family after detecting the family keywords in the CSS string.
    pub family: FontFamily,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            size:   12.0,
            bold:   false,
            italic: false,
            family: FontFamily::Roboto,
        }
    }
}

/// Detect whether a (case-insensitive) CSS font string requests PT Root UI.
///
/// Accepted spellings: `"pt root ui"`, `"pt-root-ui"`, `"ptrootui"`.
fn is_pt_root_ui(font_str_lower: &str) -> bool {
    font_str_lower.contains("pt root ui")
        || font_str_lower.contains("pt-root-ui")
        || font_str_lower.contains("ptrootui")
}

/// Detect whether a (case-insensitive) CSS font string requests a monospace
/// family.
///
/// Matches the generic `"monospace"` keyword, explicit `"jetbrains mono"` /
/// `"jetbrainsmono"` / `"jb mono"`, and common system monospace fallback
/// names (`consolas`, `courier`, `cascadia`, `fira code`, `fira mono`). All
/// of these resolve to bundled JetBrains Mono which covers the full U+2500
/// box-drawing block needed for terminal emulation.
fn is_monospace(font_str_lower: &str) -> bool {
    font_str_lower.contains("jetbrains")
        || font_str_lower.contains("jb mono")
        || font_str_lower.contains("monospace")
        || font_str_lower.contains("consolas")
        || font_str_lower.contains("courier")
        || font_str_lower.contains("cascadia")
        || font_str_lower.contains("fira code")
        || font_str_lower.contains("fira mono")
}

/// Resolve a CSS family fragment (e.g. the family portion of the CSS font
/// shorthand) to a [`FontFamily`].
///
/// Monospace keywords win over PT Root UI which wins over the default Roboto.
pub fn resolve_family(family_str: &str) -> FontFamily {
    let lower = family_str.to_ascii_lowercase();
    if is_monospace(&lower) {
        FontFamily::JetBrainsMono
    } else if is_pt_root_ui(&lower) {
        FontFamily::PtRootUi
    } else {
        FontFamily::Roboto
    }
}

/// Parse a CSS font shorthand string into a [`FontInfo`].
///
/// Recognised tokens (case-insensitive, any order):
/// - `"bold"` — sets `bold = true`
/// - `"italic"` — sets `italic = true`
/// - `"<N>px"` — sets `size = N`
/// - Any other token is accumulated as family text and fed to
///   [`resolve_family`].
///
/// Unrecognised or empty strings produce [`FontInfo::default`] (Roboto 12px).
pub fn parse_css_font(font_str: &str) -> FontInfo {
    let lower = font_str.to_ascii_lowercase();
    let mut info = FontInfo::default();
    let mut family_parts: Vec<&str> = Vec::new();

    for part in lower.split_whitespace() {
        match part {
            "bold"   => info.bold = true,
            "italic" => info.italic = true,
            s if s.ends_with("px") => {
                if let Ok(sz) = s.trim_end_matches("px").parse::<f32>() {
                    info.size = sz;
                }
            }
            other => family_parts.push(other),
        }
    }

    if !family_parts.is_empty() {
        info.family = resolve_family(&family_parts.join(" "));
    }

    info
}

/// Return the static byte slice for the requested family / style combination.
///
/// - [`FontFamily::Roboto`] has all four style combinations.
/// - [`FontFamily::PtRootUi`] is a variable font — the same bytes are returned
///   for every (bold, italic) combination and backends handle the weight axis
///   themselves.
/// - [`FontFamily::JetBrainsMono`] has regular + bold only; italic requests
///   fall back to the non-italic variant of the requested weight.
pub fn font_bytes(family: FontFamily, bold: bool, italic: bool) -> &'static [u8] {
    match family {
        FontFamily::Roboto => match (bold, italic) {
            (true,  true ) => ROBOTO_BOLD_ITALIC,
            (true,  false) => ROBOTO_BOLD,
            (false, true ) => ROBOTO_ITALIC,
            (false, false) => ROBOTO_REGULAR,
        },
        FontFamily::PtRootUi => PT_ROOT_UI_VF,
        FontFamily::JetBrainsMono => {
            let _ = italic; // no italic variant bundled
            if bold { JETBRAINS_MONO_BOLD } else { JETBRAINS_MONO_REGULAR }
        }
    }
}

/// Convenience wrapper around [`parse_css_font`] that returns only the family
/// resolution result.
pub fn family_from_css_font(font_str: &str) -> FontFamily {
    parse_css_font(font_str).family
}
