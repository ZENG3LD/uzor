//! Centralized font catalog.
//!
//! Backends should import font bytes from here rather than embedding their own copies.
//! This ensures all backends share the same font set without duplication.
//!
//! # Available families
//!
//! - **Roboto** — default UI font (regular, bold, italic, bold-italic)
//! - **PT Root UI** — variable-weight sans-serif from Paratype (single VF file)
//! - **Noto Sans Symbols2** — symbol fallback coverage
//! - **Noto Emoji** — emoji fallback coverage

// ── Roboto ────────────────────────────────────────────────────────────────────

/// Roboto Regular (400).
pub static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");

/// Roboto Bold (700).
pub static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");

/// Roboto Italic (400 italic).
pub static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");

/// Roboto Bold Italic (700 italic).
pub static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ── PT Root UI ─────────────────────────────────────────────────────────────
//
// Variable font — a single file covers the full weight axis (100–900).
// Pass it to fontdue as-is; rasterisation at any weight works from one binary.

/// PT Root UI Variable Font (covers weight axis 100–900).
pub static PT_ROOT_UI_VF: &[u8] = include_bytes!("../fonts/PTRootUI-VF.ttf");

// ── Fallback fonts ────────────────────────────────────────────────────────────

/// Noto Sans Symbols 2 — wide symbol / mathematical coverage.
pub static NOTO_SANS_SYMBOLS2: &[u8] =
    include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf");

/// Noto Emoji — color-neutral emoji coverage.
pub static NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
