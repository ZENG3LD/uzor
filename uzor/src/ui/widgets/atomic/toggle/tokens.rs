//! Token-contract implementation of [`ToggleTheme`](super::theme::ToggleTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "toggle" section).
//!
//! [`ToggleTokens`] holds one pre-rendered `String` per `ToggleTheme` method
//! — all 9 are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "toggle",
    struct ToggleTokens, keys = TOGGLE_KEYS;

    toggle_track_off => ColorSpec::Alias(Role::SurfaceControlHover, None),
    toggle_track_on  => ColorSpec::Alias(Role::AccentDefault, None),
    toggle_thumb_off => ColorSpec::Alias(Role::TextOnAccent, None),
    toggle_thumb_on  => ColorSpec::Alias(Role::TextOnAccent, None),

    // Same construction as `radio::radio_disabled_overlay` /
    // `color_swatch::fill_toggle_disabled_overlay`: `backdrop.dim`'s black
    // at a lower, disabled-overlay alpha. Format-normalizes the old
    // `"rgba(0,0,0,0.35)"` literal into the canonical hex form
    // (pixel-identical, per the companion doc's blank-Δ row).
    toggle_disabled_overlay => ColorSpec::Alias(Role::BackdropDim, Some(Modifier::Alpha(0.35))),

    toggle_label_text          => ColorSpec::Alias(Role::TextPrimary, None),
    toggle_label_text_disabled => ColorSpec::Alias(Role::TextDisabled, None),
    toggle_icon_normal         => ColorSpec::Alias(Role::TextMuted, None),
    toggle_icon_active         => ColorSpec::Alias(Role::TextOnAccent, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_toggle_key_is_registered() {
        let keys = crate::tokens::component_keys("toggle").expect("toggle must be registered");
        assert_eq!(keys, TOGGLE_KEYS);
        assert_eq!(keys.len(), 9, "one entry per ToggleTokens field");
        assert!(keys.contains(&"toggle_disabled_overlay"));
        assert!(keys.contains(&"toggle_icon_active"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.toggle();
        assert_eq!(t.toggle_track_off, "#2a2e39");
        assert_eq!(t.toggle_track_on, "#2962ff");
        assert_eq!(t.toggle_thumb_off, "#d1d4dc");
        assert_eq!(t.toggle_thumb_on, "#d1d4dc");
        // rgb(0,0,0) at alpha 0.35 (0.35 * 255 = 89.25, rounds to 89 = 0x59) —
        // pixel-identical to the old inline literal "rgba(0,0,0,0.35)".
        assert_eq!(t.toggle_disabled_overlay, "#00000059");
        assert_eq!(t.toggle_label_text, "#d1d4dc");
        assert_eq!(t.toggle_label_text_disabled, "#6a6d78");
        assert_eq!(t.toggle_icon_normal, "#787b86");
        assert_eq!(t.toggle_icon_active, "#d1d4dc");
    }
}
