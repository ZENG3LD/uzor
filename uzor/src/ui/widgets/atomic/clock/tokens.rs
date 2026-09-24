//! Token-contract implementation of [`ClockTheme`](super::theme::ClockTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "clock" section).
//!
//! [`ClockTokens`] holds one pre-rendered `String` per `ClockTheme` method —
//! both are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "clock",
    struct ClockTokens, keys = CLOCK_KEYS;

    clock_text     => ColorSpec::Alias(Role::TextPrimary, None),
    clock_bg_hover => ColorSpec::Alias(Role::SurfaceControlHover, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_clock_key_is_registered() {
        let keys = crate::tokens::component_keys("clock").expect("clock must be registered");
        assert_eq!(keys, CLOCK_KEYS);
        assert_eq!(keys.len(), 2, "one entry per ClockTokens field");
        assert!(keys.contains(&"clock_text"));
        assert!(keys.contains(&"clock_bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.clock();
        assert_eq!(c.clock_text, "#d1d4dc");
        assert_eq!(c.clock_bg_hover, "#2a2e39");
    }
}
