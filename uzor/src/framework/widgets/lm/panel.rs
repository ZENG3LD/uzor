//! `PanelBuilder` — chainable wrapper around
//! `register_layout_manager_panel`.
//!
//! Panel state is currently passed externally (not yet migrated to LM
//! ownership like modal/popup/dropdown).  Pass `&mut PanelState` to
//! `.state(...)` until the migration lands.
//!
//! Usage:
//! ```ignore
//! lm::panel("watchlist-leaf", "watchlist")
//!     .state(&mut self.watchlist_state)
//!     .header_title("Watchlist")
//!     .build(&mut layout, &mut render);
//! ```

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, PanelNode, StyleManager};
use crate::render::RenderContext;
use crate::ui::widgets::composite::panel::input::register_layout_manager_panel;
use crate::ui::widgets::composite::panel::settings::PanelSettings;
use crate::ui::widgets::composite::panel::state::PanelState;
use crate::ui::widgets::composite::panel::style::DefaultPanelStyle;
use crate::ui::widgets::composite::panel::theme::{DefaultPanelTheme, PanelTheme};
use crate::ui::widgets::composite::panel::types::{
    ColumnDef, HeaderAction, PanelHeader, PanelRenderKind, PanelView,
};

// =============================================================================
// StyledPanelTheme
// =============================================================================

struct StyledPanelTheme {
    bg:          String,
    border:      String,
    header_bg:   String,
    header_text: String,
    fallback:    DefaultPanelTheme,
}

impl StyledPanelTheme {
    fn from_styles(s: &StyleManager) -> Self {
        Self {
            bg:          s.color_or_owned("surface_0",  "#0d1117"),
            border:      s.color_or_owned("border",     "#30363d"),
            header_bg:   s.color_or_owned("surface",    "#161b22"),
            header_text: s.color_or_owned("fg_2",       "#8091a5"),
            fallback:    DefaultPanelTheme,
        }
    }
}

impl PanelTheme for StyledPanelTheme {
    fn bg(&self)                      -> &str { &self.bg }
    fn border(&self)                  -> &str { &self.border }
    fn header_bg(&self)               -> &str { &self.header_bg }
    fn header_text(&self)             -> &str { &self.header_text }
    fn column_header_bg(&self)        -> &str { &self.header_bg }
    fn column_header_text(&self)      -> &str { self.fallback.column_header_text() }
    fn row_bg_normal(&self)           -> &str { &self.bg }
    fn row_bg_hover(&self)            -> &str { self.fallback.row_bg_hover() }
    fn row_bg_selected(&self)         -> &str { self.fallback.row_bg_selected() }
    fn footer_bg(&self)               -> &str { &self.header_bg }
    fn footer_text(&self)             -> &str { self.fallback.footer_text() }
    fn divider(&self)                 -> &str { &self.border }
    fn action_icon_normal(&self)      -> &str { self.fallback.action_icon_normal() }
    fn action_icon_hover(&self)       -> &str { self.fallback.action_icon_hover() }
    fn sort_arrow_color(&self)        -> &str { self.fallback.sort_arrow_color() }
}

fn panel_settings_from_styles(s: &StyleManager) -> PanelSettings {
    PanelSettings {
        theme: Box::new(StyledPanelTheme::from_styles(s)),
        style: Box::<DefaultPanelStyle>::default(),
    }
}

/// Chainable builder for a docked content panel.
pub struct PanelBuilder<'a> {
    slot_id:        &'a str,
    widget_id:      &'a str,
    parent:         LayoutNodeId,
    state:          Option<&'a mut PanelState>,
    header_title:   Option<&'a str>,
    header_actions:&'a [HeaderAction<'a>],
    columns:        &'a [ColumnDef<'a>],
    show_scrollbar: bool,
    content_height: f64,
    content_width:  f64,
    overflow:       crate::types::OverflowMode,
    settings:       Option<PanelSettings>,
    /// Override just the colour-token bundle.  Wins over the
    /// `StyleManager`-derived default but loses to a full
    /// `.settings(...)` call.
    theme_override: Option<Box<dyn crate::ui::widgets::composite::panel::theme::PanelTheme>>,
    /// Override just the geometry bundle.  Same precedence rules as
    /// `theme_override`.
    style_override: Option<Box<dyn crate::ui::widgets::composite::panel::style::PanelStyle>>,
    kind:           PanelRenderKind,
}

/// Entry point: start a `PanelBuilder` for the given slot + widget id.
pub fn panel<'a>(slot_id: &'a str, widget_id: &'a str) -> PanelBuilder<'a> {
    PanelBuilder::new(slot_id, widget_id)
}

