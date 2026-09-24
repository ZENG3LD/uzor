//! Token-contract implementation of [`RadioTheme`](super::theme::RadioTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "radio" section).
//!
//! [`RadioTokens`] holds one pre-rendered `String` per `RadioTheme` method —
//! all 8 are required (no default-body delegators on this trait).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "radio",
    struct RadioTokens, keys = RADIO_KEYS;

    radio_outer_border          => ColorSpec::Alias(Role::BorderSubtle, None),
    radio_outer_border_selected => ColorSpec::Alias(Role::AccentDefault, None),
    radio_inner_dot             => ColorSpec::Alias(Role::AccentDefault, None),

    // Same construction as `color_swatch::fill_toggle_disabled_overlay` /
    // `toggle_disabled_overlay`: `backdrop.dim`'s black at a lower,
    // disabled-overlay alpha. Format-normalizes the old
    // `"rgba(0,0,0,0.35)"` literal into the canonical hex form
    // (pixel-identical, per the companion doc's blank-Δ row).
    radio_disabled_overlay => ColorSpec::Alias(Role::BackdropDim, Some(Modifier::Alpha(0.35))),

    radio_row_bg_hover          => ColorSpec::Alias(Role::SurfaceControlHover, None),
    radio_label_text            => ColorSpec::Alias(Role::TextPrimary, None),
    radio_label_text_selected   => ColorSpec::Alias(Role::TextOnAccent, None),
    radio_description_text      => ColorSpec::Alias(Role::TextDisabled, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_radio_key_is_registered() {
        let keys = crate::tokens::component_keys("radio").expect("radio must be registered");
        assert_eq!(keys, RADIO_KEYS);
        assert_eq!(keys.len(), 8, "one entry per RadioTokens field");
        assert!(keys.contains(&"radio_outer_border"));
        assert!(keys.contains(&"radio_description_text"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let r = tokens.radio();
        assert_eq!(r.radio_outer_border, "#2a2e39");
        assert_eq!(r.radio_outer_border_selected, "#2962ff");
        assert_eq!(r.radio_inner_dot, "#2962ff");
        // rgb(0,0,0) at alpha 0.35 (0.35 * 255 = 89.25, rounds to 89 = 0x59) —
        // pixel-identical to the old inline literal "rgba(0,0,0,0.35)".
        assert_eq!(r.radio_disabled_overlay, "#00000059");
        assert_eq!(r.radio_row_bg_hover, "#2a2e39");
        assert_eq!(r.radio_label_text, "#d1d4dc");
        assert_eq!(r.radio_label_text_selected, "#d1d4dc");
        assert_eq!(r.radio_description_text, "#6a6d78");
    }
}
