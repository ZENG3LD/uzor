//! Token-contract implementation of
//! [`ColorSwatchTheme`](super::theme::ColorSwatchTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "color_swatch"
//! section).
//!
//! [`ColorSwatchTokens`] holds one pre-rendered `String` per
//! `ColorSwatchTheme` method — all are required (no default-body delegators
//! on this trait), including [`super::theme::ColorSwatchTheme::fill_toggle_disabled_overlay`]
//! (H1 Brief 5 addition — previously an inline literal in `render.rs`, not
//! reachable through the theme trait at all; the companion table's
//! "non-semantic literals" scope note does not cover it because it IS a
//! colour role, the same disabled-overlay role `radio`/`toggle` alias onto
//! `backdrop.dim`'s black at a lower alpha).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Modifier, Rgba, Role};

component_tokens! {
    widget = "color_swatch",
    struct ColorSwatchTokens, keys = COLOR_SWATCH_KEYS;

    color_swatch_border          => ColorSpec::Alias(Role::BorderSubtle, None),
    color_swatch_hover_outline   => ColorSpec::Alias(Role::SurfaceControlHover, None),
    color_swatch_selected_border => ColorSpec::Alias(Role::AccentDefault, None),

    // Non-semantic literals (H1 companion doc "scope note on non-semantic
    // literals") — checkerboard swatch tiles are not theme colours at all,
    // but each is a literal reachable from render code via its own trait
    // method, so `TokenTheme` still owns and overrides both explicitly.
    transparency_checker_a => ColorSpec::Literal(ColorValue::Solid(Rgba::new(0xff, 0xff, 0xff, 0xff))),
    transparency_checker_b => ColorSpec::Literal(ColorValue::Solid(Rgba::new(0xcc, 0xcc, 0xcc, 0xff))),

    fill_toggle_active_border     => ColorSpec::Alias(Role::AccentDefault, None),
    fill_toggle_off_pattern_color => ColorSpec::Alias(Role::BorderSubtle, None),
    fill_toggle_background        => ColorSpec::Alias(Role::SurfaceFloating, None),

    // H1 Brief 5 addition — see module doc. Same construction as
    // `radio_disabled_overlay`/`toggle_disabled_overlay` in the companion
    // table: `backdrop.dim`'s black at a lower, disabled-overlay alpha.
    fill_toggle_disabled_overlay => ColorSpec::Alias(Role::BackdropDim, Some(Modifier::Alpha(0.35))),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_color_swatch_key_is_registered() {
        let keys =
            crate::tokens::component_keys("color_swatch").expect("color_swatch must be registered");
        assert_eq!(keys, COLOR_SWATCH_KEYS);
        assert_eq!(keys.len(), 9, "one entry per ColorSwatchTokens field");
        assert!(keys.contains(&"color_swatch_border"));
        assert!(keys.contains(&"fill_toggle_disabled_overlay"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.color_swatch();
        assert_eq!(c.color_swatch_border, "#2a2e39");
        assert_eq!(c.color_swatch_hover_outline, "#2a2e39");
        assert_eq!(c.color_swatch_selected_border, "#2962ff");
        assert_eq!(c.transparency_checker_a, "#ffffff");
        assert_eq!(c.transparency_checker_b, "#cccccc");
        assert_eq!(c.fill_toggle_active_border, "#2962ff");
        assert_eq!(c.fill_toggle_off_pattern_color, "#2a2e39");
        assert_eq!(c.fill_toggle_background, "#1e222d");
        // rgb(0,0,0) at alpha 0.35 (0.35 * 255 = 89.25, rounds to 89 = 0x59) —
        // pixel-identical to the old inline literal "rgba(0,0,0,0.35)".
        assert_eq!(c.fill_toggle_disabled_overlay, "#00000059");
    }
}
