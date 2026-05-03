//! `PopupBuilder` — chainable, default-friendly wrapper around
//! `register_layout_manager_popup`.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_popup("color-picker");
//! lm::popup(&h)
//!     .anchor_to("toolbar:tb-help") // auto-resolve from coord
//!     .build(&mut layout, &mut render);
//! ```

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::layout::{LayoutManager, LayoutNodeId, PopupHandle, PopupNode};
use uzor::render::RenderContext;
use uzor::types::{OverflowMode, SizeMode};
use uzor::ui::widgets::composite::popup::input::register_layout_manager_popup;
use uzor::ui::widgets::composite::popup::settings::PopupSettings;
use uzor::ui::widgets::composite::popup::types::{
    BackdropKind, PopupRenderKind, PopupView, PopupViewKind,
};

/// Chainable builder for a popup overlay.
pub struct PopupBuilder<'a> {
    handle:           &'a PopupHandle,
    parent:           LayoutNodeId,
    slot_id:          Option<&'a str>,
    overlay_rect:     Option<Rect>,
    anchor:           Option<Rect>,
    /// Pending widget-id lookup — resolved at `.build()` via coord.
    anchor_widget_id: Option<&'a str>,
    origin:           (f64, f64),
    backdrop:         BackdropKind,
    size_mode:        SizeMode,
    overflow:         OverflowMode,
    settings:         Option<PopupSettings>,
    kind:             PopupRenderKind,
}

/// Entry point: start a `PopupBuilder` for the given handle.
pub fn popup<'a>(handle: &'a PopupHandle) -> PopupBuilder<'a> {
    PopupBuilder::new(handle)
}

impl<'a> PopupBuilder<'a> {
    /// New builder with all fields at default.
    pub fn new(handle: &'a PopupHandle) -> Self {
        Self {
            handle,
            parent:           LayoutNodeId::ROOT,
            slot_id:          None,
            overlay_rect:     None,
            anchor:           None,
            anchor_widget_id: None,
            origin:           (0.0, 0.0),
            backdrop:         BackdropKind::default(),
            size_mode:        SizeMode::default(),
            overflow:         OverflowMode::Clip,
            settings:         None,
            kind:             PopupRenderKind::Plain,
        }
    }

    /// Override the parent layout node (default `LayoutNodeId::ROOT`).
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Override the overlay slot id (default = handle id).
    pub fn slot_id(mut self, s: &'a str) -> Self { self.slot_id = Some(s); self }

    /// Top-left screen origin of the popup (default `(0, 0)`).
    pub fn origin(mut self, o: (f64, f64)) -> Self { self.origin = o; self }

    /// Explicit overlay rect override (default: derived from origin / anchor +
    /// `size_mode`).
    pub fn rect(mut self, r: Rect) -> Self { self.overlay_rect = Some(r); self }

    /// Anchor rect used for smart re-positioning on viewport resize.
    pub fn anchor(mut self, r: Rect) -> Self { self.anchor = Some(r); self }

    /// Auto-anchor to a registered widget by id — at `.build()` time the
    /// builder looks up the widget's rect via the input coordinator and
    /// uses it as the anchor.
    pub fn anchor_to(mut self, widget_id: &'a str) -> Self {
        self.anchor_widget_id = Some(widget_id);
        self
    }

    /// Backdrop fill behind the popup (default `None`).
    pub fn backdrop(mut self, b: BackdropKind) -> Self { self.backdrop = b; self }

    /// Sizing mode (`AutoFit` measures content, `Fixed(w,h)` pins).
    pub fn size_mode(mut self, m: SizeMode) -> Self { self.size_mode = m; self }

    /// Body overflow strategy (default `Clip`).
    pub fn overflow(mut self, m: OverflowMode) -> Self { self.overflow = m; self }

    /// Override visual settings (default `PopupSettings::default()`).
    pub fn settings(mut self, s: PopupSettings) -> Self { self.settings = Some(s); self }

    /// Override render kind (default `PopupRenderKind::Plain`).
    pub fn kind(mut self, k: PopupRenderKind) -> Self { self.kind = k; self }

    /// Terminal call — register and draw the popup frame.
    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<PopupNode> {
        let slot_id = self.slot_id
            .map(str::to_owned)
            .unwrap_or_else(|| self.handle.id_str().to_string());

        // Resolve anchor: explicit `.anchor(...)` wins; otherwise look up
        // the widget rect via coord using `.anchor_to(id)`.
        let resolved_anchor: Option<Rect> = self.anchor.or_else(|| {
            self.anchor_widget_id.and_then(|wid| {
                layout.ctx().input.widget_rect(&uzor::types::unsafe_widget_id(wid))
            })
        });

        // If origin wasn't explicitly set, anchor it below the resolved anchor.
        let resolved_origin = if self.origin == (0.0, 0.0) {
            resolved_anchor
                .map(|a| (a.x, a.y + a.height))
                .unwrap_or(self.origin)
        } else {
            self.origin
        };

        let overlay_rect = self.overlay_rect.unwrap_or_else(|| {
            let (w, h) = match self.size_mode {
                SizeMode::Fixed(w, h) => (w, h),
                _                     => (240.0, 200.0),
            };
            Rect::new(resolved_origin.0, resolved_origin.1, w, h)
        });

        let mut view = PopupView {
            origin:    resolved_origin,
            anchor:    resolved_anchor,
            backdrop:  self.backdrop,
            kind:      PopupViewKind::Plain,
            size_mode: self.size_mode,
            overflow:  self.overflow,
        };

        let settings = self.settings.unwrap_or_default();

        register_layout_manager_popup(
            layout,
            render,
            self.parent,
            &slot_id,
            self.handle,
            overlay_rect,
            resolved_anchor,
            &mut view,
            &settings,
            self.kind,
        )
    }
}
