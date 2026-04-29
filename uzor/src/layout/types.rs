use std::collections::HashMap;
use crate::core::types::Rect;

/// Side of the window edge for edge panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeSide {
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
