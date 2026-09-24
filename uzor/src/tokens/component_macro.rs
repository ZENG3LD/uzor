//! `component_tokens!`: generates one widget's component-token struct, its
//! `resolve()`, and its `component.<widget>.*` key registry constant from a
//! SINGLE table (method name => default [`crate::tokens::ColorSpec`]) — so a
//! key can never exist in the struct but be missing from the registry, or
//! the reverse (H1 token contract design §3). `atomic/button/tokens.rs` is
//! the worked example; every later widget's own `tokens.rs` copies this
//! recipe.
//!
//! ```ignore
//! // ui/widgets/atomic/checkbox/tokens.rs
//! use crate::tokens::{component_tokens, ColorSpec, ColorValue, Modifier, Role};
//!
//! component_tokens! {
//!     widget = "checkbox",
//!     struct CheckboxTokens, keys = CHECKBOX_KEYS;
//!
//!     // One arm per `CheckboxTheme` method that OWNS a literal — required
//!     // methods, plus any default-body method that returns its own
//!     // literal rather than delegating to `self.other_method()` (H1's
//!     // "scope note on trait-default methods"). Pure delegators need no
//!     // arm here and no override in `impl CheckboxTheme for TokenTheme`.
//!     checkbox_bg_checked => ColorSpec::Alias(Role::AccentDefault, None),
//!     checkbox_border      => ColorSpec::Literal(ColorValue::Transparent),
//! }
//! ```
//!
//! Then, three more steps land the widget on the token contract:
//! 1. Add `"checkbox" => Some(crate::ui::widgets::atomic::checkbox::tokens::CHECKBOX_KEYS)`
//!    to the match in [`crate::tokens::component_keys`].
//! 2. Add a `checkbox: crate::ui::widgets::atomic::checkbox::tokens::CheckboxTokens`
//!    field to [`crate::tokens::set::Tokens`] (built in
//!    [`crate::tokens::set::TokenSet::resolve`] via `CheckboxTokens::resolve(&semantic,
//!    &self.components)`), plus a `pub(crate) fn checkbox(&self) -> &CheckboxTokens`
//!    accessor.
//! 3. Add `impl CheckboxTheme for TokenTheme` in `tokens/theme.rs`, one line
//!    per owned method reading `&self.0.checkbox().<field>`.
macro_rules! component_tokens {
    (
        widget = $widget:literal,
        struct $Tokens:ident, keys = $KEYS:ident;
        $( $field:ident => $default:expr ),+ $(,)?
    ) => {
        #[derive(Clone, Debug)]
        pub struct $Tokens {
            $( pub $field: String, )+
        }

        /// The `component.<widget>.*` keys this widget accepts — registered
        /// with [`crate::tokens::component_keys`] so a token file may
        /// override any of them.
        pub(crate) const $KEYS: &[&str] = &[ $( stringify!($field) ),+ ];

        impl $Tokens {
            /// Resolves every field: an authored `component.<widget>.<field>`
            /// override wins ([`crate::tokens::set::ComponentOverrides::get`]),
            /// otherwise the default [`crate::tokens::ColorSpec`] given above
            /// is resolved against `roles`. Called once per
            /// [`crate::tokens::set::TokenSet::resolve`], never per paint call.
            pub fn resolve(
                roles: &$crate::tokens::semantic::SemanticRoles,
                overrides: &$crate::tokens::set::ComponentOverrides,
            ) -> Self {
                $(
                    let $field = overrides
                        .get($widget, stringify!($field))
                        .cloned()
                        .unwrap_or_else(|| $default)
                        .resolve(roles)
                        .to_css()
                        .into_owned();
                )+
                Self { $( $field ),+ }
            }
        }
    };
}

pub(crate) use component_tokens;
