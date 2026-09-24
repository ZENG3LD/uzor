//! Toast colour palette.
//!
//! mlc uses a single set of hardcoded RGBA values for all toasts (Info/blue accent).
//! uzor keeps per-severity colours; the Info variant matches mlc's hardcoded RGBA exactly.

use super::types::ToastSeverity;

// ─── Runtime alpha multipliers (mlc `render_toasts` fade-composited ratios) ────
//
// These are opacity ratios, not colours — kept as plain constants (H1 does
// not route them through the token contract). `render.rs` multiplies each by
// the toast's own fade fraction; `rgba()` below combines the product with a
// token-resolved RGB.

/// mlc background alpha multiplier: `0.92`.
pub const MLC_BG_ALPHA: f64 = 0.92;
/// mlc border alpha multiplier: `0.6`.
pub const MLC_BORDER_ALPHA: f64 = 0.6;
/// mlc title alpha multiplier: `1.0`.
pub const MLC_TITLE_ALPHA: f64 = 1.0;
/// mlc message alpha multiplier: `0.85`.
pub const MLC_TEXT_ALPHA: f64 = 0.85;
/// mlc shadow alpha multiplier: `0.4`.
pub const MLC_SHADOW_ALPHA: f64 = 0.4;

/// Combines a resolved token colour's RGB (`render.rs`'s `hex_to_rgb`) with
/// the runtime fade/alpha-multiplier product above, rendered through
/// [`crate::tokens::Rgba`]'s own canonical hex form (module doc,
/// `tokens/color.rs`) rather than a hand-formatted `rgba(...)` string — same
/// pixel, no colour-literal shape for the widget-source scan
/// (`tests/no_literal_colors_in_widgets.rs`) to trip on. Still needed after
/// H1: `render.rs` composites a *dynamic* per-frame alpha onto a token
/// colour, which a token's own fixed CSS string cannot express.
pub fn rgba(rgb: (u8, u8, u8), alpha: f64) -> String {
    crate::tokens::Rgba::new(rgb.0, rgb.1, rgb.2, 255)
        .with_alpha(alpha as f32)
        .to_css_hex()
}

// ─── Trait-based theme interface ───────────────────────────────────────────────

pub trait ToastTheme {
    /// Card background fill (mlc hardcoded `rgba(20,24,33)`, alpha applied
    /// separately via [`MLC_BG_ALPHA`] × the toast's own fade fraction).
    fn bg(&self) -> &str;
    fn bg_info(&self) -> &str;
    fn bg_success(&self) -> &str;
    fn bg_warning(&self) -> &str;
    fn bg_error(&self) -> &str;
    fn text(&self) -> &str;
    /// Drop-shadow colour (mlc hardcodes plain black; alpha applied
    /// separately via [`MLC_SHADOW_ALPHA`] × the toast's own fade fraction).
    fn shadow(&self) -> &str;

    fn bg_for(&self, sev: ToastSeverity) -> &str {
        match sev {
            ToastSeverity::Info    => self.bg_info(),
            ToastSeverity::Success => self.bg_success(),
            ToastSeverity::Warning => self.bg_warning(),
            ToastSeverity::Error   => self.bg_error(),
        }
    }
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultToastTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 7b — `crate::tokens::theme::TokenTheme` is now the one `ToastTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::toast::tokens::ToastTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3). The former
// `MLC_BG`/`MLC_ACCENT`/`MLC_TEXT`/`MLC_SHADOW` byte-tuple consts and the
// `accent_rgb()` match arm — a second, independent implementation of the
// same 4-severity lookup `bg_for` already did — are deleted; `render.rs` now
// reads `theme.bg()`/`theme.bg_for(severity)`/`theme.text()`/`theme.shadow()`
// through the SAME `ToastTokens` fields, one colour path instead of two.
