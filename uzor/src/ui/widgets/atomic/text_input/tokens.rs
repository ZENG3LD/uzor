//! Token-contract implementation of
//! [`TextInputTheme`](super::theme::TextInputTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "text_input"
//! section).
//!
//! `text_input` is the only one of the 30 widgets whose theme trait returns
//! `[u8; 4]` instead of `&str` (`TextInputTheme`'s render call sites already
//! feed byte arrays straight into `RenderContext`, never through
//! `parse_color`). [`TextInputTokens`] therefore uses
//! [`crate::tokens::component_tokens_bytes`] — the byte-array sibling of the
//! `component_tokens!` macro every `&str`-returning widget's own `tokens.rs`
//! uses — so the field list and the `component.text_input.*` key registry
//! still come from the ONE table, just resolved to `[u8; 4]` instead of a
//! pre-rendered `String`.
//!
//! All 10 fields change look vs. the pre-H1 `DefaultTextInputTheme` literals
//! — the whole trait was authored from Win32/generic defaults, never from
//! MLC's palette at all (companion doc: "the only widget in the whole 30
//! where this is total").
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens_bytes, ColorSpec, Role};

component_tokens_bytes! {
    widget = "text_input",
    struct TextInputTokens, keys = TEXT_INPUT_KEYS;

    bg_normal      => ColorSpec::Alias(Role::SurfaceControlIdle, None),
    bg_disabled    => ColorSpec::Alias(Role::SurfaceControlIdle, None),
    border_normal  => ColorSpec::Alias(Role::BorderDefault, None),
    border_hover   => ColorSpec::Alias(Role::BorderStrong, None),
    border_focused => ColorSpec::Alias(Role::FocusRing, None),
    text_normal    => ColorSpec::Alias(Role::TextOnAccent, None),
    text_disabled  => ColorSpec::Alias(Role::TextDisabled, None),
    placeholder    => ColorSpec::Alias(Role::TextMuted, None),
    // Windows-native-blue correction (foreign-blue ledger, main design doc
    // §1) — `selection`'s own role already carries its alpha suffix
    // (`#2962ff55`), no separate modifier needed.
    selection      => ColorSpec::Alias(Role::Selection, None),
    cursor         => ColorSpec::Alias(Role::TextOnAccent, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Rgba, Tokens};

    /// Parses a `#rrggbb`/`#rrggbbaa` literal into the `[u8; 4]` shape
    /// `TextInputTokens` fields hold — a small test-only convenience so the
    /// expectations below read as hex, matching every other widget's own
    /// `dark_defaults_match_the_companion_table` test.
    fn bytes(hex: &str) -> [u8; 4] {
        let c = Rgba::from_hex(hex).expect("valid hex literal");
        [c.r, c.g, c.b, c.a]
    }

    #[test]
    fn every_text_input_key_is_registered() {
        let keys =
            crate::tokens::component_keys("text_input").expect("text_input must be registered");
        assert_eq!(keys, TEXT_INPUT_KEYS);
        assert_eq!(keys.len(), 10, "one entry per TextInputTokens field");
        assert!(keys.contains(&"border_focused"));
        assert!(keys.contains(&"cursor"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.text_input();
        assert_eq!(t.bg_normal, bytes("#1e222d"));
        assert_eq!(t.bg_disabled, bytes("#1e222d"));
        assert_eq!(t.border_normal, bytes("#363a45"));
        assert_eq!(t.border_hover, bytes("#363a45"), "border.strong == border.default in dark");
        assert_eq!(t.border_focused, bytes("#2962ff"));
        assert_eq!(t.text_normal, bytes("#d1d4dc"));
        assert_eq!(t.text_disabled, bytes("#6a6d78"));
        assert_eq!(t.placeholder, bytes("#787b86"));
        assert_eq!(t.selection, bytes("#2962ff55"));
        assert_eq!(t.cursor, bytes("#d1d4dc"));
    }
}
