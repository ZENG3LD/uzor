//! Chrome type definitions — per-frame view data, hit enum, render kind.
//!
//! Ported from `mylittlechart/crates/chart-app-vello/src/chrome.rs` audit
//! in `docs/uzor-widget-research/chrome-deep.md`.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::ChromeSettings;

// ---------------------------------------------------------------------------
// ChromeTabConfig
// ---------------------------------------------------------------------------

/// Per-frame descriptor for one tab in the chrome strip.
///
/// Lifetime `'a` ties the label and icon strings to the frame.
pub struct ChromeTabConfig<'a> {
    /// Stable string id returned in events.
    pub id: &'a str,
    /// Display label text.
    pub label: &'a str,
    /// Optional icon name / path.
    pub icon: Option<&'a str>,
    /// Optional colour tag drawn as a small dot or stripe.
    pub color_tag: Option<u32>,
    /// Whether the close-X button is shown on this tab.
    pub closable: bool,
    /// Whether this is the currently active tab (bottom accent line).
    pub active: bool,
}

// ---------------------------------------------------------------------------
// ChromeHit
// ---------------------------------------------------------------------------

/// Which zone of the chrome strip the pointer is over.
///
/// Used for both hit-test results and tooltip routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChromeHit {
    /// Pointer is outside the chrome strip or over an inert area.
    #[default]
    None,
    /// Tab body (select).  Payload = tab index in the `tabs` slice.
    Tab(usize),
    /// Close-X on a tab.  Payload = tab index.
    CloseTab(usize),
    /// "+" new-tab button.
    NewTab,
    /// Gear / hamburger menu button.
    Menu,
    /// Caption / drag zone (between tabs and window controls).
    Drag,
    /// Minimize button.
    MinBtn,
    /// Maximize / restore button.
    MaxBtn,
    /// Close-app button (red hover).
    CloseBtn,
    /// Resize corner.  Payload = `ResizeCorner` enum.
    ResizeCorner(ResizeCorner),
    /// Resize edge (top).
    ResizeTop,
    /// Resize edge (bottom).
    ResizeBottom,
    /// Resize edge (left).
    ResizeLeft,
    /// Resize edge (right).
    ResizeRight,
    /// New-window icon button.
    NewWindowBtn,
    /// Close-window icon button.
    CloseWindowBtn,
}

/// Which corner of the frame is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// ---------------------------------------------------------------------------
// ChromeView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_input_coordinator_chrome` /
/// `register_context_manager_chrome`.
pub struct ChromeView<'a> {
    /// Ordered tab configs, left-to-right.
    pub tabs: &'a [ChromeTabConfig<'a>],
    /// Which tab id is active (bottom accent line).
    pub active_tab_id: Option<&'a str>,
    /// Whether to render the "+" new-tab button after the last tab.
    pub show_new_tab_btn: bool,
    /// Whether to render the gear / menu button on the right.
    pub show_menu_btn: bool,
    /// Whether to render the new-window icon button (left of menu).
    pub show_new_window_btn: bool,
    /// Whether to render the close-window icon button (left of min/max/close).
    pub show_close_window_btn: bool,
    /// Whether the window is currently maximized (changes max icon to restore).
    pub is_maximized: bool,
    /// When `true` and `show_menu_btn` is also `true`, the menu / hamburger
    /// button is drawn at the LEFT edge of the chrome strip (≈36 px wide)
    /// instead of in the right button cluster.  The drag zone then begins
    /// after the left-side menu button.  Default: `false` (right cluster).
    pub menu_left: bool,
    /// When `false` the maximize / restore button is hidden; minimize shifts
    /// one slot left (`w - 92`) and close stays at `w - 46`.
    /// Default: `true` (maximize button visible).
    pub show_maximize: bool,
    /// Current cursor X (logical px, window-relative) — for tooltip update.
    pub cursor_x: f64,
    /// Current cursor Y (logical px, window-relative) — for tooltip update.
    pub cursor_y: f64,
    /// Elapsed time in ms (for tooltip delay).
    pub time_ms: f64,
}

// ---------------------------------------------------------------------------
// ChromeLayoutConfig
// ---------------------------------------------------------------------------

/// The layout-affecting subset of [`ChromeView`], captured at register time.
///
/// The window-host press path (`LayoutManager::handle_chrome_press`) runs on
/// pointer-down to decide drag / minimize / maximize / close / resize.  It must
/// hit-test the SAME button layout that was actually drawn this frame — but it
/// has no access to the caller's per-frame `ChromeView` (which is borrowed and
/// gone by the time the press arrives).  So `register_layout_manager_chrome`
/// stores these flags in `ChromeState`, and the press path rebuilds an
/// equivalent view from them instead of guessing a default layout.
///
/// Without this the host computed button positions for a fixed default cluster
/// (menu-right + new/close-window + maximize) while the app drew a different one
/// → clicks on the drawn minimize hit the host's maximize zone, the drag zone
/// was offset, and the menu press was swallowed as a window drag.
#[derive(Debug, Clone, Copy)]
pub struct ChromeLayoutConfig {
    pub show_new_tab_btn: bool,
    pub show_menu_btn: bool,
    pub show_new_window_btn: bool,
    pub show_close_window_btn: bool,
    pub menu_left: bool,
    pub show_maximize: bool,
}

impl Default for ChromeLayoutConfig {
    /// Matches the legacy hard-coded host view so any path that hit-tests
    /// before chrome is first registered behaves exactly as before.
    fn default() -> Self {
        Self {
            show_new_tab_btn: false,
            show_menu_btn: false,
            show_new_window_btn: true,
            show_close_window_btn: true,
            menu_left: false,
            show_maximize: true,
        }
    }
}

