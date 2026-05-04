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

use crate::docking::panels::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, SidebarHandle, SidebarNode, StyleManager};
use crate::render::RenderContext;
use crate::types::OverflowMode;
use crate::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use crate::ui::widgets::composite::sidebar::settings::SidebarSettings;
use crate::ui::widgets::composite::sidebar::style::DefaultSidebarStyle;
use crate::ui::widgets::composite::sidebar::theme::{DefaultSidebarTheme, SidebarTheme};
use crate::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarHeaderMode, SidebarRenderKind, SidebarTab, SidebarView,
};

// =============================================================================
// StyledSidebarTheme
// =============================================================================

struct StyledSidebarTheme {
    bg:          String,
    border:      String,
    header_text: String,
    tab_accent:  String,
    tab_bg_active: String,
    fallback:    DefaultSidebarTheme,
}

impl StyledSidebarTheme {
    fn from_styles(s: &StyleManager) -> Self {
        let accent     = s.color_or_owned("accent",    "#2962ff");
        let accent_dim = s.color_or_owned("accent_dim","rgba(41,98,255,0.12)");
        Self {
            bg:            s.color_or_owned("surface",      "#1e222d"),
            border:        s.color_or_owned("border_strong","#363a45"),
            header_text:   s.color_or_owned("fg_0",         "#ffffff"),
            tab_accent:    accent,
            tab_bg_active: accent_dim,
            fallback:      DefaultSidebarTheme,
        }
    }
}

impl SidebarTheme for StyledSidebarTheme {
    fn bg(&self)                      -> &str { &self.bg }
    fn border(&self)                  -> &str { &self.border }
    fn header_bg(&self)               -> &str { &self.bg }
    fn header_text(&self)             -> &str { &self.header_text }
    fn header_icon(&self)             -> &str { self.fallback.header_icon() }
    fn divider(&self)                 -> &str { &self.border }
    fn action_icon_normal(&self)      -> &str { self.fallback.action_icon_normal() }
    fn action_icon_hover(&self)       -> &str { self.fallback.action_icon_hover() }
    fn scrollbar_thumb(&self)         -> &str { self.fallback.scrollbar_thumb() }
    fn scrollbar_thumb_active(&self)  -> &str { self.fallback.scrollbar_thumb_active() }
    fn tab_text_active(&self)         -> &str { &self.header_text }
    fn tab_text_inactive(&self)       -> &str { self.fallback.tab_text_inactive() }
    fn tab_accent(&self)              -> &str { &self.tab_accent }
    fn tab_bg_active(&self)           -> &str { &self.tab_bg_active }
    fn tab_bg_hover(&self)            -> &str { self.fallback.tab_bg_hover() }
}

fn sidebar_settings_from_styles(s: &StyleManager) -> SidebarSettings {
    SidebarSettings {
        theme: Box::new(StyledSidebarTheme::from_styles(s)),
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

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<SidebarNode> {
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

        let settings = self.settings.unwrap_or_else(|| sidebar_settings_from_styles(layout.styles()));

        register_layout_manager_sidebar(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.handle,
            &mut view,
            &settings,
            &self.kind,
        )
    }
}
