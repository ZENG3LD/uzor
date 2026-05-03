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

use winit::window::Window;

use uzor::docking::panels::DockPanel;
use uzor::layout::{ChromeNode, LayoutManager, LayoutNodeId};
use uzor::render::RenderContext;
use uzor::ui::widgets::composite::chrome::{
    chrome_hit_test, handle_chrome_action, register_layout_manager_chrome, ChromeAction,
    ChromeRenderKind, ChromeSettings, ChromeState, ChromeView,
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

// ---------------------------------------------------------------------------
// Window event integration
// ---------------------------------------------------------------------------

/// Handle a left-mouse-button-down event for the chrome strip (caption drag,
/// minimize, maximize/restore, close).
///
/// Must be called **before** the WinitInputBridge processes the event so that
/// `drag_window()` is called while the button is still held (winit requires
/// this).
///
/// Returns `true` when the event was consumed by a chrome action (the caller
/// should `return` from the event handler and not forward the event further).
///
/// # Parameters
///
/// - `layout`   — solved layout for this frame.
/// - `state`    — mutable chrome state.
/// - `view`     — per-frame chrome view (tabs, cursor position, …).
/// - `settings` — chrome style settings (or `&ChromeSettings::default()`).
/// - `kind`     — render kind (or `&ChromeRenderKind::Default`).
/// - `window`   — winit window reference used to issue window commands.
/// - `mx`, `my` — pointer position in logical pixels.
pub fn handle_chrome_window_event<P: DockPanel>(
    layout:   &LayoutManager<P>,
    state:    &ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
    window:   &Window,
    mx:       f64,
    my:       f64,
) -> bool {
    let Some(chrome_rect) = layout.rect_for_chrome() else { return false };
    let hit    = chrome_hit_test(state, view, settings, kind, chrome_rect, (mx, my));
    let action = handle_chrome_action(hit);
    match action {
        ChromeAction::WindowDragStart => {
            let _ = window.drag_window();
            true
        }
        ChromeAction::Minimize => {
            window.set_minimized(true);
            true
        }
        ChromeAction::MaximizeRestore => {
            window.set_maximized(!window.is_maximized());
            true
        }
        ChromeAction::CloseApp => {
            // Caller must handle the exit flag; we return true so the event
            // is consumed and the caller can set its own exit_requested flag.
            true
        }
        _ => false,
    }
}
