//! `BlackboxBuilder` — chainable wrapper around
//! `register_layout_manager_blackbox_panel`.
//!
//! A blackbox panel is a free-form region whose paint + input handling is
//! supplied by a `BlackboxView` (closures or a `BlackboxHandler` impl wired
//! through closures).  L4 apps use this when they need a custom-painted
//! region inside the layout (chat panes, IDE editors, render previews).
//!
//! Usage:
//! ```ignore
//! let view = BlackboxView { ... };
//! lm::blackbox("chat-leaf", "chat")
//!     .state(&mut bb_state)
//!     .view(&mut view)
//!     .build(&mut layout, &mut render);
//! ```

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::input::core::coordinator::LayerId;
use uzor::layout::{BlackboxPanelNode, LayoutManager, LayoutNodeId};
use uzor::render::RenderContext;
use uzor::types::WidgetId;
use uzor::ui::widgets::composite::blackbox_panel::input::{
    register_layout_manager_blackbox_panel, register_layout_manager_stub_panel,
};
use uzor::ui::widgets::composite::blackbox_panel::settings::BlackboxPanelSettings;
use uzor::ui::widgets::composite::blackbox_panel::state::BlackboxState;
use uzor::ui::widgets::composite::blackbox_panel::types::{BlackboxRenderKind, BlackboxView};

/// Chainable builder for a blackbox panel.
pub struct BlackboxBuilder<'a> {
    slot_id:    &'a str,
    widget_id:  &'a str,
    parent:     LayoutNodeId,
    state:      Option<&'a mut BlackboxState>,
    view:       Option<&'a mut BlackboxView<'a>>,
    settings:   Option<BlackboxPanelSettings>,
    kind:       BlackboxRenderKind,
}

/// Entry point: start a `BlackboxBuilder`.
pub fn blackbox<'a>(slot_id: &'a str, widget_id: &'a str) -> BlackboxBuilder<'a> {
    BlackboxBuilder::new(slot_id, widget_id)
}

impl<'a> BlackboxBuilder<'a> {
    pub fn new(slot_id: &'a str, widget_id: &'a str) -> Self {
        Self {
            slot_id,
            widget_id,
            parent:   LayoutNodeId::ROOT,
            state:    None,
            view:     None,
            settings: None,
            kind:     BlackboxRenderKind::Default,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Required — frame-scoped state for the blackbox.
    pub fn state(mut self, s: &'a mut BlackboxState) -> Self { self.state = Some(s); self }

    /// Required — frame-scoped view (paint / event closures).
    pub fn view(mut self, v: &'a mut BlackboxView<'a>) -> Self { self.view = Some(v); self }

    pub fn settings(mut self, s: BlackboxPanelSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: BlackboxRenderKind) -> Self { self.kind = k; self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<BlackboxPanelNode> {
        let state    = self.state.expect("BlackboxBuilder: .state(...) is required");
        let view     = self.view.expect("BlackboxBuilder: .view(...) is required");
        let settings = self.settings.unwrap_or_default();

        register_layout_manager_blackbox_panel(
            layout,
            render,
            self.parent,
            self.slot_id,
            self.widget_id,
            state,
            view,
            &settings,
            &self.kind,
        )
    }
}

// ---------------------------------------------------------------------------
// stub_panel — non-blackbox, render-only composite
// ---------------------------------------------------------------------------

/// Register a stub panel (no event handling, render-only).
///
/// Use for dock-leaf panels with their own custom render that don't need a
/// `BlackboxView` body closure.  Wraps `register_layout_manager_stub_panel`.
pub fn stub_panel<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    widget_id: &str,
    rect: Rect,
) -> WidgetId {
    register_layout_manager_stub_panel(layout, widget_id, rect, &LayerId::main())
}
