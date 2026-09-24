//! Token-contract implementation of
//! [`ScrollbarTheme`](super::theme::ScrollbarTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "scrollbar" section
//! — the `DefaultScrollbarTheme` dark palette only; `LightScrollbarTheme`
//! was deleted, see module doc on [`super::theme`]).
//!
//! [`ScrollbarTokens`] holds one pre-rendered `String` per `ScrollbarTheme`
//! method — all 4 are required (no default-body delegators on this trait).
//!
//! `thumb_hover` and `thumb_active` both default to
//! `ColorSpec::Alias(Role::TextPrimary, None)` — the same role — so they
//! resolve to the identical string in every built-in set today
//! (`dark_defaults_match_the_companion_table` below asserts this for
//! `dark`). `render.rs`'s `draw_scrollbar` reads both distinctly
//! (`HandleHovered` → `thumb_hover`, `Dragging` → `thumb_active`, H1 Brief
//! 10a-3 item 0b), so the two states are ready to diverge the moment either
//! field gets its own alias — this is a token-table fact, not a bug in
//! either the render code or this table.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "scrollbar",
    struct ScrollbarTokens, keys = SCROLLBAR_KEYS;

    thumb_normal => ColorSpec::Alias(Role::TextDisabled, None),
    thumb_hover  => ColorSpec::Alias(Role::TextPrimary, None),
    thumb_active => ColorSpec::Alias(Role::TextPrimary, None),
    track_bg     => ColorSpec::Alias(Role::BorderDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_scrollbar_key_is_registered() {
        let keys =
            crate::tokens::component_keys("scrollbar").expect("scrollbar must be registered");
        assert_eq!(keys, SCROLLBAR_KEYS);
        assert_eq!(keys.len(), 4, "one entry per ScrollbarTokens field");
        assert!(keys.contains(&"thumb_normal"));
        assert!(keys.contains(&"track_bg"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let s = tokens.scrollbar();
        assert_eq!(s.thumb_normal, "#6a6d78");
        assert_eq!(s.thumb_hover, "#d1d4dc");
        assert_eq!(s.thumb_active, "#d1d4dc");
        assert_eq!(s.track_bg, "#363a45");
    }
}
