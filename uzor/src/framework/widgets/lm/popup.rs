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

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, PopupHandle, PopupNode, StyleManager};
use crate::render::RenderContext;
use crate::types::{OverflowMode, SizeMode};
use crate::ui::widgets::composite::popup::input::register_layout_manager_popup;
use crate::ui::widgets::composite::popup::settings::PopupSettings;
use crate::ui::widgets::composite::popup::style::DefaultPopupStyle;
use crate::ui::widgets::composite::popup::theme::{DefaultPopupTheme, PopupTheme};
use crate::ui::widgets::composite::popup::types::{
    BackdropKind, PopupRenderKind, PopupView, PopupViewKind,
};

// =============================================================================
// StyledPopupTheme — reads bg/fg/accent from StyleManager, delegates rest
// =============================================================================

struct StyledPopupTheme {
    bg:                String,
    border:            String,
    item_bg_hover:     String,
    item_bg_selected:  String,
    item_text:         String,
    accent:            String,
    fallback:          DefaultPopupTheme,
}

impl StyledPopupTheme {
    fn from_styles(s: &StyleManager) -> Self {
        let accent     = s.color_or_owned("accent",    "#2962ff");
        let accent_dim = s.color_or_owned("accent_dim","rgba(41,98,255,0.15)");
        Self {
            bg:               s.color_or_owned("surface",       "#1e222d"),
            border:           s.color_or_owned("border_strong", "#363a45"),
            item_bg_hover:    s.color_or_owned("surface_raised","#2a2e39"),
            item_bg_selected: accent_dim,
            item_text:        s.color_or_owned("fg_1",          "#d1d4dc"),
            accent:           accent,
            fallback:         DefaultPopupTheme,
        }
    }
}

impl PopupTheme for StyledPopupTheme {
    fn bg(&self)                     -> &str { &self.bg }
    fn border(&self)                 -> &str { &self.border }
    fn shadow(&self)                 -> &str { self.fallback.shadow() }
    fn item_bg_normal(&self)         -> &str { self.fallback.item_bg_normal() }
    fn item_bg_hover(&self)          -> &str { &self.item_bg_hover }
    fn item_bg_selected(&self)       -> &str { &self.item_bg_selected }
    fn item_text(&self)              -> &str { &self.item_text }
    fn item_text_hover(&self)        -> &str { self.fallback.item_text_hover() }
    fn item_text_disabled(&self)     -> &str { self.fallback.item_text_disabled() }
    fn item_text_danger(&self)       -> &str { self.fallback.item_text_danger() }
    fn item_bg_danger_hover(&self)   -> &str { self.fallback.item_bg_danger_hover() }
    fn header_text(&self)            -> &str { self.fallback.header_text() }
    fn separator(&self)              -> &str { &self.border }
    fn hex_input_bg(&self)           -> &str { self.fallback.hex_input_bg() }
    fn hex_input_text(&self)         -> &str { self.fallback.hex_input_text() }
    fn hex_input_border_focus(&self) -> &str { &self.accent }
    fn hsv_indicator(&self)          -> &str { self.fallback.hsv_indicator() }
    fn accent(&self)                 -> &str { &self.accent }
    fn backdrop_dim(&self)           -> &str { self.fallback.backdrop_dim() }
}

fn popup_settings_from_styles(s: &StyleManager) -> PopupSettings {
    PopupSettings {
        theme: Box::new(StyledPopupTheme::from_styles(s)),
        style: Box::<DefaultPopupStyle>::default(),
    }
}

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
                layout.ctx().input.widget_rect(&crate::types::unsafe_widget_id(wid))
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

        let settings = self.settings.unwrap_or_else(|| popup_settings_from_styles(layout.styles()));

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
