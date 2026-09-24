//! The 4 generic built-in token sets (H1 token contract design §1/§6 Brief
//! 3): `dark`/`light`/`high_contrast`/`high_contrast_mono`, values transcribed
//! verbatim from MLC's `UIColors` at the cited commit (see
//! `uzor/tests/mlc_uicolors_mapping.rs`'s header for the exact commit and
//! line numbers — that test is the proof these JSON files stay correct).
//!
//! Each set is embedded at compile time ([`include_str!`]) and lazily
//! parsed/resolved once per process, cached behind a `OnceLock` keyed by
//! [`BuiltinSet`] variant — [`Tokens::builtin`] never re-parses on repeat
//! calls, only bumps an `Arc` refcount.

use std::sync::{Arc, OnceLock};

use crate::tokens::loader::load_token_set;
use crate::tokens::set::Tokens;

const DARK_JSON: &str = include_str!("builtin/dark.json");
const LIGHT_JSON: &str = include_str!("builtin/light.json");
const HIGH_CONTRAST_JSON: &str = include_str!("builtin/high_contrast.json");
const HIGH_CONTRAST_MONO_JSON: &str = include_str!("builtin/high_contrast_mono.json");

/// The 4 generic built-in token sets uzor ships (H1 §1 D5) — `mascot` stays
/// MLC-owned data (not shipped here) and the macOS pair lands in Brief 11.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinSet {
    Dark,
    Light,
    HighContrast,
    HighContrastMono,
}

impl BuiltinSet {
    /// Every variant, for exhaustive iteration in tests and golden grids.
    pub const ALL: &'static [BuiltinSet] =
        &[BuiltinSet::Dark, BuiltinSet::Light, BuiltinSet::HighContrast, BuiltinSet::HighContrastMono];

    fn json(self) -> &'static str {
        match self {
            BuiltinSet::Dark => DARK_JSON,
            BuiltinSet::Light => LIGHT_JSON,
            BuiltinSet::HighContrast => HIGH_CONTRAST_JSON,
            BuiltinSet::HighContrastMono => HIGH_CONTRAST_MONO_JSON,
        }
    }
}

static DARK: OnceLock<Arc<Tokens>> = OnceLock::new();
static LIGHT: OnceLock<Arc<Tokens>> = OnceLock::new();
static HIGH_CONTRAST: OnceLock<Arc<Tokens>> = OnceLock::new();
static HIGH_CONTRAST_MONO: OnceLock<Arc<Tokens>> = OnceLock::new();

impl Tokens {
    /// Loads and resolves one of the 4 built-in sets, `OnceLock`-cached per
    /// variant — parsed once, `Arc`-shared thereafter.
    ///
    /// A built-in JSON file that fails to parse or resolve is a bug in this
    /// crate (a compile-time-embedded asset, never user input) — every one
    /// of the 4 must load cleanly, proven exhaustively by this module's own
    /// `all_four_builtin_sets_load_and_resolve` test, so the `panic!` below
    /// can never fire outside a broken build of uzor itself.
    pub fn builtin(set: BuiltinSet) -> Arc<Tokens> {
        let cell = match set {
            BuiltinSet::Dark => &DARK,
            BuiltinSet::Light => &LIGHT,
            BuiltinSet::HighContrast => &HIGH_CONTRAST,
            BuiltinSet::HighContrastMono => &HIGH_CONTRAST_MONO,
        };
        Arc::clone(cell.get_or_init(|| Arc::new(load_builtin(set))))
    }
}

fn load_builtin(set: BuiltinSet) -> Tokens {
    let token_set = load_token_set(set.json())
        .unwrap_or_else(|e| panic!("built-in token set {set:?} failed to parse: {e}"));
    token_set.resolve().unwrap_or_else(|e| panic!("built-in token set {set:?} failed to resolve: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::color::ColorValue;

    #[test]
    fn all_four_builtin_sets_load_and_resolve() {
        for set in BuiltinSet::ALL {
            let tokens = Tokens::builtin(*set);
            assert!(tokens.name.is_some(), "{set:?}: built-in set must carry a $name");
            assert_ne!(
                tokens.semantic.surface_app_chrome,
                ColorValue::Transparent,
                "{set:?}: surface_app_chrome must resolve to a real colour"
            );
        }
    }

    #[test]
    fn builtin_is_cached_not_reparsed() {
        let first = Tokens::builtin(BuiltinSet::Dark);
        let second = Tokens::builtin(BuiltinSet::Dark);
        assert!(Arc::ptr_eq(&first, &second), "repeat calls must share the same cached Arc");
    }

    #[test]
    fn high_contrast_mono_radius_is_zero() {
        let tokens = Tokens::builtin(BuiltinSet::HighContrastMono);
        assert_eq!(tokens.geometry.radius_md, 0.0);
        assert_eq!(tokens.geometry.radius_sm, 0.0);
        assert_eq!(tokens.geometry.radius_lg, 0.0);
    }

    #[test]
    fn dark_radius_matches_the_role_1_scale() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        assert_eq!(tokens.geometry.radius_sm, 2.0);
        assert_eq!(tokens.geometry.radius_md, 4.0);
        assert_eq!(tokens.geometry.radius_lg, 8.0);
    }

    #[test]
    fn surface_header_and_panel_alias_surface_floating_in_every_set() {
        for set in BuiltinSet::ALL {
            let tokens = Tokens::builtin(*set);
            let s = &tokens.semantic;
            assert_eq!(s.surface_header, s.surface_floating, "{set:?}: surface_header must alias surface_floating");
            assert_eq!(s.surface_panel, s.surface_floating, "{set:?}: surface_panel must alias surface_floating");
        }
    }
}
