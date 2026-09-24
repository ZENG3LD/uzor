//! Token-contract implementation of
//! [`ToolbarTheme`](super::theme::ToolbarTheme) (H1 token contract design
//! §3; companion table `docs/uzor/plans/h1-component-tokens-2026-09-24.md`,
//! "toolbar" section). Same shape as `atomic/button/tokens.rs`, the worked
//! example.
//!
//! [`ToolbarTokens`] holds one pre-rendered `String` per `ToolbarTheme`
//! method — every method on this trait is required (no default bodies), so
//! every field is owned here. Unlike `sidebar`/`popup`, `render.rs`/
//! `input.rs`/`types.rs` already read every colour through `ToolbarTheme`
//! trait methods — the `[u8; 4]` in `types.rs::ToolbarItem::ColorButton` /
//! `render.rs::rgba_to_hex` is caller-supplied per-frame swatch data (a
//! colour-picker button's *current* colour), not a theme literal, so this
//! is a straight 28-field conversion with no bypass adapters to merge.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Modifier, Role};

component_tokens! {
    widget = "toolbar",
    struct ToolbarTokens, keys = TOOLBAR_KEYS;

    bg        => ColorSpec::Alias(Role::SurfaceFloating, None),
    separator => ColorSpec::Alias(Role::BorderDefault, None),

    item_bg_normal  => ColorSpec::Literal(ColorValue::Transparent),
    item_bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    item_bg_active   => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.15))),
    item_bg_pressed  => ColorSpec::Alias(Role::AccentPressed, Some(Modifier::Alpha(0.25))),

    item_text_normal   => ColorSpec::Alias(Role::TextPrimary, None),
    item_text_hover     => ColorSpec::Alias(Role::TextOnAccent, None),
    item_text_active    => ColorSpec::Alias(Role::AccentDefault, None),
    item_text_disabled  => ColorSpec::Alias(Role::TextDisabled, None),

    icon_normal   => ColorSpec::Alias(Role::TextPrimary, None),
    icon_hover     => ColorSpec::Alias(Role::TextOnAccent, None),
    icon_active    => ColorSpec::Alias(Role::AccentDefault, None),
    icon_disabled  => ColorSpec::Alias(Role::TextDisabled, None),

    scroll_chevron_color => ColorSpec::Alias(Role::TextDisabled, None),

    label_text => ColorSpec::Alias(Role::TextDisabled, None),
    clock_text => ColorSpec::Alias(Role::TextPrimary, None),

    chrome_tab_bg_active     => ColorSpec::Alias(Role::SurfaceFloating, None),
    chrome_tab_bg_inactive   => ColorSpec::Literal(ColorValue::Transparent),
    chrome_tab_bg_hover      => ColorSpec::Alias(Role::SurfaceControlHover, None),
    chrome_tab_text_active   => ColorSpec::Alias(Role::TextOnAccent, None),
    chrome_tab_text_inactive => ColorSpec::Alias(Role::TextDisabled, None),
    chrome_ctrl_hover        => ColorSpec::Alias(Role::SurfaceControlHover, None),
    // Windows native close-red convention — kept literal (H1 token contract
    // design §1's "non-semantic literals" scope note; same convention as
    // the `chrome` composite's own `close_hover`).
    chrome_close_hover => ColorSpec::Literal(ColorValue::Solid(crate::tokens::Rgba::new(0xe8, 0x11, 0x23, 0xff))),
    chrome_ctrl_icon    => ColorSpec::Alias(Role::TextPrimary, None),

    color_swatch_border => ColorSpec::Alias(Role::BorderDefault, None),

    split_chevron => ColorSpec::Alias(Role::TextDisabled, None),
    split_divider => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_toolbar_key_is_registered() {
        let keys = crate::tokens::component_keys("toolbar").expect("toolbar must be registered");
        assert_eq!(keys, TOOLBAR_KEYS);
        assert_eq!(keys.len(), 28, "one entry per ToolbarTokens field");
        assert!(keys.contains(&"chrome_close_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.toolbar();
        assert_eq!(t.bg, "#1e222d");
        assert_eq!(t.separator, "#363a45");
        assert_eq!(t.item_bg_normal, "transparent");
        assert_eq!(t.item_bg_hover, "#2a2e39");
        assert_eq!(t.item_bg_active, "#2962ff26");
        assert_eq!(t.item_bg_pressed, "#1e53e440");
        assert_eq!(t.item_text_normal, "#d1d4dc");
        assert_eq!(t.item_text_hover, "#d1d4dc");
        assert_eq!(t.item_text_active, "#2962ff");
        assert_eq!(t.item_text_disabled, "#6a6d78");
        assert_eq!(t.icon_normal, "#d1d4dc");
        assert_eq!(t.icon_hover, "#d1d4dc");
        assert_eq!(t.icon_active, "#2962ff");
        assert_eq!(t.icon_disabled, "#6a6d78");
        assert_eq!(t.scroll_chevron_color, "#6a6d78");
        assert_eq!(t.label_text, "#6a6d78");
        assert_eq!(t.clock_text, "#d1d4dc");
        assert_eq!(t.chrome_tab_bg_active, "#1e222d");
        assert_eq!(t.chrome_tab_bg_inactive, "transparent");
        assert_eq!(t.chrome_tab_bg_hover, "#2a2e39");
        assert_eq!(t.chrome_tab_text_active, "#d1d4dc");
        assert_eq!(t.chrome_tab_text_inactive, "#6a6d78");
        assert_eq!(t.chrome_ctrl_hover, "#2a2e39");
        assert_eq!(t.chrome_close_hover, "#e81123");
        assert_eq!(t.chrome_ctrl_icon, "#d1d4dc");
        assert_eq!(t.color_swatch_border, "#363a45");
        assert_eq!(t.split_chevron, "#6a6d78");
        assert_eq!(t.split_divider, "#363a45");
    }
}
