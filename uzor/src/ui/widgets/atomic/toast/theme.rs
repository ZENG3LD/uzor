//! Toast colour palette.
//!
//! mlc uses a single set of hardcoded RGBA values for all toasts (Info/blue accent).
//! uzor keeps per-severity colours; the Info variant matches mlc's hardcoded RGBA exactly.

use super::types::ToastSeverity;

// ─── mlc hardcoded RGBA (verbatim from `render_toasts`) ───────────────────────

/// mlc background: `rgba(20,24,33)`.
pub const MLC_BG: (u8, u8, u8) = (20, 24, 33);
/// mlc background alpha multiplier: `0.92`.
pub const MLC_BG_ALPHA: f64 = 0.92;

/// mlc border/title accent: `rgba(59,130,246)`.
pub const MLC_ACCENT: (u8, u8, u8) = (59, 130, 246);
/// mlc border alpha multiplier: `0.6`.
pub const MLC_BORDER_ALPHA: f64 = 0.6;
/// mlc title alpha multiplier: `1.0`.
pub const MLC_TITLE_ALPHA: f64 = 1.0;

/// mlc message text: `rgba(220,220,230)`.
pub const MLC_TEXT: (u8, u8, u8) = (220, 220, 230);
/// mlc message alpha multiplier: `0.85`.
pub const MLC_TEXT_ALPHA: f64 = 0.85;

/// mlc drop-shadow: `rgba(0,0,0)`.
pub const MLC_SHADOW: (u8, u8, u8) = (0, 0, 0);
/// mlc shadow alpha multiplier: `0.4`.
pub const MLC_SHADOW_ALPHA: f64 = 0.4;

// ─── Per-severity accent colours (uzor extension) ─────────────────────────────

/// Formats an `rgba(r,g,b,a)` string with a pre-computed combined alpha.
pub fn rgba(rgb: (u8, u8, u8), alpha: f64) -> String {
    format!("rgba({},{},{},{:.2})", rgb.0, rgb.1, rgb.2, alpha)
}

/// Accent RGB per severity.  Info = mlc blue.  Others are uzor extensions.
pub fn accent_rgb(sev: ToastSeverity) -> (u8, u8, u8) {
    match sev {
        ToastSeverity::Info    => MLC_ACCENT,              // (59,130,246) — mlc blue
        ToastSeverity::Success => (34, 197, 94),            // green-500
        ToastSeverity::Warning => (234, 179, 8),            // yellow-500
        ToastSeverity::Error   => (239, 68, 68),            // red-500
    }
}

// ─── Trait-based theme interface (kept for callers that already use it) ────────

pub trait ToastTheme {
    fn bg_info(&self) -> &str;
    fn bg_success(&self) -> &str;
    fn bg_warning(&self) -> &str;
    fn bg_error(&self) -> &str;
    fn text(&self) -> &str;

    fn bg_for(&self, sev: ToastSeverity) -> &str {
        match sev {
            ToastSeverity::Info    => self.bg_info(),
            ToastSeverity::Success => self.bg_success(),
            ToastSeverity::Warning => self.bg_warning(),
            ToastSeverity::Error   => self.bg_error(),
        }
    }
}

/// Legacy default theme — plain CSS hex strings.
/// The mlc-parity render path uses `ToastThemeColors` instead.
#[derive(Default)]
pub struct DefaultToastTheme;

impl ToastTheme for DefaultToastTheme {
    fn bg_info(&self)    -> &str { "#3b82f6" }   // same blue as mlc accent
    fn bg_success(&self) -> &str { "#22c55e" }
    fn bg_warning(&self) -> &str { "#eab308" }
    fn bg_error(&self)   -> &str { "#ef4444" }
    fn text(&self)       -> &str { "#dcdce6" }   // same as mlc MLC_TEXT
}
