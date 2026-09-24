//! Token-contract implementation of [`ModalTheme`](super::theme::ModalTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "modal" section).
//! Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`ModalTokens`] holds one pre-rendered `String` per `ModalTheme` method —
//! every method on this trait is required (no default bodies), so every
//! field is owned here.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "modal",
    struct ModalTokens, keys = MODAL_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),

    header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    header_text => ColorSpec::Alias(Role::TextOnAccent, None),
    divider     => ColorSpec::Alias(Role::BorderDefault, None),

    footer_bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    footer_border => ColorSpec::Alias(Role::BorderDefault, None),

    close_icon       => ColorSpec::Alias(Role::TextSecondary, None),
    close_icon_hover => ColorSpec::Alias(Role::TextOnAccent, None),

    // `backdrop.dim`/`backdrop.full` — the composite specifics' own roles
    // (companion doc: "modal's `backdrop_dim`/`backdrop_full` -> `backdrop.*`
    // roles").
    backdrop_dim  => ColorSpec::Alias(Role::BackdropDim, None),
    backdrop_full => ColorSpec::Alias(Role::BackdropFull, None),

    sidebar_bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    sidebar_border => ColorSpec::Alias(Role::BorderDefault, None),

    tab_text_active   => ColorSpec::Alias(Role::TextOnAccent, None),
    tab_text_inactive => ColorSpec::Alias(Role::TextMuted, None),
    tab_accent        => ColorSpec::Alias(Role::AccentDefault, None),
    tab_bg_active     => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.12))),
    tab_bg_hover      => ColorSpec::Alias(Role::SurfaceControlHover, None),

    wizard_dot_inactive => ColorSpec::Alias(Role::BorderDefault, None),
    wizard_dot_active   => ColorSpec::Alias(Role::AccentDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_modal_key_is_registered() {
        let keys = crate::tokens::component_keys("modal").expect("modal must be registered");
        assert_eq!(keys, MODAL_KEYS);
        assert_eq!(keys.len(), 21, "one entry per ModalTokens field");
        assert!(keys.contains(&"backdrop_dim"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let m = tokens.modal();
        assert_eq!(m.bg, "#1e222d");
        assert_eq!(m.border, "#363a45");
        assert_eq!(m.shadow, "#00000066");
        assert_eq!(m.header_bg, "#1e222d");
        assert_eq!(m.header_text, "#d1d4dc");
        assert_eq!(m.divider, "#363a45");
        assert_eq!(m.footer_bg, "#1e222d");
        assert_eq!(m.footer_border, "#363a45");
        assert_eq!(m.close_icon, "#b2b5be");
        assert_eq!(m.close_icon_hover, "#d1d4dc");
        assert_eq!(m.backdrop_dim, "#00000072");
        assert_eq!(m.backdrop_full, "#131722");
        assert_eq!(m.sidebar_bg, "#1e222d");
        assert_eq!(m.sidebar_border, "#363a45");
        assert_eq!(m.tab_text_active, "#d1d4dc");
        assert_eq!(m.tab_text_inactive, "#787b86");
        assert_eq!(m.tab_accent, "#2962ff");
        assert_eq!(m.tab_bg_active, "#2962ff1f");
        assert_eq!(m.tab_bg_hover, "#2a2e39");
        assert_eq!(m.wizard_dot_inactive, "#363a45");
        assert_eq!(m.wizard_dot_active, "#2962ff");
    }
}
