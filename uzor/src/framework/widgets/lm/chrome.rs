//! `ChromeBuilder` — chainable wrapper around
//! `register_layout_manager_chrome`.
//!
//! Chrome is a **singleton** — there is at most one chrome composite per
//! window.  No handle is needed; the slot id is fixed (`"chrome"`).
//!
//! Usage:
//! ```ignore
//! lm::chrome()
//!     .tabs(&tabs)
//!     .active_tab("dashboard")
//!     .cursor((mx, my))
//!     .time_ms(now_ms)
//!     .build(&mut layout, &mut render);
//! ```

use crate::docking::panels::DockPanel;
use crate::layout::{ChromeNode, LayoutManager, LayoutNodeId, StyleManager};
use crate::render::RenderContext;
use crate::ui::widgets::composite::chrome::input::register_layout_manager_chrome;
use crate::ui::widgets::composite::chrome::settings::ChromeSettings;
use crate::ui::widgets::composite::chrome::style::DefaultChromeStyle;
use crate::ui::widgets::composite::chrome::theme::{ChromeTheme, DefaultChromeTheme};
use crate::ui::widgets::composite::chrome::types::{
    ChromeRenderKind, ChromeTabConfig, ChromeView,
};

// =============================================================================
// StyledChromeTheme — reads accent/fg from StyleManager, delegates rest
// =============================================================================

struct StyledChromeTheme {
    background:  String,
    icon_normal: String,
    icon_hover:  String,
    button_hover:String,
    tab_accent:  String,
    fallback:    DefaultChromeTheme,
}

impl StyledChromeTheme {
    fn from_styles(s: &StyleManager) -> Self {
        Self {
            background:   s.color_or_owned("surface",  "#131722"),
            icon_normal:  s.color_or_owned("fg_2",     "#a6adc8"),
            icon_hover:   s.color_or_owned("fg_0",     "#cdd6f4"),
            button_hover: s.color_or_owned("surface_raised", "#1f2937"),
            tab_accent:   s.color_or_owned("accent",   "#3b82f6"),
            fallback:     DefaultChromeTheme,
        }
    }
}

impl ChromeTheme for StyledChromeTheme {
    fn background(&self)      -> &str { &self.background }
    fn icon_normal(&self)     -> &str { &self.icon_normal }
    fn icon_hover(&self)      -> &str { &self.icon_hover }
    fn button_hover(&self)    -> &str { &self.button_hover }
    fn close_hover(&self)     -> &str { self.fallback.close_hover() }
    fn separator(&self)       -> &str { self.fallback.separator() }
    fn tab_bg_normal(&self)   -> &str { self.fallback.tab_bg_normal() }
    fn tab_bg_hover(&self)    -> &str { &self.button_hover }
    fn tab_bg_active(&self)   -> &str { self.fallback.tab_bg_active() }
    fn tab_text_normal(&self) -> &str { &self.icon_normal }
    fn tab_text_hover(&self)  -> &str { &self.icon_hover }
    fn tab_text_active(&self) -> &str { self.fallback.tab_text_active() }
    fn tab_accent(&self)      -> &str { &self.tab_accent }
    fn drag_zone_bg(&self)    -> &str { self.fallback.drag_zone_bg() }
    fn tooltip_bg(&self)      -> &str { self.fallback.tooltip_bg() }
    fn tooltip_text(&self)    -> &str { self.fallback.tooltip_text() }
}

fn chrome_settings_from_styles(s: &StyleManager) -> ChromeSettings {
    ChromeSettings {
        theme: Box::new(StyledChromeTheme::from_styles(s)),
        style: Box::<DefaultChromeStyle>::default(),
    }
}

/// Chainable builder for the singleton window chrome strip.
pub struct ChromeBuilder<'a> {
    parent:               LayoutNodeId,
    widget_id:            &'a str,
    tabs:                 &'a [ChromeTabConfig<'a>],
    active_tab_id:        Option<&'a str>,
    show_new_tab_btn:     bool,
    show_menu_btn:        bool,
    show_new_window_btn:  bool,
    show_close_window_btn:bool,
    is_maximized:         bool,
    cursor_x:             f64,
    cursor_y:             f64,
    time_ms:              f64,
    settings:             Option<ChromeSettings>,
    kind:                 ChromeRenderKind,
}

/// Entry point: start a `ChromeBuilder` with the default widget id `"chrome"`.
pub fn chrome<'a>() -> ChromeBuilder<'a> {
    ChromeBuilder::new()
}

impl<'a> ChromeBuilder<'a> {
    pub fn new() -> Self {
        Self {
            parent:                LayoutNodeId::ROOT,
            widget_id:             "chrome",
            tabs:                  &[],
            active_tab_id:         None,
            show_new_tab_btn:      false,
            show_menu_btn:         false,
            show_new_window_btn:   false,
            show_close_window_btn: false,
            is_maximized:          false,
            cursor_x:              0.0,
            cursor_y:              0.0,
            time_ms:               0.0,
            settings:              None,
            kind:                  ChromeRenderKind::Default,
        }
    }

    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn widget_id(mut self, id: &'a str) -> Self { self.widget_id = id; self }

    pub fn tabs(mut self, ts: &'a [ChromeTabConfig<'a>]) -> Self { self.tabs = ts; self }
    pub fn active_tab(mut self, id: &'a str) -> Self { self.active_tab_id = Some(id); self }

    pub fn show_new_tab_btn(mut self, on: bool) -> Self { self.show_new_tab_btn = on; self }
    pub fn show_menu_btn(mut self, on: bool) -> Self { self.show_menu_btn = on; self }
    pub fn show_new_window_btn(mut self, on: bool) -> Self { self.show_new_window_btn = on; self }
    pub fn show_close_window_btn(mut self, on: bool) -> Self { self.show_close_window_btn = on; self }
    pub fn is_maximized(mut self, on: bool) -> Self { self.is_maximized = on; self }

    /// Cursor position (logical px, window-relative) — for tooltip update.
    pub fn cursor(mut self, pos: (f64, f64)) -> Self {
        self.cursor_x = pos.0; self.cursor_y = pos.1; self
    }
    pub fn time_ms(mut self, t: f64) -> Self { self.time_ms = t; self }

    pub fn settings(mut self, s: ChromeSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: ChromeRenderKind) -> Self { self.kind = k; self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<ChromeNode> {
        // If cursor / time weren't explicitly set, read them from the
        // layout manager — runtime publishes them each frame.
        let (cx, cy) = if self.cursor_x == 0.0 && self.cursor_y == 0.0 {
            layout.cursor_pos().unwrap_or((self.cursor_x, self.cursor_y))
        } else {
            (self.cursor_x, self.cursor_y)
        };
        let time_ms = if self.time_ms == 0.0 {
            layout.frame_time_ms()
        } else {
            self.time_ms
        };

        let view = ChromeView {
            tabs:                  self.tabs,
            active_tab_id:         self.active_tab_id,
            show_new_tab_btn:      self.show_new_tab_btn,
            show_menu_btn:         self.show_menu_btn,
            show_new_window_btn:   self.show_new_window_btn,
            show_close_window_btn: self.show_close_window_btn,
            is_maximized:          self.is_maximized,
            cursor_x:              cx,
            cursor_y:              cy,
            time_ms,
        };

        let settings = self.settings.unwrap_or_else(|| chrome_settings_from_styles(layout.styles()));

        register_layout_manager_chrome(
            layout,
            render,
            self.parent,
            self.widget_id,
            &view,
            &settings,
            &self.kind,
        )
    }
}
