//! `ContextMenuBuilder` — chainable wrapper around
//! `register_layout_manager_context_menu`.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_context_menu("ctx-menu");
//! lm::context_menu(&h)
//!     .items(&items)
//!     .origin((mx, my))
//!     .build(&mut layout, &mut render);
//! ```

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{ContextMenuHandle, ContextMenuNode, LayoutManager, LayoutNodeId};
use crate::render::RenderContext;
use crate::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu;
use crate::ui::widgets::composite::context_menu::settings::ContextMenuSettings;
use crate::ui::widgets::composite::context_menu::style::{ContextMenuStyle, DefaultContextMenuStyle};
use crate::ui::widgets::composite::context_menu::theme::ContextMenuTheme;
use crate::tokens::{Tokens, TokenTheme};
use crate::ui::widgets::composite::context_menu::types::{
    ContextMenuItem, ContextMenuRenderKind, ContextMenuView,
};

/// Builds `ContextMenuSettings` from the layout's live token set (H1 Brief
/// 9, §3 "StyleManager — removed") — replaces the pre-H1
/// `StyledContextMenuTheme` bridge, which hand-copied a handful of
/// `StyleManager` keys and fell back to a dark `TokenTheme` for the rest.
fn context_menu_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> ContextMenuSettings {
    ContextMenuSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::<DefaultContextMenuStyle>::default(),
    }
}

/// Chainable builder for a context menu overlay.
pub struct ContextMenuBuilder<'a> {
    handle:           &'a ContextMenuHandle,
    parent:           LayoutNodeId,
    slot_id:          Option<&'a str>,
    overlay_rect:     Option<Rect>,
    anchor:           Option<Rect>,
    anchor_widget_id: Option<&'a str>,
    origin:           (f64, f64),
    items:            &'a [ContextMenuItem<'a>],
    target_id:        Option<&'a str>,
    title:            Option<&'a str>,
    settings:         Option<ContextMenuSettings>,
    /// Override only the colour-token bundle.
    theme_override:   Option<Box<dyn ContextMenuTheme>>,
    /// Override only the geometry bundle.
    style_override:   Option<Box<dyn ContextMenuStyle>>,
    kind:             ContextMenuRenderKind<'a>,
}

/// Entry point: start a `ContextMenuBuilder`.
pub fn context_menu<'a>(handle: &'a ContextMenuHandle) -> ContextMenuBuilder<'a> {
    ContextMenuBuilder::new(handle)
}

impl<'a> ContextMenuBuilder<'a> {
    pub fn new(handle: &'a ContextMenuHandle) -> Self {
        Self {
            handle,
            parent:           LayoutNodeId::ROOT,
            slot_id:          None,
            overlay_rect:     None,
            anchor:           None,
            anchor_widget_id: None,
            origin:           (0.0, 0.0),
            items:            &[],
            target_id:        None,
            title:            None,
            settings:         None,
            theme_override:   None,
            style_override:   None,
            kind:             ContextMenuRenderKind::Default,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn slot_id(mut self, s: &'a str) -> Self { self.slot_id = Some(s); self }
    pub fn origin(mut self, o: (f64, f64)) -> Self { self.origin = o; self }
    pub fn rect(mut self, r: Rect) -> Self { self.overlay_rect = Some(r); self }
    pub fn anchor(mut self, r: Rect) -> Self { self.anchor = Some(r); self }

    /// Auto-anchor to a registered widget by id — at `.build()` time the
    /// builder looks up the widget's rect via the input coordinator.
    pub fn anchor_to(mut self, widget_id: &'a str) -> Self {
        self.anchor_widget_id = Some(widget_id);
        self
    }
    pub fn items(mut self, items: &'a [ContextMenuItem<'a>]) -> Self { self.items = items; self }
    pub fn target_id(mut self, id: &'a str) -> Self { self.target_id = Some(id); self }
    pub fn title(mut self, t: &'a str) -> Self { self.title = Some(t); self }
    pub fn settings(mut self, s: ContextMenuSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: ContextMenuRenderKind<'a>) -> Self { self.kind = k; self }

    /// Override only the context-menu theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ContextMenuTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the context-menu style (geometry — row height, padding,
    /// separator inset …).
    pub fn style(mut self, s: Box<dyn ContextMenuStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<ContextMenuNode> {
        let slot_id = self.slot_id
            .map(str::to_owned)
            .unwrap_or_else(|| self.handle.id_str().to_string());

        let resolved_anchor: Option<Rect> = self.anchor.or_else(|| {
            self.anchor_widget_id.and_then(|wid| {
                layout.ctx().input.widget_rect(&crate::types::unsafe_widget_id(wid))
            })
        });

        // Default size based on kind: Default (180x ~item_h*N), Minimal (160x ...).
        // We approximate with a fixed (180, 200) — composite re-measures internally.
        let overlay_rect = self.overlay_rect.unwrap_or_else(|| {
            let (x, y) = if self.origin == (0.0, 0.0) {
                resolved_anchor
                    .map(|a| (a.x, a.y + a.height))
                    .unwrap_or(self.origin)
            } else {
                self.origin
            };
            Rect::new(x, y, 180.0, 240.0)
        });

        let mut view = ContextMenuView {
            items:     self.items,
            target_id: self.target_id,
            title:     self.title,
        };

        let mut settings = self.settings.unwrap_or_else(|| context_menu_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

        register_layout_manager_context_menu(
            layout,
            render,
            self.parent,
            &slot_id,
            self.handle,
            overlay_rect,
            resolved_anchor,
            &mut view,
            &settings,
            &self.kind,
        )
    }
}
