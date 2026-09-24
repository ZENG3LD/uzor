//! Token-contract implementation of
//! [`TooltipTheme`](super::theme::TooltipTheme) (H1 token contract design
//! §3; companion table `docs/uzor/plans/h1-component-tokens-2026-09-24.md`,
//! "tooltip" section, `Default::*` row).
//!
//! [`TooltipTokens`] holds one pre-rendered `String` per `TooltipTheme`
//! method — the 3 required (`bg`/`border`/`text`) plus `shadow`, a
//! default-body method that returns its own literal (`"#00000060"`) rather
//! than delegating to another method (H1's "scope note on trait-default
//! methods") — `TokenTheme` overrides it explicitly, same as `container`'s
//! `card_shadow_color` et al.
//!
//! `ChromeTooltipTheme`/`CrosshairTooltipTheme` are gone (see `theme.rs`'s
//! module doc) — there is only ever one tooltip look now, this one.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "tooltip",
    struct TooltipTokens, keys = TOOLTIP_KEYS;

    bg     => ColorSpec::Alias(Role::SurfaceControlHover, None),
    border => ColorSpec::Alias(Role::BorderDefault, None),
    text   => ColorSpec::Alias(Role::TextOnAccent, None),
    shadow => ColorSpec::Alias(Role::ShadowDefault, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_tooltip_key_is_registered() {
        let keys = crate::tokens::component_keys("tooltip").expect("tooltip must be registered");
        assert_eq!(keys, TOOLTIP_KEYS);
        assert_eq!(keys.len(), 4, "one entry per TooltipTokens field");
        assert!(keys.contains(&"shadow"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let t = tokens.tooltip();
        assert_eq!(t.bg, "#2a2e39");
        assert_eq!(t.border, "#363a45");
        assert_eq!(t.text, "#d1d4dc");
        // shadow.default = rgb(0,0,0) at alpha 0.4 (0.4 * 255 = 102 = 0x66) —
        // format-normalizes the old hex-alpha-suffix literal "#00000060"
        // into the canonical `rgba()`-derived hex form (main design doc §1's
        // `shadow.default` note).
        assert_eq!(t.shadow, "#00000066");
    }
}
