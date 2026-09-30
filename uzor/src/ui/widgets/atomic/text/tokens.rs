//! Token-contract implementation of
//! [`TextTheme`](super::theme::TextTheme) (H1 token contract design §3;
//! companion table `docs/uzor/plans/h1-component-tokens-2026-09-24.md`,
//! "text" section).
//!
//! [`TextTokens`] holds one pre-rendered `String` per `TextTheme` method —
//! all are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "text",
    struct TextTokens, keys = TEXT_KEYS;

    text_color       => ColorSpec::Alias(Role::TextPrimary, None),
    text_color_hover => ColorSpec::Alias(Role::TextOnAccent, None),
    // Selection highlight under selected glyphs (only drawn when the label
    // has a selection) — the same role a text input's selection uses.
    selection_color  => ColorSpec::Alias(Role::Selection, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_text_key_is_registered() {
        let keys = crate::tokens::component_keys("text").expect("text must be registered");
        assert_eq!(keys, TEXT_KEYS);
        assert_eq!(keys.len(), 3, "one entry per TextTokens field");
        assert!(keys.contains(&"text_color"));
        assert!(keys.contains(&"text_color_hover"));
        assert!(keys.contains(&"selection_color"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.text();
        assert_eq!(t.text_color, "#d1d4dc");
        assert_eq!(t.text_color_hover, "#d1d4dc");
        assert_eq!(t.selection_color, "#2962ff55");
    }
}
