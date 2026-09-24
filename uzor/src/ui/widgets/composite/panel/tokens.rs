//! Token-contract implementation of [`PanelTheme`](super::theme::PanelTheme)
//! (H1 token contract design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "panel" section).
//! Same shape as `atomic/button/tokens.rs`, the worked example.
//!
//! [`PanelTokens`] holds one pre-rendered `String` per `PanelTheme` method —
//! every method on this trait is required (no default bodies), so every
//! field is owned here.
//!
//! `action_bg_hover` has no companion-table row: it covers a literal
//! (`"rgba(255,255,255,0.08)"`, the header action-button hover highlight in
//! `render.rs`) the table's per-widget scan missed because it lives outside
//! `PanelTheme` entirely — H1 Brief 8b adds the trait method so the literal
//! is reachable through the token contract like every other panel colour.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "panel",
    struct PanelTokens, keys = PANEL_KEYS;

    // `surface.panel` collapse (H1 token contract design §1's flagged
    // decision) — every dark-set `panel_bg`/`row_bg_normal` in mlc's
    // unrelated GitHub-dark literal family lands on the shared
    // `surface.floating` value through this role.
    bg     => ColorSpec::Alias(Role::SurfacePanel, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),

    header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    header_text => ColorSpec::Alias(Role::TextSecondary, None),

    column_header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    column_header_text => ColorSpec::Alias(Role::TextSecondary, None),

    row_bg_normal   => ColorSpec::Alias(Role::SurfacePanel, None),
    row_bg_hover    => ColorSpec::Alias(Role::SurfaceControlHover, None),
    row_bg_selected => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.15))),

    footer_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    footer_text => ColorSpec::Alias(Role::TextSecondary, None),

    divider => ColorSpec::Alias(Role::BorderDefault, None),

    action_icon_normal => ColorSpec::Alias(Role::TextSecondary, None),
    action_icon_hover   => ColorSpec::Alias(Role::TextPrimary, None),

    sort_arrow_color => ColorSpec::Alias(Role::AccentDefault, None),

    // Header action-button hover highlight — see module doc.
    action_bg_hover => ColorSpec::Alias(Role::SurfaceControlHover, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_panel_key_is_registered() {
        let keys = crate::tokens::component_keys("panel").expect("panel must be registered");
        assert_eq!(keys, PANEL_KEYS);
        assert_eq!(keys.len(), 16, "one entry per PanelTokens field");
        assert!(keys.contains(&"action_bg_hover"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let p = tokens.panel();
        assert_eq!(p.bg, "#1e222d");
        assert_eq!(p.border, "#363a45");
        assert_eq!(p.header_bg, "#1e222d");
        assert_eq!(p.header_text, "#b2b5be");
        assert_eq!(p.column_header_bg, "#1e222d");
        assert_eq!(p.column_header_text, "#b2b5be");
        assert_eq!(p.row_bg_normal, "#1e222d");
        assert_eq!(p.row_bg_hover, "#2a2e39");
        assert_eq!(p.row_bg_selected, "#2962ff26");
        assert_eq!(p.footer_bg, "#1e222d");
        assert_eq!(p.footer_text, "#b2b5be");
        assert_eq!(p.divider, "#363a45");
        assert_eq!(p.action_icon_normal, "#b2b5be");
        assert_eq!(p.action_icon_hover, "#d1d4dc");
        assert_eq!(p.sort_arrow_color, "#2962ff");
        assert_eq!(p.action_bg_hover, "#2a2e39");
    }
}
