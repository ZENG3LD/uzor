//! Default custom-window-chrome integration helper.
//!
//! Wraps `uzor::ui::widgets::composite::chrome::register_layout_manager_chrome`
//! with sensible defaults. Apps using `decorated: false` can call
//! [`register_chrome_default`] inside their `App::ui` callback to get a working
//! titlebar in a single line.
//!
//! # Example
//!
//! ```no_run
//! use uzor::docking::panels::DockPanel;
//! use uzor::layout::LayoutManager;
//! use uzor::render::RenderContext;
//! use uzor::ui::widgets::composite::chrome::{ChromeAction, ChromeState, ChromeView};
//! use uzor_framework::chrome::register_chrome_default;
//! use uzor_framework::app::NoPanel;
//!
//! fn ui(
//!     layout: &mut LayoutManager<NoPanel>,
//!     render: &mut dyn RenderContext,
//!     state:  &mut ChromeState,
//! ) {
//!     let view = ChromeView::new(&[]);
//!     register_chrome_default(layout, render, state, &view);
//! }
//! ```

use uzor::docking::panels::DockPanel;
use uzor::layout::{ChromeNode, LayoutManager, LayoutNodeId};
use uzor::render::RenderContext;
use uzor::ui::widgets::composite::chrome::{
    register_layout_manager_chrome, ChromeRenderKind, ChromeSettings, ChromeState, ChromeView,
};

/// Register and draw the default chrome titlebar in one call.
///
/// This is a convenience wrapper around
/// `uzor::ui::widgets::composite::chrome::register_layout_manager_chrome`
/// using the default [`ChromeSettings`] (dark theme, 32 px height) and the
/// `ChromeRenderKind::Default` pipeline (tabs + drag zone + menu + window
/// controls).
///
/// The function resolves the chrome rect from the already-solved `LayoutManager`
/// (`layout.rect_for_chrome()`). Returns `None` when the chrome slot has not
/// been solved yet (e.g. before the first `layout.solve()` call) or when the
/// chrome strip is hidden.
///
/// # Caller responsibilities
///
/// - Set `layout.chrome_mut().visible = true` and optionally adjust
///   `layout.chrome_mut().height` before the first `layout.solve(viewport)`.
/// - Query `state.hovered` / call `handle_chrome_action` each frame to react to
///   drag, minimize, maximize, and close actions.
///
/// # Arguments
///
/// * `layout`  — the app's layout manager (must have been solved this frame).
/// * `render`  — a mutable `dyn RenderContext` (backend-specific draw context).
/// * `state`   — persistent per-frame chrome state (hover, click, tooltips …).
/// * `view`    — per-frame descriptor (tabs, active tab, cursor position …).
pub fn register_chrome_default<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    state: &mut ChromeState,
    view: &ChromeView<'_>,
) -> Option<ChromeNode> {
    register_chrome_with_settings(
        layout,
        render,
        state,
        view,
        &ChromeSettings::default(),
        &ChromeRenderKind::Default,
    )
}

/// Register and draw chrome with explicit settings and render kind.
///
/// Same as [`register_chrome_default`] but lets the caller supply custom
/// [`ChromeSettings`] (colours, height) and a [`ChromeRenderKind`]
/// (e.g. `WindowControlsOnly` or `Custom`).
///
/// Returns the registered [`WidgetId`], or `None` when the chrome rect is not
/// yet available from the layout solver.
pub fn register_chrome_with_settings<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    state: &mut ChromeState,
    view: &ChromeView<'_>,
    settings: &ChromeSettings,
    kind: &ChromeRenderKind,
) -> Option<ChromeNode> {
    register_layout_manager_chrome(layout, render, LayoutNodeId::ROOT, "chrome", state, view, settings, kind)
}
