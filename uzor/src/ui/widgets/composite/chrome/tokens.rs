//! Token-contract implementation of [`ChromeTheme`](super::theme::ChromeTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "chrome" section).
//! Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`ChromeTokens`] holds one pre-rendered `String` per `ChromeTheme` method —
//! every method on this trait is required (no default bodies), so every
//! field is owned here.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Role};

component_tokens! {
    widget = "chrome",
    struct ChromeTokens, keys = CHROME_KEYS;

    background   => ColorSpec::Alias(Role::SurfaceAppChrome, None),

    icon_normal  => ColorSpec::Alias(Role::TextSecondary, None),
    icon_hover   => ColorSpec::Alias(Role::TextPrimary, None),

    button_hover => ColorSpec::Alias(Role::SurfaceControlHover, None),
    // Windows close-app hover convention — not a themeable role.
    close_hover  => ColorSpec::Literal(ColorValue::Solid(crate::tokens::Rgba::new(0xe8, 0x11, 0x23, 0xff))),

    separator    => ColorSpec::Alias(Role::BorderDefault, None),

    tab_bg_normal   => ColorSpec::Literal(ColorValue::Transparent),
    tab_bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    tab_bg_active   => ColorSpec::Alias(Role::SurfaceFloating, None),
    tab_text_normal => ColorSpec::Alias(Role::TextSecondary, None),
    tab_text_hover  => ColorSpec::Alias(Role::TextPrimary, None),
    tab_text_active => ColorSpec::Alias(Role::TextOnAccent, None),
    tab_accent      => ColorSpec::Alias(Role::AccentDefault, None),

    drag_zone_bg => ColorSpec::Literal(ColorValue::Transparent),

    tooltip_bg   => ColorSpec::Alias(Role::SurfaceControlHover, None),
    tooltip_text => ColorSpec::Alias(Role::TextOnAccent, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_chrome_key_is_registered() {
        let keys = crate::tokens::component_keys("chrome").expect("chrome must be registered");
        assert_eq!(keys, CHROME_KEYS);
        assert_eq!(keys.len(), 16, "one entry per ChromeTokens field");
        assert!(keys.contains(&"tab_accent"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.chrome();
        assert_eq!(c.background, "#131722");
        assert_eq!(c.icon_normal, "#b2b5be");
        assert_eq!(c.icon_hover, "#d1d4dc");
        assert_eq!(c.button_hover, "#2a2e39");
        assert_eq!(c.close_hover, "#e81123");
        assert_eq!(c.separator, "#363a45");
        assert_eq!(c.tab_bg_normal, "transparent");
        assert_eq!(c.tab_bg_hover, "#2a2e39");
        assert_eq!(c.tab_bg_active, "#1e222d");
        assert_eq!(c.tab_text_normal, "#b2b5be");
        assert_eq!(c.tab_text_hover, "#d1d4dc");
        assert_eq!(c.tab_text_active, "#d1d4dc");
        assert_eq!(c.tab_accent, "#2962ff");
        assert_eq!(c.drag_zone_bg, "transparent");
        assert_eq!(c.tooltip_bg, "#2a2e39");
        assert_eq!(c.tooltip_text, "#d1d4dc");
    }
}
