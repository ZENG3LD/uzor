//! Token-contract implementation of
//! [`SeparatorTheme`](super::theme::SeparatorTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "separator"
//! section).
//!
//! [`SeparatorTokens`] holds one pre-rendered `String` per `SeparatorTheme`
//! method — all 7 are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "separator",
    struct SeparatorTokens, keys = SEPARATOR_KEYS;

    line          => ColorSpec::Alias(Role::BorderDefault, None),
    handle_hover  => ColorSpec::Alias(Role::TextMuted, None),
    handle_active => ColorSpec::Alias(Role::AccentDefault, None),

    pane_handle_idle => ColorSpec::Alias(Role::BorderDefault, None),
    // The old literal (`#758696`) is flagged as inconsistent by its own
    // trait doc comment ("or bright white in some themes") — consolidated
    // onto the accent role (companion doc "separator" section).
    pane_handle_hover => ColorSpec::Alias(Role::AccentDefault, None),

    sidebar_separator => ColorSpec::Alias(Role::BorderDefault, None),
    modal_divider      => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_separator_key_is_registered() {
        let keys =
            crate::tokens::component_keys("separator").expect("separator must be registered");
        assert_eq!(keys, SEPARATOR_KEYS);
        assert_eq!(keys.len(), 7, "one entry per SeparatorTokens field");
        assert!(keys.contains(&"line"));
        assert!(keys.contains(&"modal_divider"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let s = tokens.separator();
        assert_eq!(s.line, "#363a45");
        assert_eq!(s.handle_hover, "#787b86");
        assert_eq!(s.handle_active, "#2962ff");
        assert_eq!(s.pane_handle_idle, "#363a45");
        assert_eq!(s.pane_handle_hover, "#2962ff");
        assert_eq!(s.sidebar_separator, "#363a45");
        assert_eq!(s.modal_divider, "#363a45");
    }
}
