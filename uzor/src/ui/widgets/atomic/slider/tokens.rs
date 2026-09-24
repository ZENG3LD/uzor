//! Token-contract implementation of
//! [`SliderTheme`](super::theme::SliderTheme) (H1 token contract design §3;
//! companion table `docs/uzor/plans/h1-component-tokens-2026-09-24.md`,
//! "slider" section).
//!
//! [`SliderTokens`] holds one pre-rendered `String` per *owned*
//! `SliderTheme` method: the 10 companion-table fields plus 3 H1 Brief 7a
//! additions (`input_bg_disabled`/`input_placeholder`/`input_selection`) —
//! see [`super::theme::SliderTheme`]'s own doc comments on those 3 methods
//! for why they were added (`render.rs`'s numeric-input sub-theme held their
//! values as `[u8; 4]` literals, never reachable through `SliderTheme` at
//! all, before this brief). `handle_border` and `input_text` stay pure
//! delegators (`-> self.accent()` / `-> self.text_normal()`) — no token,
//! per the component_macro ambiguity/scope-note rule.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "slider",
    struct SliderTokens, keys = SLIDER_KEYS;

    track_empty          => ColorSpec::Alias(Role::BorderDefault, None),
    // Foreign Material-blue (companion doc ledger) — corrected onto the
    // shared accent role.
    accent                => ColorSpec::Alias(Role::AccentDefault, None),
    text_normal           => ColorSpec::Alias(Role::TextPrimary, None),
    // Value matches `text.muted` exactly, not `text.disabled` — name/role
    // mismatch noted in the companion table, zero value delta.
    text_disabled         => ColorSpec::Alias(Role::TextMuted, None),
    input_bg              => ColorSpec::Alias(Role::SurfaceControlHover, None),
    input_border_normal   => ColorSpec::Alias(Role::BorderDefault, None),
    input_border_focused  => ColorSpec::Alias(Role::FocusRing, None),
    toolbar_track_empty   => ColorSpec::Alias(Role::BorderDefault, None),
    toolbar_track_filled  => ColorSpec::Alias(Role::AccentDefault, None),
    toolbar_handle        => ColorSpec::Alias(Role::TextPrimary, None),

    // H1 Brief 7a additions — see `SliderTheme`'s own doc comments.
    input_bg_disabled  => ColorSpec::Alias(Role::SurfaceControlIdle, None),
    input_placeholder  => ColorSpec::Alias(Role::TextMuted, None),
    input_selection    => ColorSpec::Alias(Role::Selection, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_slider_key_is_registered() {
        let keys = crate::tokens::component_keys("slider").expect("slider must be registered");
        assert_eq!(keys, SLIDER_KEYS);
        assert_eq!(keys.len(), 13, "one entry per SliderTokens field");
        assert!(keys.contains(&"track_empty"));
        assert!(keys.contains(&"input_selection"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let s = tokens.slider();
        assert_eq!(s.track_empty, "#363a45");
        assert_eq!(s.accent, "#2962ff");
        assert_eq!(s.text_normal, "#d1d4dc");
        assert_eq!(s.text_disabled, "#787b86");
        assert_eq!(s.input_bg, "#2a2e39");
        assert_eq!(s.input_border_normal, "#363a45");
        assert_eq!(s.input_border_focused, "#2962ff");
        assert_eq!(s.toolbar_track_empty, "#363a45");
        assert_eq!(s.toolbar_track_filled, "#2962ff");
        assert_eq!(s.toolbar_handle, "#d1d4dc");
        assert_eq!(s.input_bg_disabled, "#1e222d");
        assert_eq!(s.input_placeholder, "#787b86");
        assert_eq!(s.input_selection, "#2962ff55");
    }
}
