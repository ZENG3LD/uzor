//! Token-contract implementation of
//! [`ContextMenuTheme`](super::theme::ContextMenuTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "context_menu"
//! section). Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`ContextMenuTokens`] holds one pre-rendered `String` per
//! `ContextMenuTheme` method — every method on this trait is required (no
//! default bodies), so every field is owned here. `dropdown`'s own tokens
//! alias the same roles for the fields the two widgets share (H1 companion
//! doc: "that is the point") — this is a separate registered widget
//! (`component.context_menu.*`), not a shared struct, so each keeps its own
//! override surface.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "context_menu",
    struct ContextMenuTokens, keys = CONTEXT_MENU_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),

    item_bg_normal       => ColorSpec::Alias(Role::SurfaceFloating, None),
    item_bg_hover        => ColorSpec::Alias(Role::SurfaceControlHover, None),
    item_bg_danger_hover => ColorSpec::Alias(Role::StatusDangerBg, None),
    item_text            => ColorSpec::Alias(Role::TextPrimary, None),
    item_text_hover      => ColorSpec::Alias(Role::TextOnAccent, None),
    item_text_disabled   => ColorSpec::Alias(Role::TextDisabled, None),
    item_text_danger     => ColorSpec::Alias(Role::StatusDanger, None),

    separator => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_context_menu_key_is_registered() {
        let keys = crate::tokens::component_keys("context_menu")
            .expect("context_menu must be registered");
        assert_eq!(keys, CONTEXT_MENU_KEYS);
        assert_eq!(keys.len(), 11, "one entry per ContextMenuTokens field");
        assert!(keys.contains(&"item_bg_danger_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.context_menu();
        assert_eq!(c.bg, "#1e222d");
        assert_eq!(c.border, "#363a45");
        assert_eq!(c.shadow, "#00000066");
        assert_eq!(c.item_bg_normal, "#1e222d");
        assert_eq!(c.item_bg_hover, "#2a2e39");
        assert_eq!(c.item_bg_danger_hover, "#f2364526");
        assert_eq!(c.item_text, "#d1d4dc");
        assert_eq!(c.item_text_hover, "#d1d4dc");
        assert_eq!(c.item_text_disabled, "#6a6d78");
        assert_eq!(c.item_text_danger, "#f23645");
        assert_eq!(c.separator, "#363a45");
    }
}
