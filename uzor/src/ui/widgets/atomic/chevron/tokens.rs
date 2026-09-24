//! Token-contract implementation of [`ChevronTheme`](super::theme::ChevronTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "chevron" section).
//!
//! [`ChevronTokens`] holds one pre-rendered `String` per `ChevronTheme`
//! method — all 6 are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "chevron",
    struct ChevronTokens, keys = CHEVRON_KEYS;

    color          => ColorSpec::Alias(Role::TextSecondary, None),
    color_hover    => ColorSpec::Alias(Role::TextPrimary, None),
    color_pressed  => ColorSpec::Alias(Role::TextMuted, None),
    color_disabled => ColorSpec::Alias(Role::TextDisabled, None),
    color_active   => ColorSpec::Alias(Role::AccentDefault, None),
    bg_hover       => ColorSpec::Alias(Role::SurfaceControlHover, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_chevron_key_is_registered() {
        let keys = crate::tokens::component_keys("chevron").expect("chevron must be registered");
        assert_eq!(keys, CHEVRON_KEYS);
        assert_eq!(keys.len(), 6, "one entry per ChevronTokens field");
        assert!(keys.contains(&"color"));
        assert!(keys.contains(&"bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.chevron();
        assert_eq!(c.color, "#b2b5be");
        assert_eq!(c.color_hover, "#d1d4dc");
        assert_eq!(c.color_pressed, "#787b86");
        assert_eq!(c.color_disabled, "#6a6d78");
        assert_eq!(c.color_active, "#2962ff");
        assert_eq!(c.bg_hover, "#2a2e39");
    }
}
