//! Bundled font assets for the uzor UI framework.
//!
//! All fonts are embedded as `&[u8]` constants so they are available at
//! compile time without any file-system access at runtime.
//!
//! # Families
//!
//! - **Roboto** — default UI sans-serif (regular, bold, italic, bold-italic)
//! - **PT Root UI** — variable-weight sans-serif from Paratype (single VF file)
//! - **JetBrains Mono** — monospace with full Unicode box-drawing coverage
//! - **Symbols Nerd Font Mono** — Powerline / Nerd Font PUA / dev icons
//! - **Noto Sans Symbols 2** — wide symbol / mathematical coverage
//! - **Noto Color Emoji** — color emoji (COLRv1/v0 + CBDT bitmaps)
//! - **Noto Emoji** — color-neutral emoji fallback (legacy, all backends)

// ── Roboto ────────────────────────────────────────────────────────────────────

/// Roboto Regular (400).
pub static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");

/// Roboto Bold (700).
pub static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");

/// Roboto Italic (400 italic).
pub static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");

/// Roboto Bold Italic (700 italic).
pub static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ── PT Root UI ────────────────────────────────────────────────────────────────
//
// Variable font — a single file covers the full weight axis (100–900).
// Pass it to fontdue as-is; rasterisation at any weight works from one binary.

/// PT Root UI Variable Font (covers weight axis 100–900).
pub static PT_ROOT_UI_VF: &[u8] = include_bytes!("../fonts/PTRootUI-VF.ttf");

// ── JetBrains Mono ────────────────────────────────────────────────────────────
//
// Monospace font with full Unicode box-drawing coverage (U+2500–U+259F).
// Used for terminal-style rendering (PTY output, code blocks, etc.).

/// JetBrains Mono Regular — monospace with box-drawing support.
pub static JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../fonts/JetBrainsMono-Regular.ttf");

/// JetBrains Mono Bold.
pub static JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../fonts/JetBrainsMono-Bold.ttf");

// ── Fallback fonts ────────────────────────────────────────────────────────────

/// Symbols Nerd Font Mono — Powerline separators (U+E0B0–U+E0B3), Nerd Font PUA
/// (U+E000–U+F8FF), dev icons, and wide Unicode symbol coverage.
pub static SYMBOLS_NERD_FONT_MONO: &[u8] =
    include_bytes!("../fonts/SymbolsNerdFontMono-Regular.ttf");

/// Noto Sans Symbols 2 — wide symbol / mathematical coverage.
pub static NOTO_SANS_SYMBOLS2: &[u8] =
    include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf");

/// Noto Color Emoji — color emoji font containing both COLRv1/v0 vectors and CBDT bitmaps.
/// Supported natively by vello-gpu (skrifa 0.40+); other backends fall through
/// to the monochrome outline or skip to the next fallback.
pub static NOTO_COLOR_EMOJI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/NotoColorEmoji.ttf"));

/// DejaVu Sans Regular — broad BMP coverage. Fills the gap left by subsetted
/// Roboto (no arrows / dingbats / geometric shapes) and NotoSansSymbols2
/// (mostly supplementary U+1xxxx symbols).
///
/// Covers: Arrows (U+2190–21FF), General Punctuation, Math Operators,
/// Geometric Shapes (U+25A0–25FF), Box Drawing, Dingbats (U+2700–27BF, partial),
/// Letterlike Symbols. Bundled directly (~757 KB) — small enough not to need a
/// separate download.
pub static DEJAVU_SANS: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");

/// Noto Emoji — color-neutral emoji coverage (legacy, works on all backends).
pub static NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
