//! Token-contract implementation of
//! [`ContainerTheme`](super::theme::ContainerTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "container" section).
//!
//! [`ContainerTokens`] holds one pre-rendered `String` per `ContainerTheme`
//! method — 3 required (`bg`/`border`/`shadow`) plus 5 default-body methods
//! that each return their own literal rather than delegating to another
//! `self.method()` call (H1's "scope note on trait-default methods":
//! `card_shadow_color`, `section_header_bg`, `section_header_text`,
//! `panel_bg`, `panel_border` all qualify, bridging the two mlc families
//! this trait covers — `ToolbarTheme` slots (`bg`/`border`/`shadow`) and
//! `PanelTheme` slots (`panel_bg`/`panel_border`) — per this trait's own
//! module doc), plus `section_border` (H1 Brief 10a-3 item 0a — a required
//! method added directly, not a trait-default body).
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, ColorValue, Role};

component_tokens! {
    widget = "container",
    struct ContainerTokens, keys = CONTAINER_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceFloating, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),

    card_shadow_color => ColorSpec::Alias(Role::ShadowDefault, None),

    section_header_bg   => ColorSpec::Alias(Role::SurfaceHeader, None),
    section_header_text => ColorSpec::Alias(Role::TextOnAccent, None),

    // PanelTheme bridge (module doc): `surface.panel` collapses onto
    // `surface.floating` in every built-in set (same consolidation the
    // `panel` composite widget uses, main design doc §1).
    panel_bg     => ColorSpec::Alias(Role::SurfacePanel, None),
    panel_border => ColorSpec::Alias(Role::BorderDefault, None),

    // No stroke by default (`draw_section_container` skips it when
    // "transparent") — `high_contrast`/`high_contrast_mono` override this to
    // `{color.border.default}` (H1 Brief 10a-3 item 0a; `theme.rs`'s own doc
    // comment on this method has the full rationale).
    section_border => ColorSpec::Literal(ColorValue::Transparent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_container_key_is_registered() {
        let keys =
            crate::tokens::component_keys("container").expect("container must be registered");
        assert_eq!(keys, CONTAINER_KEYS);
        assert_eq!(keys.len(), 9, "one entry per ContainerTokens field");
        assert!(keys.contains(&"bg"));
        assert!(keys.contains(&"panel_border"));
        assert!(keys.contains(&"section_border"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let c = tokens.container();
        assert_eq!(c.bg, "#1e222d");
        assert_eq!(c.border, "#363a45");
        assert_eq!(c.shadow, "#00000066");
        assert_eq!(c.card_shadow_color, "#00000066");
        assert_eq!(c.section_header_bg, "#1e222d");
        assert_eq!(c.section_header_text, "#d1d4dc");
        assert_eq!(c.panel_bg, "#1e222d");
        assert_eq!(c.panel_border, "#363a45");
        assert_eq!(c.section_border, "transparent");
    }

    /// `high_contrast`/`high_contrast_mono` override `section_border` to
    /// `{color.border.default}` (H1 Brief 10a-3 item 0a); `dark`/`light` keep
    /// the "no stroke" default.
    #[test]
    fn only_high_contrast_sets_override_section_border() {
        let dark = Tokens::builtin(BuiltinSet::Dark);
        let light = Tokens::builtin(BuiltinSet::Light);
        let hc = Tokens::builtin(BuiltinSet::HighContrast);
        let hc_mono = Tokens::builtin(BuiltinSet::HighContrastMono);

        assert_eq!(dark.container().section_border, "transparent");
        assert_eq!(light.container().section_border, "transparent");
        assert_eq!(hc.container().section_border, hc.container().border);
        assert_eq!(hc_mono.container().section_border, hc_mono.container().border);
        assert_ne!(hc.container().section_border, "transparent");
        assert_ne!(hc_mono.container().section_border, "transparent");
    }
}
