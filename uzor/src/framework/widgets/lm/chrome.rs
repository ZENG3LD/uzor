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

use crate::layout::docking::DockPanel;
use crate::layout::{ChromeNode, LayoutManager, LayoutNodeId};
use crate::render::RenderContext;
use crate::ui::widgets::composite::chrome::input::register_layout_manager_chrome;
use crate::ui::widgets::composite::chrome::settings::ChromeSettings;
use crate::ui::widgets::composite::chrome::style::{ChromeStyle, DefaultChromeStyle};
use crate::ui::widgets::composite::chrome::theme::ChromeTheme;
use crate::ui::widgets::composite::chrome::types::{
    ChromeRenderKind, ChromeTabConfig, ChromeView,
};
use crate::tokens::{Tokens, TokenTheme};

/// Builds `ChromeSettings` from the layout's live token set (H1 Brief 9, §3
/// "StyleManager — removed") — replaces the pre-H1 `StyledChromeTheme`
/// bridge, which hand-copied a handful of `StyleManager` keys and fell back
/// to a dark `TokenTheme` for the rest. `TokenTheme` covers every
/// `ChromeTheme` method uniformly from whatever token set is currently
/// active.
fn chrome_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> ChromeSettings {
    ChromeSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
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
    /// When `true`, the menu button is placed at the LEFT edge instead of the
    /// right button cluster.  Requires `show_menu_btn(true)` to be visible.
    menu_left:            bool,
    /// When `false`, the maximize / restore button is hidden and minimize
    /// shifts left by one slot.  Default: `true`.
    show_maximize:        bool,
    cursor_x:             f64,
    cursor_y:             f64,
    time_ms:              f64,
    settings:             Option<ChromeSettings>,
    /// Override only the colour-token bundle.
    theme_override:       Option<Box<dyn ChromeTheme>>,
    /// Override only the geometry bundle.
    style_override:       Option<Box<dyn ChromeStyle>>,
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
            menu_left:             false,
            show_maximize:         true,
            cursor_x:              0.0,
            cursor_y:              0.0,
            time_ms:               0.0,
            settings:              None,
            theme_override:        None,
            style_override:        None,
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
    /// Place the menu / hamburger button at the LEFT edge of the chrome strip
    /// instead of in the right button cluster.  Has no effect if
    /// `show_menu_btn` is `false`.
    pub fn menu_left(mut self, on: bool) -> Self { self.menu_left = on; self }
    /// Show or hide the maximize / restore button.  When `false`, minimize
    /// shifts left and close stays at `w - 46`.  Default: `true`.
    pub fn show_maximize(mut self, on: bool) -> Self { self.show_maximize = on; self }

    /// Cursor position (logical px, window-relative) — for tooltip update.
    pub fn cursor(mut self, pos: (f64, f64)) -> Self {
        self.cursor_x = pos.0; self.cursor_y = pos.1; self
    }
    pub fn time_ms(mut self, t: f64) -> Self { self.time_ms = t; self }

    pub fn settings(mut self, s: ChromeSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: ChromeRenderKind) -> Self { self.kind = k; self }

    /// Override only the chrome theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ChromeTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the chrome style (geometry — strip height, button size,
    /// drag-zone offset …).
    pub fn style(mut self, s: Box<dyn ChromeStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

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
            menu_left:             self.menu_left,
            show_maximize:         self.show_maximize,
            cursor_x:              cx,
            cursor_y:              cy,
            time_ms,
        };

        let mut settings = self.settings.unwrap_or_else(|| chrome_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

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
