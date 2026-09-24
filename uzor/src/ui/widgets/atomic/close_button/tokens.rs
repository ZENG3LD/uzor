//! Token-contract implementation of
//! [`CloseButtonTheme`](super::theme::CloseButtonTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "close_button"
//! section).
//!
//! [`CloseButtonTokens`] holds one pre-rendered `String` per
//! `CloseButtonTheme` method — all 3 are required (no default-body
//! delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "close_button",
    struct CloseButtonTokens, keys = CLOSE_BUTTON_KEYS;

    close_button_x_color       => ColorSpec::Alias(Role::TextMuted, None),
    close_button_x_color_hover => ColorSpec::Alias(Role::TextOnAccent, None),
    close_button_bg_hover      => ColorSpec::Alias(Role::SurfaceControlHover, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_close_button_key_is_registered() {
        let keys =
            crate::tokens::component_keys("close_button").expect("close_button must be registered");
        assert_eq!(keys, CLOSE_BUTTON_KEYS);
        assert_eq!(keys.len(), 3, "one entry per CloseButtonTokens field");
        assert!(keys.contains(&"close_button_x_color"));
        assert!(keys.contains(&"close_button_bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.close_button();
        assert_eq!(c.close_button_x_color, "#787b86");
        assert_eq!(c.close_button_x_color_hover, "#d1d4dc");
        assert_eq!(c.close_button_bg_hover, "#2a2e39");
    }
}
