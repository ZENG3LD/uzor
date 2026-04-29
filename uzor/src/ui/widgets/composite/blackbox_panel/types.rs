//! BlackboxPanel type definitions — per-frame view data, event enum, render kind.
//!
//! # Defining constraint
//!
//! `BlackboxView::body` does NOT receive `&mut InputCoordinator`.
//! Caller handles all sub-element hit-testing internally.
//! The coordinator registers ONE rect for the whole panel; no children.

use crate::input::Sense;
use crate::input::MouseButton;
use crate::input::KeyCode;
use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::BlackboxPanelSettings;

// ---------------------------------------------------------------------------
// BlackboxEvent
// ---------------------------------------------------------------------------

/// Input event forwarded to the caller's handler closure.
///
/// All coordinates are in panel-local space (origin = panel top-left corner).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BlackboxEvent {
    /// Pointer moved inside the panel.
    PointerMove { local_x: f64, local_y: f64 },
    /// Pointer button pressed inside the panel.
    PointerDown { local_x: f64, local_y: f64, button: MouseButton },
    /// Pointer button released (anywhere after a press inside the panel).
    PointerUp   { local_x: f64, local_y: f64, button: MouseButton },
    /// Mouse wheel / trackpad scroll.
    Wheel       { delta_x: f64, delta_y: f64 },
    /// Key pressed while the panel has focus.
    KeyPress    { key: KeyCode },
    /// Focus gained (`true`) or lost (`false`).
    Focus(bool),
    /// Pointer entered the panel rect.
    PointerEnter,
    /// Pointer left the panel rect.
    PointerLeave,
}

// ---------------------------------------------------------------------------
// BlackboxEventResult
// ---------------------------------------------------------------------------

/// Result returned by the caller's event handler.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlackboxEventResult {
    /// Event was consumed — coordinator should not propagate it further.
    Consumed,
    /// Event was not handled — coordinator may propagate it.
    NotConsumed,
    /// Event was handled and the panel needs a redraw.
    Redraw,
}

// ---------------------------------------------------------------------------
// BlackboxView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_blackbox_panel`.
pub struct BlackboxView<'a> {
    /// Optional title string displayed in the header strip
    /// (`WithHeader` / `WithHeaderBorder` kinds).
    pub title: Option<&'a str>,

    /// Body render closure.
    ///
    /// Called with `(ctx, body_rect)` — **no** `&mut InputCoordinator`.
    /// Caller handles all sub-element rendering and input internally.
    pub body: Box<dyn FnMut(&mut dyn RenderContext, Rect) + 'a>,

    /// Event handler called by `dispatch_blackbox_event`.
    pub handle_event: Box<dyn FnMut(BlackboxEvent) -> BlackboxEventResult + 'a>,

    /// Sense flags registered with the coordinator.
    ///
    /// Typical full value: `Sense::CLICK | Sense::HOVER | Sense::DRAG | Sense::SCROLL`.
    pub sense: Sense,
}

// ---------------------------------------------------------------------------
// BlackboxRenderKind
// ---------------------------------------------------------------------------

/// Selects the layout / draw pipeline for the blackbox panel.
///
/// | Kind               | header | border | body |
/// |--------------------|--------|--------|------|
/// | `Default`          | ✗      | ✗      | ✓    |
/// | `WithHeader`       | ✓      | ✗      | ✓    |
/// | `WithBorder`       | ✗      | ✓      | ✓    |
/// | `WithHeaderBorder` | ✓      | ✓      | ✓    |
/// | `Custom`           | —      | —      | —    |
pub enum BlackboxRenderKind {
    /// Background fill only; body fills the whole rect.
    Default,
    /// Header strip (title + close-X chrome) above body rect.
    WithHeader,
    /// 1 px border around body; no header.
    WithBorder,
    /// Header strip + 1 px border.
    WithHeaderBorder,
    /// Escape hatch — caller drives every draw call.
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &BlackboxView<'_>, &BlackboxPanelSettings)>),
}
