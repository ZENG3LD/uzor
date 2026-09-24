//! `ToolbarBuilder` — chainable wrapper around
//! `register_layout_manager_toolbar`.
//!
//! Toolbars are anchored to **edge slots** (top / bottom / left / right of the
//! dock area), not to a free overlay rect.  The `slot_id` arg is the edge
//! slot id added via `EdgePanels::add(...)` during `App::init`.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_toolbar("top-toolbar");
//! lm::toolbar(&h, "top-toolbar")
//!     .start(&top_items)
//!     .end(&clock_items)
//!     .build(&mut layout, &mut render);
//! ```

use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, ResizeEdge, ToolbarHandle, ToolbarNode};
use crate::render::RenderContext;
use crate::types::OverflowMode;
use crate::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar;
use crate::ui::widgets::composite::toolbar::settings::ToolbarSettings;
use crate::ui::widgets::composite::toolbar::style::{DefaultToolbarStyle, ToolbarStyle};
use crate::ui::widgets::composite::toolbar::theme::ToolbarTheme;
use crate::ui::widgets::composite::toolbar::types::{
    ChromeStripView, ToolbarItem, ToolbarRenderKind, ToolbarSection, ToolbarView,
};
use crate::tokens::{Tokens, TokenTheme};

/// Builds `ToolbarSettings` from the layout's live token set (H1 Brief 9,
/// §3 "StyleManager — removed") — replaces the pre-H1 `StyledToolbarTheme`
/// bridge, which hand-copied a handful of `StyleManager` keys and fell back
/// to a dark `TokenTheme` for the rest.
fn toolbar_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> ToolbarSettings {
    ToolbarSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::<DefaultToolbarStyle>::default(),
    }
}

/// Chainable builder for an edge-anchored toolbar.
pub struct ToolbarBuilder<'a> {
    handle:      &'a ToolbarHandle,
    slot_id:     &'a str,
    parent:      LayoutNodeId,
    start:       &'a [ToolbarItem<'a>],
    center:      &'a [ToolbarItem<'a>],
    end:         &'a [ToolbarItem<'a>],
    chrome:      Option<ChromeStripView<'a>>,
    overflow:    OverflowMode,
    resize_edge: Option<ResizeEdge>,
    settings:    Option<ToolbarSettings>,
    /// Override only the colour-token bundle.
    theme_override: Option<Box<dyn ToolbarTheme>>,
    /// Override only the geometry bundle.
    style_override: Option<Box<dyn ToolbarStyle>>,
    kind:        ToolbarRenderKind,
}

/// Entry point: start a `ToolbarBuilder` for the given handle + edge slot.
pub fn toolbar<'a>(handle: &'a ToolbarHandle, slot_id: &'a str) -> ToolbarBuilder<'a> {
    ToolbarBuilder::new(handle, slot_id)
}

impl<'a> ToolbarBuilder<'a> {
    pub fn new(handle: &'a ToolbarHandle, slot_id: &'a str) -> Self {
        Self {
            handle,
            slot_id,
            parent:      LayoutNodeId::ROOT,
            start:       &[],
            center:      &[],
            end:         &[],
            chrome:      None,
            overflow:    OverflowMode::Clip,
            resize_edge: None,
            settings:    None,
            theme_override: None,
            style_override: None,
            kind:        ToolbarRenderKind::Horizontal,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Items in the start (left/top) section.
    pub fn start(mut self, items: &'a [ToolbarItem<'a>]) -> Self { self.start = items; self }
    /// Items in the center section.
    pub fn center(mut self, items: &'a [ToolbarItem<'a>]) -> Self { self.center = items; self }
    /// Items in the end (right/bottom) section.
    pub fn end(mut self, items: &'a [ToolbarItem<'a>]) -> Self { self.end = items; self }

    /// ChromeStrip-specific data (only used by `ChromeStrip` kind).
    pub fn chrome(mut self, c: ChromeStripView<'a>) -> Self { self.chrome = Some(c); self }

    /// Item overflow strategy (default `Clip`).
    pub fn overflow(mut self, m: OverflowMode) -> Self { self.overflow = m; self }

    /// Which edge exposes a resize handle (default `None`).
    pub fn resize_edge(mut self, e: ResizeEdge) -> Self { self.resize_edge = Some(e); self }

    pub fn settings(mut self, s: ToolbarSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: ToolbarRenderKind) -> Self { self.kind = k; self }

    /// Override only the toolbar theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ToolbarTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the toolbar style (geometry — height, gaps, padding …).
    pub fn style(mut self, s: Box<dyn ToolbarStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<ToolbarNode> {
        let view = ToolbarView {
            start:       ToolbarSection { items: self.start },
            center:      ToolbarSection { items: self.center },
            end:         ToolbarSection { items: self.end },
            chrome:      self.chrome,
            overflow:    self.overflow,
            resize_edge: self.resize_edge,
        };

        let mut settings = self.settings.unwrap_or_else(|| toolbar_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

        register_layout_manager_toolbar(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.handle,
            &view,
            &settings,
            &self.kind,
        )
    }
}
