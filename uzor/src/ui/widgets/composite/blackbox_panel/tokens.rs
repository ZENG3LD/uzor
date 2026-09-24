//! Token-contract implementation of [`BlackboxTheme`](super::theme::BlackboxTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "blackbox_panel"
//! section). Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`BlackboxPanelTokens`] holds one pre-rendered `String` per `BlackboxTheme`
//! method — every method on this trait is required (no default bodies), so
//! every field is owned here.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "blackbox_panel",
    struct BlackboxPanelTokens, keys = BLACKBOX_PANEL_KEYS;

    bg          => ColorSpec::Alias(Role::SurfaceFloating, None),
    border      => ColorSpec::Alias(Role::BorderDefault, None),
    header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    header_text => ColorSpec::Alias(Role::TextOnAccent, None),
    divider     => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_blackbox_panel_key_is_registered() {
        let keys = crate::tokens::component_keys("blackbox_panel")
            .expect("blackbox_panel must be registered");
        assert_eq!(keys, BLACKBOX_PANEL_KEYS);
        assert_eq!(keys.len(), 5, "one entry per BlackboxPanelTokens field");
        assert!(keys.contains(&"header_text"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let b = tokens.blackbox_panel();
        assert_eq!(b.bg, "#1e222d");
        assert_eq!(b.border, "#363a45");
        assert_eq!(b.header_bg, "#1e222d");
        assert_eq!(b.header_text, "#d1d4dc");
        assert_eq!(b.divider, "#363a45");
    }
}
