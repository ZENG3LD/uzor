//! Chrome type definitions.

use super::settings::ChromeSettings;
use crate::render::RenderContext;
use crate::types::Rect;

// TODO: populate after deep mlc audit
//   - config: &'a ChromeConfig
//   - active_tab: Option<&'a str>
pub struct ChromeView<'a> {
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// Render strategy for Chrome.
pub enum ChromeRenderKind {
    Default,
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ChromeView<'_>, &ChromeSettings)>),
}

use crate::ui::widgets::atomic::tab::TabConfig;

/// Which titlebar button was interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromeButton {
    Min,
    Max,
    Close,
    NewTab,
    Settings,
}

/// Color configuration for the Chrome titlebar.
#[derive(Debug, Clone)]
pub struct ChromeColors {
    pub titlebar_bg: u32,
    pub button_hover: u32,
    pub border: u32,
    pub text: u32,
}

impl Default for ChromeColors {
    fn default() -> Self {
        Self {
            titlebar_bg: 0xFF1E1E2E,
            button_hover: 0xFF313244,
            border: 0xFF45475A,
            text: 0xFFCDD6F4,
        }
    }
}

/// Configuration for the Chrome (window decoration) widget.
#[derive(Debug, Clone)]
pub struct ChromeConfig {
    /// Tabs in the strip. Order matches left-to-right display order.
    pub tabs: Vec<TabConfig>,
    /// Show the minimize button.
    pub show_min: bool,
    /// Show the maximize/restore button.
    pub show_max: bool,
    /// Show the close button.
    pub show_close: bool,
    /// Height of the titlebar in logical pixels.
    pub height: f64,
    /// Rect that acts as the window drag handle (subset of the chrome rect).
    pub drag_region_rect: Rect,
}

impl ChromeConfig {
    pub fn new(drag_region_rect: Rect) -> Self {
        Self {
            tabs: Vec::new(),
            show_min: true,
            show_max: true,
            show_close: true,
            height: 32.0,
            drag_region_rect,
        }
    }

    pub fn with_tabs(mut self, tabs: Vec<TabConfig>) -> Self {
        self.tabs = tabs;
        self
    }

    pub fn without_min(mut self) -> Self {
        self.show_min = false;
        self
    }

    pub fn without_max(mut self) -> Self {
        self.show_max = false;
        self
    }
}

/// Events produced by the Chrome widget in a frame.
#[derive(Debug, Clone, Default)]
pub struct ChromeResponse {
    /// ID of the tab that was clicked (tab body, not close button).
    pub tab_clicked: Option<String>,
    /// ID of the tab whose close button was clicked.
    pub tab_close_clicked: Option<String>,
    /// Which titlebar button was clicked, if any.
    pub button_clicked: Option<ChromeButton>,
}
