//! Size policy for self-sizing composites (popup, dropdown).
//!
//! Orthogonal to [`OverflowMode`](super::overflow::OverflowMode):
//! `SizeMode` decides what the composite's natural rect is, `OverflowMode`
//! decides what to do when that rect doesn't fit in the viewport.

/// How a composite picks its outer rect.
///
/// Used by composites that decide their own size (popups, dropdowns) — the
/// modal/sidebar/toolbar have their size driven by the layout solver, not
/// by this enum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeMode {
    /// Composite measures its content each frame and uses that rect. The
    /// host has nothing to set — the popup just hugs its content.
    AutoFit,

    /// Caller-supplied fixed `(width, height)`. The composite ignores its
    /// content's natural size.
    Fixed(f64, f64),
}

impl Default for SizeMode {
    fn default() -> Self { SizeMode::AutoFit }
}
