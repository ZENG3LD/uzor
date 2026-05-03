use std::collections::HashMap;
use crate::core::types::Rect;

/// Outcome of a composite drag-start consume.
///
/// Returned by `drag_outcome_*` helpers after `consume_event` returns `None`
/// (consumed). The app uses this to set its own `DragTarget` enum without
/// reading composite-internal state fields.
#[derive(Debug, Clone)]
pub enum DragOutcome {
    /// The modal body scrollbar thumb started dragging.
    ModalBodyScroll,
    /// The modal frame is being resized (resize-handle drag started).
    ModalResize,
    /// The popup body scrollbar thumb started dragging.
    PopupBodyScroll,
    /// The popup frame is being resized.
    PopupResize,
    /// A toolbar resize handle started dragging.  `which` is an app-supplied
    /// tag identifying the toolbar (e.g. `"top"`, `"demo-left2"`).
    ToolbarResize { which: &'static str },
    /// A sidebar resize handle started dragging.  `which` tags the sidebar.
    SidebarResize { which: &'static str },
    /// A sidebar scrollbar thumb started dragging.
    SidebarScrollbar {
        track_rect: Rect,
        content_h:  f64,
        viewport_h: f64,
    },
}

/// Side of the window edge for edge panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EdgeSide {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Kind of overlay composite widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    Dropdown,
    Popup,
    Modal,
    ContextMenu,
    ColorPicker,
    Tooltip,
}

/// Stable string identifier for a named slot in the layout.
pub type SlotId = String;

/// Computed rects for all four edge sides after a solve pass.
#[derive(Debug, Clone, Default)]
pub struct EdgeRects {
    /// Per-slot rects on the top edge, ordered by `EdgeSlot::order`.
    pub top: Vec<Rect>,
    /// Per-slot rects on the bottom edge, ordered by `EdgeSlot::order`.
    pub bottom: Vec<Rect>,
    /// Per-slot rects on the left edge, ordered by `EdgeSlot::order`.
    pub left: Vec<Rect>,
    /// Per-slot rects on the right edge, ordered by `EdgeSlot::order`.
    pub right: Vec<Rect>,
}

/// Overlay rect with kind and z-order hint for the renderer.
#[derive(Debug, Clone)]
pub struct OverlayRect {
    /// Overlay kind (used for z-ordering).
    pub kind: OverlayKind,
    /// Screen-space rect of the overlay.
    pub rect: Rect,
    /// Z-layer hint (assigned from `ZLayerTable`).
    pub z: i32,
}

/// Full result of a macro layout solve pass.
///
/// Produced by `solve_layout`; consumed by frame registration code.
#[derive(Debug, Clone, Default)]
pub struct LayoutSolved {
    /// Chrome strip rect, or `None` when chrome is hidden.
    pub chrome: Option<Rect>,
    /// Per-side per-slot edge rects.
    pub edges: EdgeRects,
    /// Dock content area (viewport minus chrome minus all edges).
    pub dock_area: Rect,
    /// Floating layer area — same as `dock_area` (floating is z-above dock).
    pub floating_area: Rect,
    /// Named overlay rects (keyed by `SlotId`).
    pub overlays: HashMap<SlotId, OverlayRect>,
}
