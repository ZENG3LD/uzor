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

use uzor::docking::panels::DockPanel;
use uzor::layout::{LayoutManager, LayoutNodeId, SidebarHandle, SidebarNode};
use uzor::render::RenderContext;
use uzor::types::OverflowMode;
use uzor::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use uzor::ui::widgets::composite::sidebar::settings::SidebarSettings;
use uzor::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarHeaderMode, SidebarRenderKind, SidebarTab, SidebarView,
};

/// Chainable builder for an edge-anchored sidebar.
pub struct SidebarBuilder<'a> {
    handle:         &'a SidebarHandle,
    slot_id:        &'a str,
    parent:         LayoutNodeId,
    header_icon:    Option<&'a uzor::types::IconId>,
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

    pub fn header_icon(mut self, icon: &'a uzor::types::IconId) -> Self {
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

        let settings = self.settings.unwrap_or_default();

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
