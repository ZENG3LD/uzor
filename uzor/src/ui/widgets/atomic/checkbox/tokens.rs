//! Token-contract implementation of [`CheckboxTheme`](super::theme::CheckboxTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "checkbox" section).
//!
//! [`CheckboxTokens`] holds one pre-rendered `String` per `CheckboxTheme`
//! method — all 6 are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "checkbox",
    struct CheckboxTokens, keys = CHECKBOX_KEYS;

    checkbox_bg_checked         => ColorSpec::Alias(Role::AccentDefault, None),
    checkbox_bg_unchecked       => ColorSpec::Alias(Role::SurfaceControlIdle, None),
    checkbox_border             => ColorSpec::Alias(Role::BorderSubtle, None),
    checkbox_checkmark          => ColorSpec::Alias(Role::TextOnAccent, None),
    checkbox_notification_inner => ColorSpec::Alias(Role::TextOnAccent, None),
    checkbox_label_text         => ColorSpec::Alias(Role::TextPrimary, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_checkbox_key_is_registered() {
        let keys = crate::tokens::component_keys("checkbox").expect("checkbox must be registered");
        assert_eq!(keys, CHECKBOX_KEYS);
        assert_eq!(keys.len(), 6, "one entry per CheckboxTokens field");
        assert!(keys.contains(&"checkbox_bg_checked"));
        assert!(keys.contains(&"checkbox_label_text"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.checkbox();
        assert_eq!(c.checkbox_bg_checked, "#2962ff");
        assert_eq!(c.checkbox_bg_unchecked, "#1e222d");
        assert_eq!(c.checkbox_border, "#2a2e39");
        assert_eq!(c.checkbox_checkmark, "#d1d4dc");
        assert_eq!(c.checkbox_notification_inner, "#d1d4dc");
        assert_eq!(c.checkbox_label_text, "#d1d4dc");
    }
}
