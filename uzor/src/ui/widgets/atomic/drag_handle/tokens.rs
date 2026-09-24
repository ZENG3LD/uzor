//! Token-contract implementation of
//! [`DragHandleTheme`](super::theme::DragHandleTheme) (H1 token contract
//! design §3; companion table
//! `docs/uzor/plans/h1-component-tokens-2026-09-24.md`, "drag_handle"
//! section).
//!
//! [`DragHandleTokens`] holds one pre-rendered `String` per
//! `DragHandleTheme` method — the trait's single required method.
//!
//! Built once per [`crate::tokens::set::Tokens`]
//! ([`crate::tokens::set::TokenSet::resolve`]), never recomputed per paint
//! call.

use crate::tokens::{component_tokens, ColorSpec, Role};

component_tokens! {
    widget = "drag_handle",
    struct DragHandleTokens, keys = DRAG_HANDLE_KEYS;

    grip_dots_color => ColorSpec::Alias(Role::TextDisabled, None),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{BuiltinSet, Tokens};

    #[test]
    fn every_drag_handle_key_is_registered() {
        let keys = crate::tokens::component_keys("drag_handle")
            .expect("drag_handle must be registered");
        assert_eq!(keys, DRAG_HANDLE_KEYS);
        assert_eq!(keys.len(), 1, "one entry per DragHandleTokens field");
        assert!(keys.contains(&"grip_dots_color"));
    }

    #[test]
    fn dark_defaults_match_the_companion_table() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let d = tokens.drag_handle();
        assert_eq!(d.grip_dots_color, "#6a6d78");
    }
}
