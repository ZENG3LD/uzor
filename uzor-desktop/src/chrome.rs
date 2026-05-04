//! Default custom-window-chrome integration helper.
//!
//! Wraps `uzor::ui::widgets::composite::chrome::register_layout_manager_chrome`
//! with sensible defaults. Apps using `decorated: false` can call
//! [`register_chrome_default`] inside their `App::ui` callback to get a working
//! titlebar in a single line.

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
/// `ChromeRenderKind::Default` pipeline.
pub fn register_chrome_default<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    view: &ChromeView<'_>,
) -> Option<ChromeNode> {
    register_chrome_with_settings(
        layout,
        render,
        view,
        &ChromeSettings::default(),
        &ChromeRenderKind::Default,
    )
}

/// Register and draw chrome with explicit settings and render kind.
pub fn register_chrome_with_settings<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    view: &ChromeView<'_>,
    settings: &ChromeSettings,
    kind: &ChromeRenderKind,
) -> Option<ChromeNode> {
    register_layout_manager_chrome(layout, render, LayoutNodeId::ROOT, "chrome", view, settings, kind)
}

// ---------------------------------------------------------------------------
// Window event integration
// ---------------------------------------------------------------------------

/// Handle a left-mouse-button-down event for the chrome strip.
///
/// Returns `true` when the event was consumed by a chrome action.
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
            true
        }
        _ => false,
    }
}
