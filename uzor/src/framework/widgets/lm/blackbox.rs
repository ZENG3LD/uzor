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

use crate::core::types::Rect;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::layout::{BlackboxPanelNode, LayoutManager, LayoutNodeId, StyleManager};
use crate::render::RenderContext;
use crate::types::WidgetId;
use crate::ui::widgets::composite::blackbox_panel::input::{
    register_layout_manager_blackbox_panel, register_layout_manager_stub_panel,
};
use crate::ui::widgets::composite::blackbox_panel::settings::BlackboxPanelSettings;
use crate::ui::widgets::composite::blackbox_panel::state::BlackboxState;
use crate::ui::widgets::composite::blackbox_panel::style::DefaultBlackboxStyle;
use crate::ui::widgets::composite::blackbox_panel::theme::BlackboxTheme;
use crate::ui::widgets::composite::blackbox_panel::types::{BlackboxRenderKind, BlackboxView};

// =============================================================================
// StyledBlackboxTheme
// =============================================================================

struct StyledBlackboxTheme {
    bg:          String,
    border:      String,
    header_bg:   String,
    header_text: String,
}

impl StyledBlackboxTheme {
    fn from_styles(s: &StyleManager) -> Self {
        Self {
            bg:          s.color_or_owned("surface_0",  "#1a1d28"),
            border:      s.color_or_owned("border",     "#363a45"),
            header_bg:   s.color_or_owned("surface",    "#1e222d"),
            header_text: s.color_or_owned("fg_0",       "#ffffff"),
        }
    }
}

impl BlackboxTheme for StyledBlackboxTheme {
    fn bg(&self)          -> &str { &self.bg }
    fn border(&self)      -> &str { &self.border }
    fn header_bg(&self)   -> &str { &self.header_bg }
    fn header_text(&self) -> &str { &self.header_text }
    fn divider(&self)     -> &str { &self.border }
}

fn blackbox_settings_from_styles(s: &StyleManager) -> BlackboxPanelSettings {
    BlackboxPanelSettings {
        theme: Box::new(StyledBlackboxTheme::from_styles(s)),
        style: Box::<DefaultBlackboxStyle>::default(),
    }
}

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
        let settings = self.settings.unwrap_or_else(|| blackbox_settings_from_styles(layout.styles()));

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
