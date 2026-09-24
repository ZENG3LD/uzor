//! Token-contract implementation of [`DropdownTheme`](super::theme::DropdownTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "dropdown" section).
//! Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`DropdownTokens`] holds one pre-rendered `String` per `DropdownTheme`
//! method — every method on this trait is required (no default bodies), so
//! every field is owned here. `context_menu`'s own tokens alias the same
//! roles for the fields the two widgets share (H1 companion doc: "that is
//! the point") — this is a separate registered widget (`component.dropdown.*`),
//! not a shared struct, so each keeps its own override surface.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "dropdown",
    struct DropdownTokens, keys = DROPDOWN_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),

    item_bg_normal       => ColorSpec::Alias(Role::SurfaceFloating, None),
    item_bg_hover        => ColorSpec::Alias(Role::SurfaceControlHover, None),
    item_bg_selected     => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.15))),
    item_bg_danger_hover => ColorSpec::Alias(Role::StatusDangerBg, None),
    item_text            => ColorSpec::Alias(Role::TextPrimary, None),
    item_text_hover      => ColorSpec::Alias(Role::TextOnAccent, None),
    item_text_disabled   => ColorSpec::Alias(Role::TextDisabled, None),
    item_text_danger     => ColorSpec::Alias(Role::StatusDanger, None),

    header_text   => ColorSpec::Alias(Role::TextOnAccent, None),
    header_border => ColorSpec::Alias(Role::BorderDefault, None),

    separator => ColorSpec::Alias(Role::BorderDefault, None),

    shortcut_text => ColorSpec::Alias(Role::TextDisabled, None),
    caret_color   => ColorSpec::Alias(Role::TextDisabled, None),

    toggle_on     => ColorSpec::Alias(Role::AccentDefault, None),
    toggle_off    => ColorSpec::Alias(Role::TextDisabled, None),
    toggle_thumb  => ColorSpec::Alias(Role::TextOnAccent, None),

    trigger_bg       => ColorSpec::Alias(Role::SurfaceControlHover, None),
    // Value match, odd role-name reuse (companion doc) — not a copy-paste
    // bug: `trigger_bg_hover`'s literal already equalled `border.default`'s.
    trigger_bg_hover => ColorSpec::Alias(Role::BorderDefault, None),
    trigger_border   => ColorSpec::Alias(Role::BorderDefault, None),
    trigger_text     => ColorSpec::Alias(Role::TextPrimary, None),
    trigger_arrow    => ColorSpec::Alias(Role::TextDisabled, None),

    // Value match, odd role-name reuse (companion doc) — see `trigger_bg_hover`.
    checkbox_border  => ColorSpec::Alias(Role::TextDisabled, None),
    checkbox_checked => ColorSpec::Alias(Role::AccentDefault, None),

    cell_bg_hover => ColorSpec::Alias(Role::SurfaceControlHover, None),
    cell_border   => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_dropdown_key_is_registered() {
        let keys = crate::tokens::component_keys("dropdown").expect("dropdown must be registered");
        assert_eq!(keys, DROPDOWN_KEYS);
        assert_eq!(keys.len(), 28, "one entry per DropdownTokens field");
        assert!(keys.contains(&"item_bg_selected"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let d = tokens.dropdown();
        assert_eq!(d.bg, "#1e222d");
        assert_eq!(d.border, "#363a45");
        assert_eq!(d.shadow, "#00000066");
        assert_eq!(d.item_bg_normal, "#1e222d");
        assert_eq!(d.item_bg_hover, "#2a2e39");
        assert_eq!(d.item_bg_selected, "#2962ff26");
        assert_eq!(d.item_bg_danger_hover, "#f2364526");
        assert_eq!(d.item_text, "#d1d4dc");
        assert_eq!(d.item_text_hover, "#d1d4dc");
        assert_eq!(d.item_text_disabled, "#6a6d78");
        assert_eq!(d.item_text_danger, "#f23645");
        assert_eq!(d.header_text, "#d1d4dc");
        assert_eq!(d.header_border, "#363a45");
        assert_eq!(d.separator, "#363a45");
        assert_eq!(d.shortcut_text, "#6a6d78");
        assert_eq!(d.caret_color, "#6a6d78");
        assert_eq!(d.toggle_on, "#2962ff");
        assert_eq!(d.toggle_off, "#6a6d78");
        assert_eq!(d.toggle_thumb, "#d1d4dc");
        assert_eq!(d.trigger_bg, "#2a2e39");
        assert_eq!(d.trigger_bg_hover, "#363a45");
        assert_eq!(d.trigger_border, "#363a45");
        assert_eq!(d.trigger_text, "#d1d4dc");
        assert_eq!(d.trigger_arrow, "#6a6d78");
        assert_eq!(d.checkbox_border, "#6a6d78");
        assert_eq!(d.checkbox_checked, "#2962ff");
        assert_eq!(d.cell_bg_hover, "#2a2e39");
        assert_eq!(d.cell_border, "#363a45");
    }
}