impl ChromeLayoutConfig {
    /// Capture the layout-affecting flags from a per-frame view.
    pub fn from_view(view: &ChromeView<'_>) -> Self {
        Self {
            show_new_tab_btn: view.show_new_tab_btn,
            show_menu_btn: view.show_menu_btn,
            show_new_window_btn: view.show_new_window_btn,
            show_close_window_btn: view.show_close_window_btn,
            menu_left: view.menu_left,
            show_maximize: view.show_maximize,
        }
    }
}

impl<'a> ChromeView<'a> {
    /// Minimal constructor with sensible defaults.
    pub fn new(tabs: &'a [ChromeTabConfig<'a>]) -> Self {
        Self {
            tabs,
            active_tab_id: None,
            show_new_tab_btn: true,
            show_menu_btn: true,
            show_new_window_btn: true,
            show_close_window_btn: true,
            is_maximized: false,
            menu_left: false,
            show_maximize: true,
            cursor_x: 0.0,
            cursor_y: 0.0,
            time_ms: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ChromeRenderKind
// ---------------------------------------------------------------------------

/// Selects which layout pipeline the Chrome composite runs.
///
/// ```text
/// Default            — tabs + drag zone + menu + window controls
/// Minimal            — tab strip only (no window controls)
/// WindowControlsOnly — min/max/close only (no tabs)
/// Custom(closure)    — caller drives every draw call
/// ```
pub enum ChromeRenderKind {
    /// Full chrome: tabs + drag region + menu button + window controls.
    Default,
    /// Tab strip only — no window controls.  Used for embedded / child windows.
    Minimal,
    /// Window controls only (min / max / close).  No tabs, no menu.
    WindowControlsOnly,
    /// Escape hatch — caller drives every draw call.
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ChromeView<'_>, &ChromeSettings)>),
}

// ---------------------------------------------------------------------------
// ChromeButton  (kept for callers that used the old API)
// ---------------------------------------------------------------------------

/// Which titlebar button was interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromeButton {
    Min,
    Max,
    Close,
    NewTab,
    /// Gear / hamburger menu.
    Menu,
    NewWindow,
    CloseWindow,
}

// ---------------------------------------------------------------------------
// ChromeColors
// ---------------------------------------------------------------------------

/// All colour tokens used by the Chrome composite.
///
/// Values are ARGB `u32` (same convention as the rest of uzor).
#[derive(Debug, Clone)]
pub struct ChromeColors {
    /// Titlebar background strip.
    pub titlebar_bg:   u32,
    /// Button hover background (most buttons).
    pub button_hover:  u32,
    /// Frame border (bottom separator + dividers).
    pub border:        u32,
    /// Primary text / label colour.
    pub text:          u32,
    /// Icon in normal state.
    pub icon_normal:   u32,
    /// Icon on hover / active.
    pub icon_hover:    u32,
    /// Close-app button hover background (red).
    pub close_hover:   u32,
    /// Divider lines between button groups.
    pub separator:     u32,
    /// Active tab bottom accent line (2 px blue).
    pub tab_accent:    u32,
    /// Tooltip background.
    pub tooltip_bg:    u32,
    /// Tooltip text.
    pub tooltip_text:  u32,
}

impl Default for ChromeColors {
    fn default() -> Self {
        Self {
            titlebar_bg:   0xFF131722,
            button_hover:  0xFF1F2937,
            border:        0xFF313244,
            text:          0xFFCDD6F4,
            icon_normal:   0xFFA6ADC8,
            icon_hover:    0xFFCDD6F4,
            close_hover:   0xFFE81123,
            separator:     0xFF313244,
            tab_accent:    0xFF3B82F6,
            tooltip_bg:    0xFF323232,
            tooltip_text:  0xFFFFFFFF,
        }
    }
}

// ---------------------------------------------------------------------------
// ChromeResponse
// ---------------------------------------------------------------------------

/// Events produced by the Chrome composite during a frame.
#[derive(Debug, Clone, Default)]
pub struct ChromeResponse {
    /// Tab id clicked (tab body, not close button).
    pub tab_selected: Option<String>,
    /// Tab id whose close-X was clicked.
    pub tab_close_clicked: Option<String>,
    /// "+" button clicked — caller should create a new tab.
    pub new_tab: bool,
    /// New-window icon clicked.
    pub new_window: bool,
    /// Gear / menu clicked.
    pub menu_open: bool,
    /// Drag gesture began on the caption zone.
    pub window_drag_start: bool,
    /// Minimize button clicked.
    pub minimize: bool,
    /// Maximize / restore button clicked (toggle).
    pub maximize_restore: bool,
    /// Close-app button clicked.
    pub close_app: bool,
    /// Close-window icon clicked.
    pub close_window: bool,
    /// Resize edge currently active (if any).
    pub resize_edge: Option<ChromeHit>,
}

// ---------------------------------------------------------------------------
// ChromeAction  (emitted by handle_chrome_action)
// ---------------------------------------------------------------------------

/// High-level action emitted by `handle_chrome_action`.
#[derive(Debug, Clone, PartialEq)]
pub enum ChromeAction {
    SelectTab(usize),
    CloseTab(usize),
    NewTab,
    NewWindow,
    OpenMenu,
    WindowDragStart,
    Minimize,
    MaximizeRestore,
    CloseApp,
    CloseWindow,
    BeginResize(ChromeHit),
    None,
}
