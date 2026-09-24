//! Token-contract implementation of [`PopupTheme`](super::theme::PopupTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "popup" section).
//! Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`PopupTokens`] holds one pre-rendered `String` per `PopupTheme` method —
//! every method on this trait is required (no default bodies), so every
//! field is owned here.
//!
//! `hsv_indicator` stays `ColorSpec::Literal(ColorValue::Sentinel("rainbow"))`
//! per coordinator amendment A4 — it is a gradient marker, not a colour;
//! renderers may ignore it and draw a native rainbow gradient. It is a
//! carried crutch, marked for removal once the colour picker is promoted
//! (H1 token contract design, ledger row 11 / A4).
//!
//! `grid_hover_halo` has no companion-table row: it covers a literal
//! (`"#ffffff"`, the hover halo `input.rs::register_popup_grid` paints
//! behind a hovered colour-picker cell) the table's per-widget scan missed
//! because it lives outside `PopupTheme` entirely — H1 Brief 8b adds the
//! trait method so the literal is reachable through the token contract.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Modifier, Role};

component_tokens! {
    widget = "popup",
    struct PopupTokens, keys = POPUP_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),

    item_bg_normal      => ColorSpec::Alias(Role::SurfaceFloating, None),
    item_bg_hover        => ColorSpec::Alias(Role::SurfaceControlHover, None),
    item_bg_selected     => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.15))),
    item_text            => ColorSpec::Alias(Role::TextPrimary, None),
    item_text_hover      => ColorSpec::Alias(Role::TextOnAccent, None),
    item_text_disabled   => ColorSpec::Alias(Role::TextDisabled, None),
    item_text_danger     => ColorSpec::Alias(Role::StatusDanger, None),
    item_bg_danger_hover => ColorSpec::Alias(Role::StatusDangerBg, None),
    header_text          => ColorSpec::Alias(Role::TextOnAccent, None),
    separator            => ColorSpec::Alias(Role::BorderDefault, None),

    hex_input_bg           => ColorSpec::Alias(Role::SurfaceControlHover, None),
    hex_input_text         => ColorSpec::Alias(Role::TextPrimary, None),
    hex_input_border_focus => ColorSpec::Alias(Role::FocusRing, None),

    // Gradient marker, not a colour — see module doc / A4.
    hsv_indicator => ColorSpec::Literal(ColorValue::Sentinel("rainbow")),
    accent        => ColorSpec::Alias(Role::AccentDefault, None),

    backdrop_dim => ColorSpec::Alias(Role::BackdropDim, None),

    // Colour-picker grid hover halo — see module doc.
    grid_hover_halo => ColorSpec::Alias(Role::TextOnAccent, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_popup_key_is_registered() {
        let keys = crate::tokens::component_keys("popup").expect("popup must be registered");
        assert_eq!(keys, POPUP_KEYS);
        assert_eq!(keys.len(), 20, "one entry per PopupTokens field");
        assert!(keys.contains(&"grid_hover_halo"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let p = tokens.popup();
        assert_eq!(p.bg, "#1e222d");
        assert_eq!(p.border, "#363a45");
        assert_eq!(p.shadow, "#00000066");
        assert_eq!(p.item_bg_normal, "#1e222d");
        assert_eq!(p.item_bg_hover, "#2a2e39");
        assert_eq!(p.item_bg_selected, "#2962ff26");
        assert_eq!(p.item_text, "#d1d4dc");
        assert_eq!(p.item_text_hover, "#d1d4dc");
        assert_eq!(p.item_text_disabled, "#6a6d78");
        assert_eq!(p.item_text_danger, "#f23645");
        assert_eq!(p.item_bg_danger_hover, "#f2364526");
        assert_eq!(p.header_text, "#d1d4dc");
        assert_eq!(p.separator, "#363a45");
        assert_eq!(p.hex_input_bg, "#2a2e39");
        assert_eq!(p.hex_input_text, "#d1d4dc");
        assert_eq!(p.hex_input_border_focus, "#2962ff");
        assert_eq!(p.hsv_indicator, "rainbow");
        assert_eq!(p.accent, "#2962ff");
        assert_eq!(p.backdrop_dim, "#00000072");
        assert_eq!(p.grid_hover_halo, "#d1d4dc");
    }
}
