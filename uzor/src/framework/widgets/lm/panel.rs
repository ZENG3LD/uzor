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
use crate::docking::panels::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, PanelNode};
use crate::render::RenderContext;
use crate::ui::widgets::composite::panel::input::register_layout_manager_panel;
use crate::ui::widgets::composite::panel::settings::PanelSettings;
use crate::ui::widgets::composite::panel::state::PanelState;
use crate::ui::widgets::composite::panel::types::{
    ColumnDef, HeaderAction, PanelHeader, PanelRenderKind, PanelView,
};

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
    settings:       Option<PanelSettings>,
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
            settings:       None,
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

    pub fn settings(mut self, s: PanelSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: PanelRenderKind) -> Self { self.kind = k; self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<PanelNode> {
        let state = self.state.expect("PanelBuilder: .state(...) is required");

        let mut view = PanelView {
            header: self.header_title.map(|title| PanelHeader {
                title,
                actions: self.header_actions,
            }),
            columns:        self.columns,
            show_scrollbar: self.show_scrollbar,
            content_height: self.content_height,
        };

        let settings = self.settings.unwrap_or_default();

        register_layout_manager_panel(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.widget_id,
            state,
            &mut view,
            &settings,
            &self.kind,
        )
    }
}

// Suppress unused-import warning when `Rect` isn't referenced in this file.
#[allow(dead_code)]
const _: Option<Rect> = None;
