//! Token-contract implementation of
//! [`ScrollChevronTheme`](super::theme::ScrollChevronTheme) (H1 token
//! contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "scroll_chevron"
//! section).
//!
//! [`ScrollChevronTokens`] holds one pre-rendered `String` per
//! `ScrollChevronTheme` method — all 4 are required (no default-body
//! delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "scroll_chevron",
    struct ScrollChevronTokens, keys = SCROLL_CHEVRON_KEYS;

    scroll_chevron_color          => ColorSpec::Alias(Role::TextPrimary, None),
    scroll_chevron_color_hover    => ColorSpec::Alias(Role::TextOnAccent, None),
    scroll_chevron_color_disabled => ColorSpec::Alias(Role::TextDisabled, None),
    scroll_chevron_bg_hover       => ColorSpec::Alias(Role::SurfaceControlHover, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_scroll_chevron_key_is_registered() {
        let keys = crate::tokens::component_keys("scroll_chevron")
            .expect("scroll_chevron must be registered");
        assert_eq!(keys, SCROLL_CHEVRON_KEYS);
        assert_eq!(keys.len(), 4, "one entry per ScrollChevronTokens field");
        assert!(keys.contains(&"scroll_chevron_color"));
        assert!(keys.contains(&"scroll_chevron_bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let s = tokens.scroll_chevron();
        assert_eq!(s.scroll_chevron_color, "#d1d4dc");
        assert_eq!(s.scroll_chevron_color_hover, "#d1d4dc");
        assert_eq!(s.scroll_chevron_color_disabled, "#6a6d78");
        assert_eq!(s.scroll_chevron_bg_hover, "#2a2e39");
    }
}
