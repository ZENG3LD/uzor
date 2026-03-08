//! Panel application trait
//!
//! The core trait that panel crates implement to become autonomous applications
//! within the terminal.

use uzor_core::render::RenderContext;
use crate::types::{PanelRect, PanelInput, PanelTheme};
use crate::toolbar::PanelToolbarDef;

// Re-export Any for downcast support
pub use std::any::Any;

/// Position where this panel wants its toolbar rendered
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarPosition {
    /// Toolbar at the top of the panel (below header)
    #[default]
    Top,
    /// Toolbar on the left side of the panel
    Left,
    /// Toolbar on the right side of the panel
    Right,
    /// Toolbar at the bottom of the panel
    Bottom,
}

/// Panel application trait — makes a panel crate an autonomous application.
///
/// The terminal orchestrator calls these methods to:
/// 1. Query what toolbar the panel wants (`toolbar_def`)
/// 2. Let the panel render its toolbar (`render_toolbar`)
/// 3. Let the panel render its content (`render_content`)
/// 4. Route clicks/actions to the panel (`handle_toolbar_click`, `handle_action`)
///
/// # Rendering model
///
/// The panel receives a `RenderContext` already translated to its origin (0, 0 = top-left
/// of the panel's allocated rect). The panel renders within `(0, 0, width, height)`.
///
/// If the panel has a toolbar, the terminal carves out space based on `toolbar_def().size`
/// and calls `render_toolbar` for the toolbar area and `render_content` for the remaining area.
///
/// # Lifecycle
///
/// ```text
/// Terminal creates panel → panel.toolbar_def() → terminal allocates toolbar space
/// Each frame:
///   terminal calls panel.render_toolbar(ctx, toolbar_rect, theme, input)
///   terminal calls panel.render_content(ctx, content_rect, input)
/// On user click in toolbar:
///   terminal calls panel.handle_toolbar_click(item_id)
/// ```
pub trait PanelApp {
    /// Panel metadata: display title
    fn title(&self) -> &str;

    /// Panel type identifier (e.g., "chart", "map", "dom", "watchlist")
    fn type_id(&self) -> &'static str;

    /// Minimum panel dimensions (width, height)
    fn min_size(&self) -> (f64, f64) { (200.0, 200.0) }

    /// Toolbar definition — what toolbar items this panel wants.
    /// Return None if this panel has no toolbar.
    fn toolbar_def(&self) -> Option<PanelToolbarDef> { None }

    /// Where should the toolbar be positioned relative to panel content?
    fn toolbar_position(&self) -> ToolbarPosition { ToolbarPosition::Top }

    /// Render the panel's local toolbar.
    ///
    /// Called by the terminal orchestrator each frame if `toolbar_def()` returns Some.
    /// The `ctx` is already translated so (0, 0) is the top-left of the toolbar area.
    /// The panel should render within `(0, 0, rect.width, rect.height)`.
    ///
    /// Returns hit zones for toolbar items.
    fn render_toolbar(
        &self,
        ctx: &mut dyn RenderContext,
        rect: PanelRect,
        theme: &PanelTheme,
        input: &PanelInput,
    ) -> Vec<crate::types::HitZone> {
        let _ = (ctx, rect, theme, input);
        Vec::new()
    }

    /// Render the panel's main content.
    ///
    /// Called by the terminal orchestrator each frame.
    /// The `ctx` is already translated so (0, 0) is the top-left of the content area
    /// (below/beside the toolbar if one exists).
    ///
    /// The default implementation is a no-op. Panels that use an extended
    /// `RenderContext` (e.g., the chart panel, which needs coordinate-conversion
    /// methods) override rendering via a concrete method and leave this as a no-op,
    /// using `as_any_mut` to downcast when the orchestrator needs to call them.
    fn render_content(
        &mut self,
        ctx: &mut dyn RenderContext,
        rect: PanelRect,
        input: &PanelInput,
    ) {
        let _ = (ctx, rect, input);
    }

    /// Return a mutable `Any` reference to `self`.
    ///
    /// Override this in every concrete type so callers can downcast a
    /// `&mut dyn PanelApp` back to the concrete type:
    ///
    /// ```ignore
    /// if let Some(chart) = panel.as_any_mut().downcast_mut::<ChartPanelApp>() {
    ///     chart.render_chart_content(ctx, area);
    /// }
    /// ```
    ///
    /// The default implementation panics to make missing overrides visible at
    /// runtime rather than silently doing nothing.
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!("PanelApp::as_any_mut not overridden for panel type_id: {}", PanelApp::type_id(self))
    }

    /// Handle a click on a toolbar item.
    ///
    /// Called when the user clicks a hit zone returned by `render_toolbar`.
    /// The panel should update its internal state accordingly.
    ///
    /// Returns an optional action string that the terminal should handle
    /// (e.g., "open_modal:symbol_search" for actions that need terminal-level UI).
    fn handle_toolbar_click(&mut self, item_id: &str) -> Option<String> {
        let _ = item_id;
        None
    }

    /// Handle a dropdown item selection.
    ///
    /// Called when the user selects an item from a dropdown menu in the toolbar.
    /// `dropdown_id` is the parent dropdown, `item_id` is the selected item.
    fn handle_dropdown_select(&mut self, dropdown_id: &str, item_id: &str) -> Option<String> {
        let _ = (dropdown_id, item_id);
        None
    }

    /// Whether this panel supports being grouped with others sharing a toolbar.
    /// When true, the terminal may merge this panel's toolbar with siblings in a branch.
    fn supports_toolbar_grouping(&self) -> bool { false }
}
