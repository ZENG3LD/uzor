//! Popup type definitions — universal popup composite.
//!
//! Popup is a *transient surface* that hugs its content. There are only two
//! variants:
//! - `Plain`  — frame + caller-drawn body
//! - `Custom` — escape hatch (caller drives every paint call)
//!
//! Anything fancier (color pickers, indicator strips, swatch grids) is a
//! domain-specific composition the *application* assembles inside a `Plain`
//! popup. The composite has no knowledge of business templates.

use super::settings::PopupSettings;
use super::state::PopupState;
use crate::render::RenderContext;
use crate::types::Rect;

// ---------------------------------------------------------------------------
// BackdropKind
// ---------------------------------------------------------------------------

/// Controls any fill drawn behind the popup frame.
///
/// Popups are non-modal by default (`None`). A popup that should block
/// lower-layer events sets `Dim` so the coordinator gets a hint to apply
/// modal-blocking hit-test rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackdropKind {
    /// No backdrop — popup floats freely (default for non-modal popups).
    #[default]
    None,
    /// Semi-transparent dim fill `rgba(0,0,0,0.45)`.
    Dim,
}

// ---------------------------------------------------------------------------
// PopupView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_popup`.
pub struct PopupView<'a> {
    /// Top-left origin of the popup in screen coordinates.
    pub origin: (f64, f64),

    /// Anchor rect used for smart re-positioning on window resize.
    /// `None` — fixed origin, no re-anchor.
    pub anchor: Option<Rect>,

    /// Backdrop fill strategy (non-modal popups use `None`).
    pub backdrop: BackdropKind,

    /// Template-specific data and (for `Plain`) the body closure.
    pub kind: PopupViewKind<'a>,

    /// How the popup picks its outer rect. `AutoFit` (default) measures
    /// content; `Fixed(w, h)` pins the rect.
    pub size_mode: crate::types::SizeMode,

    /// What to do when content exceeds the popup rect (the viewport
    /// shrunk an `AutoFit` popup, or `Fixed` is smaller than content).
    /// `Clip` just hides; `Scrollbar` / `Chevrons` activate paging.
    /// Popups never resize / drag — for that, use a modal.
    pub overflow: crate::types::OverflowMode,
}

// ---------------------------------------------------------------------------
// PopupViewKind
// ---------------------------------------------------------------------------

/// Template-specific per-frame data.
pub enum PopupViewKind<'a> {
    /// Frame + caller-drawn body. The composite paints the chrome
    /// (shadow / background / border); the caller fills the body rect
    /// with whatever atomic / composite widgets it needs.
    Plain,

    /// Escape hatch — the caller drives every paint call. No frame is
    /// drawn by the composite when this variant is active.
    Custom {
        /// Caller-supplied draw closure.
        draw: Box<dyn Fn(&mut dyn RenderContext, Rect, &PopupState, &PopupSettings) + 'a>,
    },
}

// ---------------------------------------------------------------------------
// PopupRenderKind  (discriminant-only, for registration/layout dispatch)
// ---------------------------------------------------------------------------

/// Layout / input registration strategy selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupRenderKind {
    /// Frame only — body via `PopupViewKind::Plain`.
    Plain,
    /// Escape hatch — caller drives all draw calls.
    Custom,
}
