//! Token-contract implementation of [`ToastTheme`](super::theme::ToastTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "toast" section).
//!
//! [`ToastTokens`] holds one pre-rendered `String` per `ToastTheme` method —
//! the 5 companion-table fields (`bg_info`/`bg_success`/`bg_warning`/
//! `bg_error`/`text`) plus two new required methods (`bg`, `shadow`) that
//! collapse the former `MLC_BG`/`MLC_SHADOW` byte-tuple consts and the
//! `accent_rgb()` free function's own literal RGB match arms into this SAME
//! struct — one colour path instead of two (companion doc's "Additionally"
//! paragraph).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Rgba, Role};

component_tokens! {
    widget = "toast",
    struct ToastTokens, keys = TOAST_KEYS;

    // Card background (former `MLC_BG` = `rgba(20,24,33)`, corrected onto
    // `surface.floating` — the companion doc's own delta note).
    bg => ColorSpec::Alias(Role::SurfaceFloating, None),

    // MLC's own chat-blue (`#3b82f6`) vs. TradingView-blue — corrected onto
    // `status.info` (= `accent.default`), main design doc §1.
    bg_info    => ColorSpec::Alias(Role::StatusInfo, None),
    bg_success => ColorSpec::Alias(Role::StatusSuccess, None),
    bg_warning => ColorSpec::Alias(Role::StatusWarning, None),
    bg_error   => ColorSpec::Alias(Role::StatusDanger, None),
    text       => ColorSpec::Alias(Role::TextOnAccent, None),

    // Non-semantic literal (H1 companion doc "scope note on non-semantic
    // literals"): mlc hardcodes a plain black drop-shadow in every preset,
    // not a themed colour — stays literal here rather than aliased to a
    // role, same treatment as `button`'s checkerboard tiles.
    shadow => ColorSpec::Literal(ColorValue::Solid(Rgba::new(0, 0, 0, 255))),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_toast_key_is_registered() {
        let keys = crate::tokens::component_keys("toast").expect("toast must be registered");
        assert_eq!(keys, TOAST_KEYS);
        assert_eq!(keys.len(), 7, "one entry per ToastTokens field");
        assert!(keys.contains(&"bg"));
        assert!(keys.contains(&"shadow"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.toast();
        assert_eq!(t.bg, "#1e222d");
        assert_eq!(t.bg_info, "#2962ff");
        assert_eq!(t.bg_success, "#26a69a");
        assert_eq!(t.bg_warning, "#ff9800");
        assert_eq!(t.bg_error, "#f23645");
        assert_eq!(t.text, "#d1d4dc");
        assert_eq!(t.shadow, "#000000");
    }
}