impl<'a> PanelBuilder<'a> {
    pub fn new(slot_id: &'a str, widget_id: &'a str) -> Self {
        Self {
            slot_id,
            widget_id,
            parent:         LayoutNodeId::ROOT,
            state:          None,
            header_title:   None,
            header_actions: &[],
            columns:        &[],
            show_scrollbar: false,
            content_height: 0.0,
            content_width:  0.0,
            overflow:       crate::types::OverflowMode::Clip,
            settings:       None,
            theme_override: None,
            style_override: None,
            kind:           PanelRenderKind::Plain,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Pass mutable state.  Required — composite has no LM-owned state yet.
    pub fn state(mut self, s: &'a mut PanelState) -> Self { self.state = Some(s); self }

    pub fn header_title(mut self, t: &'a str) -> Self { self.header_title = Some(t); self }
    pub fn header_actions(mut self, a: &'a [HeaderAction<'a>]) -> Self {
        self.header_actions = a; self
    }
    pub fn columns(mut self, cs: &'a [ColumnDef<'a>]) -> Self { self.columns = cs; self }
    pub fn show_scrollbar(mut self, on: bool) -> Self { self.show_scrollbar = on; self }
    pub fn content_height(mut self, h: f64) -> Self { self.content_height = h; self }
    pub fn content_width(mut self, w: f64) -> Self { self.content_width = w; self }
    pub fn overflow(mut self, m: crate::types::OverflowMode) -> Self { self.overflow = m; self }

    pub fn settings(mut self, s: PanelSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: PanelRenderKind) -> Self { self.kind = k; self }

    /// Override only the panel theme (colour tokens).  Useful for
    /// per-panel highlights without forking the whole `PanelSettings`.
    pub fn theme(
        mut self,
        t: Box<dyn crate::ui::widgets::composite::panel::theme::PanelTheme>,
    ) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the panel style (geometry — radius, padding,
    /// header height …).
    pub fn style(
        mut self,
        s: Box<dyn crate::ui::widgets::composite::panel::style::PanelStyle>,
    ) -> Self {
        self.style_override = Some(s);
        self
    }


    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<PanelNode> {
        self.build_with_body(layout, render, |_, _, _: Rect| {})
    }

    /// Same as [`build`] but lets the caller paint the panel body
    /// inside the composite's body rect.
    ///
    /// `body` runs *after* the panel chrome (background, header,
    /// scrollbar, footer) is drawn, with the renderer already
    /// clipped to the body rect and the overflow transform applied:
    ///
    /// - [`OverflowMode::Scrollbar`] — body is translated by
    ///   `-state.scroll.offset`; caller draws as if scrolled to top.
    /// - [`OverflowMode::Compress`] — body is uniformly scaled so
    ///   `content_height` fits the body rect height.
    /// - [`OverflowMode::Clip`] / `Chevrons` — only the clip is applied.
    ///
    /// `body` receives `(&mut LayoutManager<P>, &mut dyn RenderContext, body_rect)`
    /// so nested `lm::*` widgets work normally.
    pub fn build_with_body<P, F>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
        body: F,
    ) -> Option<PanelNode>
    where
        P: DockPanel,
        F: FnOnce(&mut LayoutManager<P>, &mut dyn RenderContext, Rect),
    {
        let state = self.state.expect("PanelBuilder: .state(...) is required");

        let mut view = PanelView {
            header: self.header_title.map(|title| PanelHeader {
                title,
                actions: self.header_actions,
            }),
            columns:        self.columns,
            show_scrollbar: self.show_scrollbar,
            content_height: self.content_height,
            content_width:  self.content_width,
            overflow:       self.overflow,
        };

        // Resolve settings: explicit `.settings(...)` wins outright,
        // otherwise build from StyleManager and then patch in any
        // `.theme(...)` / `.style(...)` overrides.
        let mut settings = self.settings.unwrap_or_else(|| panel_settings_from_styles(layout.styles()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

        // Snapshot scroll offset before `state` is moved into the
        // composite registration call.
        let scroll_off = state.scroll.offset;

        let node = register_layout_manager_panel(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.widget_id,
            state,
            &mut view,
            &settings,
            &self.kind,
        );

        // Resolve the panel's frame rect (from either the dock-leaf
        // map or an overlay registration) and paint the body.
        let frame = layout
            .rect_for(self.slot_id)
            .or_else(|| layout.rect_for_overlay(self.slot_id))
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        if frame.width > 0.0 && frame.height > 0.0 {
            let body_rect = crate::ui::widgets::composite::panel::render::body_rect(
                frame, &view, &settings, &self.kind,
            );
            render.save();
            render.clip_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height);
            match self.overflow {
                crate::types::OverflowMode::Scrollbar => {
                    render.translate(0.0, -scroll_off);
                }
                crate::types::OverflowMode::Compress => {
                    if self.content_height > body_rect.height && self.content_height > 0.0 {
                        let s = body_rect.height / self.content_height;
                        render.translate(body_rect.x, body_rect.y);
                        render.scale(s, s);
                        render.translate(-body_rect.x, -body_rect.y);
                    }
                }
                _ => {}
            }
            body(layout, render, body_rect);
            render.restore();
        }

        node
    }
}

// Suppress unused-import warning when `Rect` isn't referenced in this file.
#[allow(dead_code)]
const _: Option<Rect> = None;
