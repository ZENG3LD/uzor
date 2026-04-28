//! Modal type definitions.

use crate::input::InputCoordinator;
use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::ModalSettings;

// ---------------------------------------------------------------------------
// Backdrop
// ---------------------------------------------------------------------------

/// Controls the fill drawn behind the modal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropKind {
    /// No backdrop fill — modal floats freely.
    /// Used by regular settings modals (IndicatorSettings, ChartSettings, etc.).
    None,
    /// Semi-transparent dim — `rgba(0,0,0,0.45)`.
    /// Used by sub-modals (TemplateName) drawn above a parent settings modal.
    Dim,
    /// Full opaque block — Wizard / unlock screens.
    /// Coordinator intercepts every event behind the modal.
    FullBlock,
}

// ---------------------------------------------------------------------------
// Footer button descriptors
// ---------------------------------------------------------------------------

/// Describes one footer action button passed to the modal composite.
pub struct FooterBtn<'a> {
    /// Display label ("Save", "Cancel", "OK", …).
    pub label: &'a str,
    /// Visual variant selects which atomic button draw function is used.
    pub style: FooterBtnStyle,
}

/// Visual variant for a footer action button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterBtnStyle {
    /// Blue filled primary action button (OK, Save, Create).
    Primary,
    /// Ghost / outline cancel button.
    Ghost,
    /// Red semi-transparent destructive action button.
    Danger,
}

// ---------------------------------------------------------------------------
// Wizard page descriptor
// ---------------------------------------------------------------------------

/// Minimal per-page info needed by the composite to render the page indicator.
pub struct WizardPageInfo<'a> {
    /// Optional page label (not rendered by the composite — reserved for future use).
    pub label: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Per-frame data handed to `draw_modal`.
///
/// Fields not relevant to the selected `ModalRenderKind` are simply ignored —
/// the composite only reads what the active layout pipeline needs.
/// For example, `tabs` is ignored by `Plain` / `WithHeader` / `WithHeaderFooter` / `Wizard`.
pub struct ModalView<'a> {
    /// Title text rendered in the header zone.
    ///
    /// Ignored by `Plain` (caller draws title inside the body closure).
    /// Ignored by `Wizard` (no header).
    pub title: Option<&'a str>,

    /// Tab labels for `TopTabs` (horizontal strip) and `SideTabs` (icon sidebar).
    ///
    /// Ignored by `Plain`, `WithHeader`, `WithHeaderFooter`, `Wizard`.
    pub tabs: &'a [&'a str],

    /// Footer action buttons for `WithHeaderFooter`, `SideTabs`.
    ///
    /// Ignored by `Plain`, `WithHeader`, `TopTabs` (optional footer handled via
    /// `footer_buttons.is_empty()`), `Wizard`.
    pub footer_buttons: &'a [FooterBtn<'a>],

    /// Page descriptors for `Wizard` — length determines page count.
    ///
    /// Ignored by all non-Wizard kinds.
    pub wizard_pages: &'a [WizardPageInfo<'a>],

    /// Backdrop fill strategy.
    pub backdrop: BackdropKind,

    /// Body closure — called by the composite with the computed body rect after
    /// the frame, header, and tabs are drawn.
    ///
    /// The caller registers and draws whatever it wants inside the body area.
    ///
    /// Per-frame `Box` allocation is acceptable here (single per-frame alloc).
    pub body: Box<dyn FnMut(&mut dyn RenderContext, Rect, &mut InputCoordinator) + 'a>,
}

// ---------------------------------------------------------------------------
// RenderKind
// ---------------------------------------------------------------------------

/// Selects which layout pipeline the composite runs.
///
/// | Kind | frame | close-X | drag header | tabs | footer btns | wizard nav | body |
/// |------|-------|---------|-------------|------|-------------|------------|------|
/// | `Plain` | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
/// | `WithHeader` | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ | ✓ |
/// | `WithHeaderFooter` | ✓ | ✓ | ✓ | ✗ | ✓ | ✗ | ✓ |
/// | `TopTabs` | ✓ | ✓ | ✓ | top | optional | ✗ | ✓ |
/// | `SideTabs` | ✓ | ✓ | ✓ | side | ✓ | ✗ | ✓ |
/// | `Wizard` | ✓ | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ |
pub enum ModalRenderKind {
    /// Frame only — body fills the whole interior.
    Plain,
    /// Frame + draggable header with title and close-X. No footer, no tabs.
    WithHeader,
    /// Frame + draggable header + footer with action buttons.
    WithHeaderFooter,
    /// Frame + header + horizontal tab bar + optional footer.
    TopTabs,
    /// Frame + header + vertical icon sidebar + footer with action buttons.
    SideTabs,
    /// Fullscreen-blocking frame with wizard page indicator and Back/Next nav.
    Wizard,
    /// Escape hatch — caller drives every draw call.
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ModalView<'_>, &ModalSettings)>),
}
