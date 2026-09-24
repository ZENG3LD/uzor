//! Token-contract implementation of [`ItemTheme`](super::theme::ItemTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "item" section).
//!
//! [`ItemTokens`] holds one pre-rendered `String` for `item_text` —
//! `ItemTheme`'s only required method. `item_toolbar_text` is a
//! default-body delegator (`self.item_text()`, H1's "scope note on
//! trait-default methods") and needs no separate field or override.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "item",
    struct ItemTokens, keys = ITEM_KEYS;

    item_text => ColorSpec::Alias(Role::TextPrimary, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_item_key_is_registered() {
        let keys = crate::tokens::component_keys("item").expect("item must be registered");
        assert_eq!(keys, ITEM_KEYS);
        assert_eq!(keys.len(), 1, "one entry per ItemTokens field");
        assert!(keys.contains(&"item_text"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let i = tokens.item();
        assert_eq!(i.item_text, "#d1d4dc");
    }
}
