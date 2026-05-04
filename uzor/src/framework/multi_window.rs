//! Multi-window primitives for the L4 framework.

use crate::core::types::Rect;
use crate::layout::LayoutManager;
use crate::docking::panels::DockPanel;
use super::builder::RgbaIcon;
use crate::platform::types::CornerStyle;

/// Stable, app-supplied tag identifying a window across sessions.
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
#[derive(Debug, Clone)]
pub struct WindowSpec {
    pub key:          WindowKey,
    pub title:        String,
    pub size:         (u32, u32),
    pub min_size:     Option<(u32, u32)>,
    pub decorations:  bool,
    pub background:   u32,
    pub icon:         Option<RgbaIcon>,
    /// Preferred OS corner-rounding style. Default: let the OS decide.
    pub corner_style: CornerStyle,
    /// Override the OS border accent colour (`0x00RRGGBB` ARGB). `None` = OS default.
    pub border_color: Option<u32>,
    /// Override the OS drop-shadow. `None` = OS default.
    pub shadow:       Option<bool>,
}

impl WindowSpec {
    pub fn new(key: impl Into<WindowKey>, title: impl Into<String>) -> Self {
        Self {
            key:          key.into(),
            title:        title.into(),
            size:         (1280, 800),
            min_size:     Some((400, 300)),
            decorations:  false,
            background:   0xFF_FF_FF_FF,
            icon:         None,
            corner_style: CornerStyle::Default,
            border_color: None,
            shadow:       None,
        }
    }

    pub fn size(mut self, w: u32, h: u32) -> Self { self.size = (w, h); self }
    pub fn min_size(mut self, w: u32, h: u32) -> Self { self.min_size = Some((w, h)); self }
    pub fn decorations(mut self, on: bool) -> Self { self.decorations = on; self }
    pub fn background(mut self, argb: u32) -> Self { self.background = argb; self }
    pub fn icon(mut self, icon: RgbaIcon) -> Self { self.icon = Some(icon); self }

    /// Set the preferred corner-rounding style.
    pub fn corner_style(mut self, style: CornerStyle) -> Self {
        self.corner_style = style;
        self
    }

    /// Override the OS border accent colour (`0x00RRGGBB`). Pass `None` to clear.
    pub fn border_color(mut self, color: u32) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Override the OS drop-shadow.
    pub fn shadow(mut self, on: bool) -> Self {
        self.shadow = Some(on);
        self
    }
}

/// Per-window context handed to `App::ui` for each open window in turn.
pub struct WindowCtx<'a, P: DockPanel> {
    pub key:    &'a WindowKey,
    pub layout: &'a mut LayoutManager<P>,
    pub render: &'a mut dyn crate::render::RenderContext,
    /// Window content rect in window-local coordinates.
    pub rect:   Rect,
}
