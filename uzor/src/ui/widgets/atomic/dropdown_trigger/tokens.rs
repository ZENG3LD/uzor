//! Token-contract implementation of
//! [`DropdownTriggerTheme`](super::theme::DropdownTriggerTheme) (H1 token
//! contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "dropdown_trigger"
//! section).
//!
//! [`DropdownTriggerTokens`] holds one pre-rendered `String` per
//! `DropdownTriggerTheme` method — all 5 are required (no default-body
//! delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "dropdown_trigger",
    struct DropdownTriggerTokens, keys = DROPDOWN_TRIGGER_KEYS;

    dropdown_field_bg       => ColorSpec::Alias(Role::SurfaceFloating, None),
    dropdown_field_bg_hover => ColorSpec::Alias(Role::SurfaceControlHover, None),
    dropdown_field_border   => ColorSpec::Alias(Role::BorderSubtle, None),
    dropdown_field_text     => ColorSpec::Alias(Role::TextPrimary, None),
    dropdown_chevron_color  => ColorSpec::Alias(Role::TextPrimary, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_dropdown_trigger_key_is_registered() {
        let keys = crate::tokens::component_keys("dropdown_trigger")
            .expect("dropdown_trigger must be registered");
        assert_eq!(keys, DROPDOWN_TRIGGER_KEYS);
        assert_eq!(keys.len(), 5, "one entry per DropdownTriggerTokens field");
        assert!(keys.contains(&"dropdown_field_bg"));
        assert!(keys.contains(&"dropdown_chevron_color"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let d = tokens.dropdown_trigger();
        assert_eq!(d.dropdown_field_bg, "#1e222d");
        assert_eq!(d.dropdown_field_bg_hover, "#2a2e39");
        assert_eq!(d.dropdown_field_border, "#2a2e39");
        assert_eq!(d.dropdown_field_text, "#d1d4dc");
        assert_eq!(d.dropdown_chevron_color, "#d1d4dc");
    }
}
