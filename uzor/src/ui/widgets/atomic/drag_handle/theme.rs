//! Drag handle theme trait and default implementation.

/// Colour slots for the drag handle grip-dots visual.
pub trait DragHandleTheme {
    /// Fill colour of each grip dot.  Default: `#4a4e5a`.
    fn grip_dots_color(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultDragHandleTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 6 — `crate::tokens::theme::TokenTheme` is now the one
// `DragHandleTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::drag_handle::tokens::DragHandleTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
