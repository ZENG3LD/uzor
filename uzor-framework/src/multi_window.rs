//! Multi-window primitives for the L4 framework.
//!
//! L4 apps can run with one or many top-level windows.  Shared state lives
//! on the user's `App` struct; per-window UI runs through `App::ui` for
//! each open window in turn.  Spawn / close requests are queued by the app
//! and drained by the runtime between frames (mlc pattern — winit forbids
//! creating new windows from inside `window_event`).

use uzor::core::types::Rect;
use uzor::layout::LayoutManager;
use uzor::docking::panels::DockPanel;
use uzor_render_hub::WindowRenderState;
use uzor_window_hub::RgbaIcon;

/// Stable, app-supplied tag identifying a window across sessions.
///
/// Apps use this to remember "which window is which" — e.g. `WindowKey::new("main")`
/// for the dashboard and `WindowKey::new("settings")` for the settings dialog.
/// Two windows must not share the same key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct WindowKey(pub String);

impl WindowKey {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl From<&str> for WindowKey {
    fn from(s: &str) -> Self { Self(s.to_string()) }
}
impl From<String> for WindowKey {
    fn from(s: String) -> Self { Self(s) }
}

/// Declarative description of a window the runtime should create.
///
/// Apps either:
/// - Pre-register one or more windows on the builder via `.window(spec)`;
/// - Or push new specs at runtime via `App::take_pending_spawn`.
#[derive(Debug, Clone)]
pub struct WindowSpec {
    pub key:         WindowKey,
    pub title:       String,
    pub size:        (u32, u32),
    pub min_size:    Option<(u32, u32)>,
    pub decorations: bool,
    pub background:  u32,
    pub icon:        Option<RgbaIcon>,
}

impl WindowSpec {
    /// Minimal spec — borderless window with the given key + title.
    pub fn new(key: impl Into<WindowKey>, title: impl Into<String>) -> Self {
        Self {
            key:         key.into(),
            title:       title.into(),
            size:        (1280, 800),
            min_size:    Some((400, 300)),
            decorations: false,
            background:  0xFF_FF_FF_FF,
            icon:        None,
        }
    }

    pub fn size(mut self, w: u32, h: u32) -> Self { self.size = (w, h); self }
    pub fn min_size(mut self, w: u32, h: u32) -> Self { self.min_size = Some((w, h)); self }
    pub fn decorations(mut self, on: bool) -> Self { self.decorations = on; self }
    pub fn background(mut self, argb: u32) -> Self { self.background = argb; self }
    pub fn icon(mut self, icon: RgbaIcon) -> Self { self.icon = Some(icon); self }
}

/// Per-window context handed to `App::ui` for each open window in turn.
///
/// The app reads `key` to decide what to draw (a settings dialog vs. the
/// main dashboard) and writes into `layout` / `render` exactly as in the
/// single-window flow.
pub struct WindowCtx<'a, P: DockPanel> {
    pub key:    &'a WindowKey,
    pub layout: &'a mut LayoutManager<P>,
    pub render: &'a mut WindowRenderState,
    /// Window content rect in window-local coordinates (origin = `(0, 0)`).
    pub rect:   Rect,
}
