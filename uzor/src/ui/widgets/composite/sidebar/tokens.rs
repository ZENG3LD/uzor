//! Token-contract implementation of
//! [`SidebarTheme`](super::theme::SidebarTheme) (H1 token contract design
//! §3; companion table `docs/uzor/plans/h1-component-tokens-2026-09-24.md`,
//! "sidebar" section). Same shape as `atomic/button/tokens.rs`, the worked
//! example.
//!
//! [`SidebarTokens`] holds one pre-rendered `String` per `SidebarTheme`
//! method — every method on this trait is required (no default bodies), so
//! every field is owned here.
//!
//! Fields from `bg` through `tab_bg_hover` (15) are the companion table's
//! own rows. The remaining fields have no companion-table row: `input.rs`'s
//! `SidebarBodyBuilder` (a generic body-content helper, live in
//! `uzor-examples/src/l3/dashboard.rs`'s sidebar spawn/panel-list UI) and
//! `render.rs`'s header-action hover highlight and chevron-strip backdrop
//! painted literals with no `SidebarTheme` slot to read at all — H1 Brief
//! 8b adds one trait method per literal so every colour in this composite
//! (not just the ones the companion table's per-widget scan happened to
//! find in `theme.rs`) is reachable through the token contract:
//!
//! - `action_bg_hover` — header action-button hover highlight (`render.rs`).
//! - `chevron_strip_bg` — chevron-pager backdrop strip (`render.rs`).
//! - `accent` — generic accent for body-content controls (radio-button dot,
//!   action-button fill); mirrors `popup::accent`'s already-established
//!   multi-purpose role.
//! - `on_accent_text` — text painted on an accent-filled generic control
//!   (selected radio label, action-button label).
//! - `content_text` — primary body text for generic content rows (inactive
//!   panel-list title).
//! - `content_muted_text` — de-emphasized body text (unselected radio
//!   label).
//! - `section_header_text` — all-caps section heading (`"NEW PANEL"`).
//! - `sub_label_text` — muted sub-label (`"Type:"`, `"Split:"`).
//! - `radio_dot_inactive` — unselected radio-dot fill.
//! - `panel_row_bg_active` / `panel_row_bg_inactive` — dock-panel list row
//!   backgrounds.
//! - `panel_close_icon` — dock-panel list row close-glyph colour.
//!
//! (`divider` and the accent-fill radio dot / action button already have a
//! home in the 15 companion-table fields / the new `accent` field above —
//! `SidebarBodyBuilder::add_divider` reuses `divider()` directly, no
//! separate field.)
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Modifier, Role};

component_tokens! {
    widget = "sidebar",
    struct SidebarTokens, keys = SIDEBAR_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),

    header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    header_text => ColorSpec::Alias(Role::TextOnAccent, None),
    header_icon => ColorSpec::Alias(Role::TextSecondary, None),
    divider     => ColorSpec::Alias(Role::BorderDefault, None),

    action_icon_normal => ColorSpec::Alias(Role::TextSecondary, None),
    action_icon_hover   => ColorSpec::Alias(Role::TextOnAccent, None),

    scrollbar_thumb        => ColorSpec::Alias(Role::BorderDefault, None),
    scrollbar_thumb_active => ColorSpec::Alias(Role::TextMuted, None),

    tab_text_active   => ColorSpec::Alias(Role::TextOnAccent, None),
    tab_text_inactive => ColorSpec::Alias(Role::TextMuted, None),
    tab_accent        => ColorSpec::Alias(Role::AccentDefault, None),
    tab_bg_active     => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.12))),
    tab_bg_hover      => ColorSpec::Alias(Role::SurfaceControlHover, None),

    // --- render.rs literals with no prior SidebarTheme slot (module doc) ---
    action_bg_hover  => ColorSpec::Alias(Role::SurfaceControlHover, None),
    chevron_strip_bg => ColorSpec::Alias(Role::SurfaceAppChrome, Some(Modifier::Alpha(0.85))),

    // --- input.rs::SidebarBodyBuilder literals (module doc) ---
    accent              => ColorSpec::Alias(Role::AccentDefault, None),
    on_accent_text      => ColorSpec::Alias(Role::TextOnAccent, None),
    content_text        => ColorSpec::Alias(Role::TextPrimary, None),
    content_muted_text  => ColorSpec::Alias(Role::TextSecondary, None),
    section_header_text => ColorSpec::Alias(Role::TextMuted, None),
    sub_label_text      => ColorSpec::Alias(Role::TextSecondary, None),
    radio_dot_inactive  => ColorSpec::Alias(Role::SurfaceControlHover, None),
    panel_row_bg_active   => ColorSpec::Alias(Role::AccentDefault, Some(Modifier::Alpha(0.18))),
    panel_row_bg_inactive => ColorSpec::Alias(Role::SurfaceControlHover, None),
    panel_close_icon     => ColorSpec::Alias(Role::StatusDanger, Some(Modifier::Alpha(0.5))),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_sidebar_key_is_registered() {
        let keys = crate::tokens::component_keys("sidebar").expect("sidebar must be registered");
        assert_eq!(keys, SIDEBAR_KEYS);
        assert_eq!(keys.len(), 27, "one entry per SidebarTokens field");
        assert!(keys.contains(&"panel_close_icon"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let s = tokens.sidebar();
        assert_eq!(s.bg, "#1e222d");
        assert_eq!(s.border, "#363a45");
        assert_eq!(s.header_bg, "#1e222d");
        assert_eq!(s.header_text, "#d1d4dc");
        assert_eq!(s.header_icon, "#b2b5be");
        assert_eq!(s.divider, "#363a45");
        assert_eq!(s.action_icon_normal, "#b2b5be");
        assert_eq!(s.action_icon_hover, "#d1d4dc");
        assert_eq!(s.scrollbar_thumb, "#363a45");
        assert_eq!(s.scrollbar_thumb_active, "#787b86");
        assert_eq!(s.tab_text_active, "#d1d4dc");
        assert_eq!(s.tab_text_inactive, "#787b86");
        assert_eq!(s.tab_accent, "#2962ff");
        assert_eq!(s.tab_bg_active, "#2962ff1f");
        assert_eq!(s.tab_bg_hover, "#2a2e39");
        assert_eq!(s.action_bg_hover, "#2a2e39");
        assert_eq!(s.chevron_strip_bg, "#131722d9");
        assert_eq!(s.accent, "#2962ff");
        assert_eq!(s.on_accent_text, "#d1d4dc");
        assert_eq!(s.content_text, "#d1d4dc");
        assert_eq!(s.content_muted_text, "#b2b5be");
        assert_eq!(s.section_header_text, "#787b86");
        assert_eq!(s.sub_label_text, "#b2b5be");
        assert_eq!(s.radio_dot_inactive, "#2a2e39");
        assert_eq!(s.panel_row_bg_active, "#2962ff2e");
        assert_eq!(s.panel_row_bg_inactive, "#2a2e39");
        assert_eq!(s.panel_close_icon, "#f2364580");
    }
}
