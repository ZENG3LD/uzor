//! `SidebarBuilder` — chainable wrapper around
//! `register_layout_manager_sidebar`.
//!
//! Sidebars are anchored to **edge slots** like toolbars.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_sidebar("left-panel");
//! lm::sidebar(&h, "left-panel")
//!     .header_title("Project")
//!     .content_height(800.0)
//!     .build(&mut layout, &mut render);
//! ```

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, SidebarHandle, SidebarNode};
use crate::render::RenderContext;
use crate::types::OverflowMode;
use crate::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use crate::ui::widgets::composite::sidebar::settings::SidebarSettings;
use crate::ui::widgets::composite::sidebar::style::{DefaultSidebarStyle, SidebarStyle};
use crate::ui::widgets::composite::sidebar::theme::SidebarTheme;
use crate::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarHeaderMode, SidebarRenderKind, SidebarTab, SidebarView,
};
use crate::tokens::{Tokens, TokenTheme};

/// Builds `SidebarSettings` from the layout's live token set (H1 Brief 9,
/// §3 "StyleManager — removed") — replaces the pre-H1 `StyledSidebarTheme`
/// bridge, which hand-copied a handful of `StyleManager` keys and fell back
/// to a dark `TokenTheme` for the rest.
fn sidebar_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> SidebarSettings {
    SidebarSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::<DefaultSidebarStyle>::default(),
    }
}

/// Chainable builder for an edge-anchored sidebar.
pub struct SidebarBuilder<'a> {
    handle:         &'a SidebarHandle,
    slot_id:        &'a str,
    parent:         LayoutNodeId,
    header_icon:    Option<&'a crate::types::IconId>,
    header_title:   &'a str,
    header_actions: &'a [HeaderAction<'a>],
    header_mode:    SidebarHeaderMode,
    tabs:           &'a [SidebarTab<'a>],
    active_tab:     Option<&'a str>,
    show_scrollbar: bool,
    content_height: f64,
    overflow:       OverflowMode,
    settings:       Option<SidebarSettings>,
    /// Override only the colour-token bundle.  Wins over the
    /// token-set-derived default but loses to a full
    /// `.settings(...)` call.
    theme_override: Option<Box<dyn SidebarTheme>>,
    /// Override only the geometry bundle.  Same precedence rules as
    /// `theme_override`.
    style_override: Option<Box<dyn SidebarStyle>>,
    kind:           SidebarRenderKind,
}

/// Entry point: start a `SidebarBuilder` for the given handle + edge slot.
pub fn sidebar<'a>(handle: &'a SidebarHandle, slot_id: &'a str) -> SidebarBuilder<'a> {
    SidebarBuilder::new(handle, slot_id)
}

impl<'a> SidebarBuilder<'a> {
    pub fn new(handle: &'a SidebarHandle, slot_id: &'a str) -> Self {
        Self {
            handle,
            slot_id,
            parent:         LayoutNodeId::ROOT,
            header_icon:    None,
            header_title:   "",
            header_actions: &[],
            header_mode:    SidebarHeaderMode::default(),
            tabs:           &[],
            active_tab:     None,
            show_scrollbar: false,
            content_height: 0.0,
            overflow:       OverflowMode::Clip,
            settings:       None,
            theme_override: None,
            style_override: None,
            kind:           SidebarRenderKind::Left,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    pub fn header_icon(mut self, icon: &'a crate::types::IconId) -> Self {
        self.header_icon = Some(icon); self
    }
    pub fn header_title(mut self, t: &'a str) -> Self { self.header_title = t; self }
    pub fn header_actions(mut self, a: &'a [HeaderAction<'a>]) -> Self {
        self.header_actions = a; self
    }
    pub fn header_mode(mut self, m: SidebarHeaderMode) -> Self { self.header_mode = m; self }

    /// Tabs for `WithTypeSelector` kind.
    pub fn tabs(mut self, ts: &'a [SidebarTab<'a>]) -> Self { self.tabs = ts; self }
    pub fn active_tab(mut self, id: &'a str) -> Self { self.active_tab = Some(id); self }

    pub fn show_scrollbar(mut self, on: bool) -> Self { self.show_scrollbar = on; self }
    pub fn content_height(mut self, h: f64) -> Self { self.content_height = h; self }
    pub fn overflow(mut self, m: OverflowMode) -> Self { self.overflow = m; self }

    pub fn settings(mut self, s: SidebarSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: SidebarRenderKind) -> Self { self.kind = k; self }

    /// Override only the sidebar theme (colour tokens).  Useful for
    /// per-sidebar accents without forking the whole `SidebarSettings`.
    pub fn theme(mut self, t: Box<dyn SidebarTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the sidebar style (geometry — header height, default
    /// width, scrollbar width, padding …).
    pub fn style(mut self, s: Box<dyn SidebarStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<SidebarNode> {
        self.build_with_body(layout, render, |_, _, _: Rect| {})
    }

    /// Same as [`build`] but lets the caller paint the sidebar body
    /// inside the composite's body rect.
    ///
    /// `body` runs *after* the sidebar chrome (background, header,
    /// tab strip, scrollbar) is drawn, with the renderer already
    /// clipped to the body rect and the overflow transform applied:
    ///
    /// - [`OverflowMode::Scrollbar`] — body is translated by
    ///   `-scroll.offset` for the active tab; caller draws as if
    ///   scrolled to top.
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
    ) -> Option<SidebarNode>
    where
        P: DockPanel,
        F: FnOnce(&mut LayoutManager<P>, &mut dyn RenderContext, Rect),
    {
        let mut view = SidebarView {
            header: SidebarHeader {
                icon:    self.header_icon,
                title:   self.header_title,
                actions: self.header_actions,
            },
            header_mode:    self.header_mode,
            tabs:           self.tabs,
            active_tab:     self.active_tab,
            show_scrollbar: self.show_scrollbar,
            content_height: self.content_height,
            overflow:       self.overflow,
        };

        // Resolve settings: explicit `.settings(...)` wins outright,
        // otherwise build from the layout's live token set and then patch
        // in any `.theme(...)` / `.style(...)` overrides.
        let mut settings = self.settings.unwrap_or_else(|| sidebar_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

        // Snapshot scroll offset for the active tab before `state` is
        // moved into the composite registration call.
        let panel_key = self.active_tab.unwrap_or("default").to_string();
        let scroll_off = layout
            .sidebars_map_mut()
            .get(&self.handle.id)
            .and_then(|s| s.scroll_per_panel.get(panel_key.as_str()))
            .map(|s| s.offset)
            .unwrap_or(0.0);

        let node = register_layout_manager_sidebar(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.handle,
            &mut view,
            &settings,
            &self.kind,
        );

        // Resolve the sidebar's frame rect from the edge slot map and
        // paint the body.
        let frame = layout
            .rect_for_edge_slot(self.slot_id)
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        if frame.width > 0.0 && frame.height > 0.0 {
            // Reuse composite's body_viewport — header/tab/scrollbar excluded.
            // We need a SidebarState reference for the call; fetch read-only.
            let body_rect = if let Some(state) = layout.sidebars_map_mut().get(&self.handle.id) {
                let vp = crate::ui::widgets::composite::sidebar::render::body_viewport(
                    frame, state, &view, &settings, &self.kind,
                );
                vp.clip_rect
            } else {
                frame
            };

            render.save();
            render.clip_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height);
            match self.overflow {
                OverflowMode::Scrollbar => {
                    render.translate(0.0, -scroll_off);
                }
                OverflowMode::Compress => {
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
