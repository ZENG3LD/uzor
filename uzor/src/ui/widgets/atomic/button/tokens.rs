//! Token-contract implementation of [`ButtonTheme`](super::theme::ButtonTheme)
//! — the worked example every later widget's own `tokens.rs` copies (H1
//! token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "button" section).
//!
//! [`ButtonTokens`] holds one pre-rendered `String` per method this widget
//! OWNS a literal for: every non-default `ButtonTheme` method, plus the 3
//! default-body methods that return their OWN literal rather than
//! delegating to another method (`transparency_checker_a`/`_b`,
//! `dropdown_menu_row_bg_normal` — H1's "scope note on trait-default
//! methods"). Pure-delegator default methods (`clock_text`,
//! `color_swatch_*`, `dropdown_field_*`, `selector_*`, `close_button_x_color*`,
//! `scroll_chevron_color*`, …) need no field here and no override in `impl
//! ButtonTheme for TokenTheme` (`crate::tokens::theme`) — the trait's own
//! default body already resolves through the methods this file DOES
//! override.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Modifier, Rgba, Role};

component_tokens! {
    widget = "button",
    struct ButtonTokens, keys = BUTTON_KEYS;

    // Backgrounds
    button_bg_normal   => ColorSpec::Literal(ColorValue::Transparent),
    button_bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    button_bg_pressed  => ColorSpec::Alias(Role::AccentPressed, None),
    button_bg_active   => ColorSpec::Alias(Role::AccentPressed, None),
    button_bg_disabled => ColorSpec::Alias(Role::SurfaceControlHover, None),

    // Text
    button_text_normal   => ColorSpec::Alias(Role::TextPrimary, None),
    button_text_hover    => ColorSpec::Alias(Role::TextOnAccent, None),
    button_text_active   => ColorSpec::Alias(Role::TextOnAccent, None),
    button_text_disabled => ColorSpec::Alias(Role::TextDisabled, None),

    // Icons
    button_icon_normal   => ColorSpec::Alias(Role::TextMuted, None),
    button_icon_hover    => ColorSpec::Alias(Role::TextPrimary, None),
    button_icon_active   => ColorSpec::Alias(Role::TextOnAccent, None),
    button_icon_disabled => ColorSpec::Alias(Role::TextDisabled, None),

    // Borders
    button_border_normal => ColorSpec::Alias(Role::BorderDefault, None),
    // Corrects a bug-style default: the old literal `#e5e7eb` disagreed
    // with the trait's own doc comment ("Typical: #2a2a2a") and is really
    // `UIColors.button_hover_stroke`'s "transparent" slot in every set but
    // high_contrast_mono (companion table, button row `button_border_hover`).
    button_border_hover   => ColorSpec::Literal(ColorValue::Transparent),
    button_border_focused => ColorSpec::Alias(Role::FocusRing, None),

    // Semantic
    button_accent  => ColorSpec::Alias(Role::AccentDefault, None),
    button_danger  => ColorSpec::Alias(Role::StatusDanger, None),
    button_success => ColorSpec::Alias(Role::StatusSuccess, None),
    button_warning => ColorSpec::Alias(Role::StatusWarning, None),

    // Toolbar-specific slots
    toolbar_item_bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    toolbar_item_bg_active   => ColorSpec::Alias(Role::AccentDefault, None),
    toolbar_item_text        => ColorSpec::Alias(Role::TextPrimary, None),
    toolbar_item_text_hover  => ColorSpec::Alias(Role::TextOnAccent, None),
    toolbar_item_text_active => ColorSpec::Alias(Role::TextOnAccent, None),
    toolbar_separator        => ColorSpec::Alias(Role::BorderSubtle, None),
    toolbar_background       => ColorSpec::Alias(Role::SurfaceFloating, None),
    toolbar_accent           => ColorSpec::Alias(Role::AccentDefault, None),

    // Modal action button slots
    button_primary_bg           => ColorSpec::Alias(Role::AccentDefault, None),
    button_primary_bg_hover     => ColorSpec::Alias(Role::AccentHover, None),
    button_danger_bg            => ColorSpec::Alias(Role::StatusDangerBg, None),
    button_danger_bg_hover      => ColorSpec::Alias(Role::StatusDanger, Some(Modifier::Alpha(0.35))),
    button_danger_border        => ColorSpec::Alias(Role::StatusDanger, Some(Modifier::Alpha(0.5))),
    button_danger_border_hover  => ColorSpec::Alias(Role::StatusDanger, Some(Modifier::Alpha(0.75))),
    button_danger_text          => ColorSpec::Alias(Role::StatusDanger, None),
    button_secondary_hover_bg   => ColorSpec::Alias(Role::SurfaceControlHover, None),
    button_secondary_text_muted => ColorSpec::Alias(Role::TextMuted, None),
    button_secondary_text       => ColorSpec::Alias(Role::TextPrimary, None),
    button_ghost_idle_bg        => ColorSpec::Alias(Role::SurfaceFloating, None),
    button_utility_bg           => ColorSpec::Alias(Role::SurfaceControlHover, None),
    button_utility_bg_hover     => ColorSpec::Alias(Role::BorderDefault, None),

    // Non-semantic literals (H1 companion doc "scope note on non-semantic
    // literals") — checkerboard swatch tiles and the transparent dropdown
    // row background are not theme colours at all, but each is a literal
    // reachable from render code via its own trait method's default body,
    // so `TokenTheme` still owns and overrides all three explicitly.
    transparency_checker_a      => ColorSpec::Literal(ColorValue::Solid(Rgba::new(0xff, 0xff, 0xff, 0xff))),
    transparency_checker_b      => ColorSpec::Literal(ColorValue::Solid(Rgba::new(0xcc, 0xcc, 0xcc, 0xff))),
    dropdown_menu_row_bg_normal => ColorSpec::Literal(ColorValue::Transparent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{load_token_set, BuiltinSet, Tokens};

    #[test]
    fn every_button_key_is_registered() {
        let keys = crate::tokens::component_keys("button").expect("button must be registered");
        assert_eq!(keys, BUTTON_KEYS);
        assert_eq!(keys.len(), 44, "one entry per ButtonTokens field");
        assert!(keys.contains(&"button_bg_hover"));
        assert!(keys.contains(&"transparency_checker_a"));
        assert!(keys.contains(&"dropdown_menu_row_bg_normal"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let b = tokens.button();
        assert_eq!(b.button_bg_normal, "transparent");
        assert_eq!(b.button_bg_hover, "#2a2e39");
        assert_eq!(b.button_bg_pressed, "#1e53e4");
        assert_eq!(b.button_bg_active, "#1e53e4");
        assert_eq!(b.button_bg_disabled, "#2a2e39");
        assert_eq!(b.button_text_normal, "#d1d4dc");
        assert_eq!(b.button_text_hover, "#d1d4dc");
        assert_eq!(b.button_text_active, "#d1d4dc");
        assert_eq!(b.button_text_disabled, "#6a6d78");
        assert_eq!(b.button_icon_normal, "#787b86");
        assert_eq!(b.button_icon_hover, "#d1d4dc");
        assert_eq!(b.button_icon_active, "#d1d4dc");
        assert_eq!(b.button_icon_disabled, "#6a6d78");
        assert_eq!(b.button_border_normal, "#363a45");
        assert_eq!(b.button_border_hover, "transparent");
        assert_eq!(b.button_border_focused, "#2962ff");
        assert_eq!(b.button_accent, "#2962ff");
        assert_eq!(b.button_danger, "#f23645");
        assert_eq!(b.button_success, "#26a69a");
        assert_eq!(b.button_warning, "#ff9800");
        assert_eq!(b.toolbar_item_bg_hover, "#2a2e39");
        assert_eq!(b.toolbar_item_bg_active, "#2962ff");
        assert_eq!(b.toolbar_item_text, "#d1d4dc");
        assert_eq!(b.toolbar_item_text_hover, "#d1d4dc");
        assert_eq!(b.toolbar_item_text_active, "#d1d4dc");
        assert_eq!(b.toolbar_separator, "#2a2e39");
        assert_eq!(b.toolbar_background, "#1e222d");
        assert_eq!(b.toolbar_accent, "#2962ff");
        assert_eq!(b.button_primary_bg, "#2962ff");
        assert_eq!(b.button_primary_bg_hover, "#1e53e4");
        assert_eq!(b.button_danger_bg, "#f2364526");
        assert_eq!(b.button_danger_bg_hover, "#f2364559");
        assert_eq!(b.button_danger_border, "#f2364580");
        assert_eq!(b.button_danger_border_hover, "#f23645bf");
        assert_eq!(b.button_danger_text, "#f23645");
        assert_eq!(b.button_secondary_hover_bg, "#2a2e39");
        assert_eq!(b.button_secondary_text_muted, "#787b86");
        assert_eq!(b.button_secondary_text, "#d1d4dc");
        assert_eq!(b.button_ghost_idle_bg, "#1e222d");
        assert_eq!(b.button_utility_bg, "#2a2e39");
        assert_eq!(b.button_utility_bg_hover, "#363a45");
        assert_eq!(b.transparency_checker_a, "#ffffff");
        assert_eq!(b.transparency_checker_b, "#cccccc");
        assert_eq!(b.dropdown_menu_row_bg_normal, "transparent");
    }

    /// Minimal but complete fixture (every required role + geometry field,
    /// same shape as `tokens::loader`'s own test fixture) plus one
    /// `component.button.*` override — proves an override changes exactly
    /// the key it names and leaves every sibling key at its normal default.
    #[test]
    fn a_component_override_changes_exactly_that_key() {
        let json = r##"{
            "color": {
                "surface": {
                    "app_chrome": { "$type": "color", "$value": "#131722" },
                    "control": {
                        "idle": { "$type": "color", "$value": "#1e222d" },
                        "hover": { "$type": "color", "$value": "#2a2e39" },
                        "active": { "$type": "color", "$value": "#2962ff" }
                    },
                    "floating": { "$type": "color", "$value": "#1e222d" },
                    "header": { "$type": "color", "$value": "{color.surface.floating}" },
                    "panel": { "$type": "color", "$value": "{color.surface.floating}" }
                },
                "text": {
                    "primary": { "$type": "color", "$value": "#d1d4dc" },
                    "secondary": { "$type": "color", "$value": "#b2b5be" },
                    "muted": { "$type": "color", "$value": "#787b86" },
                    "disabled": { "$type": "color", "$value": "#6a6d78" },
                    "on_accent": { "$type": "color", "$value": "{color.text.primary}" }
                },
                "border": {
                    "subtle": { "$type": "color", "$value": "#2a2e39" },
                    "default": { "$type": "color", "$value": "#363a45" },
                    "strong": { "$type": "color", "$value": "{color.border.default}" }
                },
                "accent": {
                    "default": { "$type": "color", "$value": "#2962ff" },
                    "hover": { "$type": "color", "$value": "#1e53e4" },
                    "pressed": { "$type": "color", "$value": "{color.accent.hover}" }
                },
                "status": {
                    "success": { "$type": "color", "$value": "#26a69a" },
                    "success_bg": { "$type": "color", "$value": "{color.status.success}", "$extensions": { "uzor.alpha": 0.15 } },
                    "danger": { "$type": "color", "$value": "#f23645" },
                    "danger_bg": { "$type": "color", "$value": "{color.status.danger}", "$extensions": { "uzor.alpha": 0.15 } },
                    "warning": { "$type": "color", "$value": "#ff9800" },
                    "warning_bg": { "$type": "color", "$value": "{color.status.warning}", "$extensions": { "uzor.alpha": 0.15 } },
                    "info": { "$type": "color", "$value": "{color.accent.default}" },
                    "info_bg": { "$type": "color", "$value": "{color.status.info}", "$extensions": { "uzor.alpha": 0.15 } }
                },
                "selection": { "$type": "color", "$value": "#2962ff55" },
                "focus_ring": { "$type": "color", "$value": "{color.accent.default}" },
                "backdrop": {
                    "dim": { "$type": "color", "$value": "rgba(0,0,0,0.45)" },
                    "full": { "$type": "color", "$value": "{color.surface.app_chrome}" }
                },
                "shadow": {
                    "default": { "$type": "color", "$value": "rgba(0,0,0,0.4)" }
                }
            },
            "geometry": {
                "radius": {
                    "sm": { "$type": "dimension", "$value": 2.0 },
                    "md": { "$type": "dimension", "$value": 4.0 },
                    "lg": { "$type": "dimension", "$value": 8.0 }
                }
            },
            "component": {
                "button": {
                    "button_bg_hover": { "$type": "color", "$value": "{color.status.danger}" }
                }
            }
        }"##;
        let tokens = load_token_set(json).expect("load").resolve().expect("resolve");
        let b = tokens.button();
        assert_eq!(b.button_bg_hover, "#f23645", "the overridden key must reflect the override");
        assert_eq!(b.button_bg_pressed, "#1e53e4", "a sibling key keeps its normal default");
        assert_eq!(b.button_accent, "#2962ff", "an unrelated key keeps its normal default");
    }
}
