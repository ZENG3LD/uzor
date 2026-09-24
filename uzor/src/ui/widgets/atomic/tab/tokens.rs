//! Token-contract implementation of [`TabTheme`](super::theme::TabTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "tab" section).
//!
//! [`TabTokens`] holds one pre-rendered `String` per *owned* `TabTheme`
//! method: the 8 required methods (`bg_normal`/`bg_hover`/`bg_active`/
//! `text_normal`/`text_active`/`accent`/`close_normal`/`close_hover`) plus
//! `tags_pill_bg_hover` — a default-body method whose desired alias
//! (`text.secondary`) diverges from what its delegation target
//! (`self.text_normal()`, aliased to `text.muted`) would resolve to, so it
//! cannot stay a pure delegator on `TokenTheme` and needs its own token
//! (component_macro's ambiguity/scope-note rule: only a delegator whose
//! target already resolves to the SAME value can skip a token).
//!
//! The other 5 default-body methods (`chrome_bottom_accent`,
//! `chrome_hover_line`, `sidebar_left_accent`, `sidebar_bg_active`,
//! `tags_pill_bg_active`) DO resolve to the same value as their delegation
//! target once every required field above is aliased per the companion
//! table, so `TokenTheme` does not override them — they fall through to
//! `TabTheme`'s own default body, same as any other implementor:
//! - `chrome_bottom_accent -> self.accent()` (both `accent.default`)
//! - `chrome_hover_line -> self.bg_hover()` (both `surface.control.hover`)
//! - `sidebar_left_accent -> self.accent()` (both `accent.default`)
//! - `sidebar_bg_active -> self.bg_active()` (both `accent.pressed`)
//! - `tags_pill_bg_active -> self.accent()` (both `accent.default`)
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "tab",
    struct TabTokens, keys = TAB_KEYS;

    bg_normal   => ColorSpec::Alias(Role::SurfaceControlIdle, None),
    bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    bg_active   => ColorSpec::Alias(Role::AccentPressed, None),
    text_normal => ColorSpec::Alias(Role::TextMuted, None),
    text_active => ColorSpec::Alias(Role::TextOnAccent, None),
    accent      => ColorSpec::Alias(Role::AccentDefault, None),
    close_normal => ColorSpec::Alias(Role::TextMuted, None),
    close_hover  => ColorSpec::Alias(Role::TextOnAccent, None),

    // See module doc — this default-body method needs its own token because
    // its target's alias (`text.muted`) differs from its own alias
    // (`text.secondary`).
    tags_pill_bg_hover => ColorSpec::Alias(Role::TextSecondary, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_tab_key_is_registered() {
        let keys = crate::tokens::component_keys("tab").expect("tab must be registered");
        assert_eq!(keys, TAB_KEYS);
        assert_eq!(keys.len(), 9, "one entry per TabTokens field");
        assert!(keys.contains(&"bg_normal"));
        assert!(keys.contains(&"tags_pill_bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.tab();
        assert_eq!(t.bg_normal, "#1e222d");
        assert_eq!(t.bg_hover, "#2a2e39");
        assert_eq!(t.bg_active, "#1e53e4");
        assert_eq!(t.text_normal, "#787b86");
        assert_eq!(t.text_active, "#d1d4dc");
        assert_eq!(t.accent, "#2962ff");
        assert_eq!(t.close_normal, "#787b86");
        assert_eq!(t.close_hover, "#d1d4dc");
        assert_eq!(t.tags_pill_bg_hover, "#b2b5be");
    }
}
