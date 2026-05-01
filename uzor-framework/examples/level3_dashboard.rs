//! # Level 3 — full uzor stack: LayoutManager + composite widgets
//!
//! Layout:
//! - Chrome titlebar (tabs: Dashboard / Charts / Settings, clock, min/max/close)
//! - Top toolbar (File/View/Help dropdowns, clock on right)
//! - Left vertical toolbar (icon buttons, sidebar toggle)
//! - Slide-out sidebar (section headers + 3 modal buttons + settings button)
//! - Main content area (dock, tab-dependent content)
//! - Modal (L2 widget set / L1 custom button / settings)
//! - Popup (hover on toolbar items)
//! - Dropdown (File / View / Help menus)
//! - Context menu (right-click in main area)
//! - Tooltip (hover atomic widgets)
//!
//! ```sh
//! cargo run --example level3_dashboard -p uzor-framework
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

// ── vello ──────────────────────────────────────────────────────────────────────
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext as VelloRenderCx, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};

// ── uzor core ─────────────────────────────────────────────────────────────────
use uzor::docking::panels::{DockPanel, SplitKind};
use uzor::input::core::coordinator::LayerId;
use uzor::input::pointer::state::{InputState, PointerState};
use uzor::input::text::store::TextFieldConfig;
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager, LayoutNodeId, OverlayEntry, OverlayKind};
use uzor::types::{Rect, WidgetId, WidgetState};

// ── composite widgets ─────────────────────────────────────────────────────────
use uzor::ui::widgets::composite::chrome::input::{
    chrome_hit_test, handle_chrome_action, register_layout_manager_chrome,
};
use uzor::ui::widgets::composite::chrome::settings::ChromeSettings;
use uzor::ui::widgets::composite::chrome::state::ChromeState;
use uzor::ui::widgets::composite::chrome::style::ChromeStyle;
use uzor::ui::widgets::composite::chrome::types::{
    ChromeAction, ChromeHit, ChromeRenderKind, ChromeTabConfig, ChromeView,
};

use uzor::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu;
use uzor::ui::widgets::composite::context_menu::settings::ContextMenuSettings;
use uzor::ui::widgets::composite::context_menu::state::ContextMenuState;
use uzor::ui::widgets::composite::context_menu::types::{
    ContextMenuItem, ContextMenuRenderKind, ContextMenuView,
};

use uzor::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown;
use uzor::ui::widgets::composite::dropdown::render::measure_flat;
use uzor::ui::widgets::composite::dropdown::settings::DropdownSettings;
use uzor::ui::widgets::composite::dropdown::state::DropdownState;
use uzor::ui::widgets::composite::dropdown::types::{
    DropdownItem, DropdownItemRight, DropdownRenderKind, DropdownView, DropdownViewKind,
};

use uzor::ui::widgets::composite::modal::input::{handle_modal_drag, register_layout_manager_modal};
use uzor::ui::widgets::composite::modal::settings::ModalSettings;
use uzor::ui::widgets::composite::modal::state::ModalState;
use uzor::ui::widgets::composite::modal::types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView,
};


use uzor::ui::widgets::composite::popup::input::register_layout_manager_popup;
use uzor::ui::widgets::composite::popup::settings::PopupSettings;
use uzor::ui::widgets::composite::popup::state::PopupState;
use uzor::ui::widgets::composite::popup::types::{
    BackdropKind as PopupBackdrop, PopupRenderKind, PopupView, PopupViewKind,
};

use uzor::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use uzor::ui::widgets::composite::sidebar::settings::SidebarSettings;
use uzor::ui::widgets::composite::sidebar::state::SidebarState;
use uzor::ui::widgets::composite::sidebar::style::{DefaultSidebarStyle, SidebarStyle};
use uzor::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarRenderKind, SidebarView,
};

use uzor::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar;
use uzor::ui::widgets::composite::toolbar::settings::ToolbarSettings;
use uzor::ui::widgets::composite::toolbar::state::ToolbarState;
use uzor::ui::widgets::composite::toolbar::style::{
    BackgroundFill as ToolbarBackgroundFill, HorizontalToolbarStyle, ToolbarStyle,
    VerticalToolbarStyle,
};
use uzor::ui::widgets::composite::toolbar::types::{
    ToolbarItem, ToolbarRenderKind, ToolbarSection, ToolbarView,
};

// ── atomic widgets (used inside modals) ──────────────────────────────────────
use uzor::ui::widgets::atomic::button::input::register_context_manager_button;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonTheme, ButtonView};

use uzor::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox;
use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::theme::CheckboxTheme;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

use uzor::ui::widgets::atomic::color_swatch::input::register_context_manager_color_swatch;
use uzor::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
use uzor::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};

use uzor::ui::widgets::atomic::drag_handle::input::register_context_manager_drag_handle;
use uzor::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
use uzor::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};

use uzor::ui::widgets::atomic::item::input::register_context_manager_item;
use uzor::ui::widgets::atomic::item::render::ItemView;
use uzor::ui::widgets::atomic::item::settings::ItemSettings;
use uzor::ui::widgets::atomic::item::style::ItemStyle;
use uzor::ui::widgets::atomic::item::types::ItemRenderKind;

use uzor::ui::widgets::atomic::radio::input::register_context_manager_radio;
use uzor::ui::widgets::atomic::radio::settings::RadioSettings;
use uzor::ui::widgets::atomic::radio::types::{DotShape, RadioDotView, RadioRenderKind};

use uzor::ui::widgets::atomic::scrollbar::input::register_context_manager_scrollbar;
use uzor::ui::widgets::atomic::scrollbar::settings::ScrollbarSettings;

use uzor::ui::widgets::atomic::separator::input::{
    register_context_manager_separator, SeparatorKind,
};
use uzor::ui::widgets::atomic::separator::render::SeparatorView;
use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};

use uzor::ui::widgets::atomic::slider::input::register_context_manager_slider;
use uzor::ui::widgets::atomic::slider::render::SliderView;
use uzor::ui::widgets::atomic::slider::settings::SliderSettings;
use uzor::ui::widgets::atomic::slider::types::{DualSliderHandle, SliderType};

use uzor::ui::widgets::atomic::tab::input::register_context_manager_tab;
use uzor::ui::widgets::atomic::tab::render::TabView;
use uzor::ui::widgets::atomic::tab::settings::TabSettings;
use uzor::ui::widgets::atomic::tab::types::TabConfig;

use uzor::ui::widgets::atomic::text_input::render::{draw_input, draw_input_cursor, InputView};
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::types::InputType;

use uzor::ui::widgets::atomic::toggle::input::register_context_manager_toggle;
use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

use uzor::render::{draw_svg_icon, RenderContext};
use uzor::ui::widgets::atomic::text::{draw_text, TextSettings};
use uzor::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use uzor::render::{TextAlign, TextBaseline};

// ── GPU render context ────────────────────────────────────────────────────────
use uzor_render_vello_gpu::VelloGpuRenderContext;

// ── winit input bridge ────────────────────────────────────────────────────────
use uzor_window_desktop::WinitInputBridge;

// =============================================================================
// Constants
// =============================================================================

const WIN_W: u32 = 1200;
const WIN_H: u32 = 800;

const BG: Color = Color::from_rgb8(0x16, 0x16, 0x1e);
const SIDEBAR_OPEN_W: f64 = 220.0;
const LEFT_VTOOLBAR_W: f64 = 44.0;
const TOP_TOOLBAR_H: f64 = 36.0;
const CHROME_H: f64 = 30.0;

// SVG icons for toolbars
const SVG_CIRCLE: &str =
    r#"<svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="9" stroke-width="2"/></svg>"#;
const SVG_SQUARE: &str =
    r#"<svg viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="18" height="18" rx="2" stroke-width="2"/></svg>"#;
const SVG_TRIANGLE: &str =
    r#"<svg viewBox="0 0 24 24" fill="none"><polyline points="12,3 22,21 2,21 12,3" stroke-width="2"/></svg>"#;
const SVG_DIAMOND: &str =
    r#"<svg viewBox="0 0 24 24" fill="none"><polyline points="12,2 22,12 12,22 2,12 12,2" stroke-width="2"/></svg>"#;

// L2 modal constants (same geometry as level2_launcher)
const L2_WIN_W: f64 = 560.0;
const L2_WIN_H: f64 = 420.0;

const BTN_RECT: Rect = Rect { x: 28.0, y: 28.0, width: 130.0, height: 36.0 };
const CLOSE_RECT: Rect = Rect { x: 278.0, y: 28.0, width: 24.0, height: 24.0 };
const CB_RECT: Rect = Rect { x: 28.0, y: 88.0, width: 160.0, height: 22.0 };
const TOG_RECT: Rect = Rect { x: 28.0, y: 130.0, width: 80.0, height: 24.0 };
const SLID_RECT: Rect = Rect { x: 28.0, y: 200.0, width: 260.0, height: 24.0 };
const RANGE_RECT: Rect = Rect { x: 28.0, y: 228.0, width: 260.0, height: 24.0 };
const TI_RECT: Rect = Rect { x: 28.0, y: 278.0, width: 200.0, height: 28.0 };

const TAB_STRIP_Y: f64 = 12.0;
const TAB_STRIP_H: f64 = 28.0;
const CONTENT_START_Y: f64 = 52.0;
const SB_W: f64 = 10.0;
const SB_H: f64 = 376.0;
const CONTENT_ROWS: usize = 20;
const ROW_H: f64 = 28.0;
const CONTENT_H: f64 = CONTENT_ROWS as f64 * ROW_H;
const SPLITTER_W: f64 = 6.0;
const LEFT_PANEL_X: f64 = 12.0;

// =============================================================================
// Text label helper — thin wrapper around draw_text for static labels.
// =============================================================================

/// Draw a static text label clipped to `rect`.
/// `align` controls horizontal position; baseline is always `Middle`.
fn label(
    render:  &mut dyn RenderContext,
    rect:    Rect,
    text:    &str,
    align:   TextAlign,
    color:   &str,
) {
    draw_text(render, rect, &TextView {
        text,
        align,
        baseline: TextBaseline::Middle,
        color:    Some(color),
        font:     None,
        overflow: TextOverflow::Clip,
        hovered:  false,
    }, &TextSettings::default());
}


// =============================================================================
// Themes (copied from level2_launcher)
// =============================================================================

struct VisibleButtonTheme;
impl ButtonTheme for VisibleButtonTheme {
    fn button_bg_normal(&self) -> &str { "#2962ff" }
    fn button_bg_hover(&self) -> &str { "#4080ff" }
    fn button_bg_pressed(&self) -> &str { "#1a40c8" }
    fn button_bg_active(&self) -> &str { "#10b981" }
    fn button_bg_disabled(&self) -> &str { "#2a2a2a" }
    fn button_text_normal(&self) -> &str { "#ffffff" }
    fn button_text_hover(&self) -> &str { "#ffffff" }
    fn button_text_active(&self) -> &str { "#ffffff" }
    fn button_text_disabled(&self) -> &str { "#4a4a4a" }
    fn button_icon_normal(&self) -> &str { "#ffffff" }
    fn button_icon_hover(&self) -> &str { "#ffffff" }
    fn button_icon_active(&self) -> &str { "#ffffff" }
    fn button_icon_disabled(&self) -> &str { "#4a4a4a" }
    fn button_border_normal(&self) -> &str { "#3a5aff" }
    fn button_border_hover(&self) -> &str { "#6080ff" }
    fn button_border_focused(&self) -> &str { "#2962ff" }
    fn button_accent(&self) -> &str { "#2962ff" }
    fn button_danger(&self) -> &str { "#ef5350" }
    fn button_success(&self) -> &str { "#10b981" }
    fn button_warning(&self) -> &str { "#f59e0b" }
    fn toolbar_item_bg_hover(&self) -> &str { "#2a2e39" }
    fn toolbar_item_bg_active(&self) -> &str { "#2196F3" }
    fn toolbar_item_text(&self) -> &str { "#d1d4dc" }
    fn toolbar_item_text_hover(&self) -> &str { "#ffffff" }
    fn toolbar_item_text_active(&self) -> &str { "#ffffff" }
    fn toolbar_separator(&self) -> &str { "#2a2e39" }
    fn toolbar_background(&self) -> &str { "#1e222d" }
    fn toolbar_accent(&self) -> &str { "#2962ff" }
    fn button_primary_bg(&self) -> &str { "#2962ff" }
    fn button_primary_bg_hover(&self) -> &str { "#4080ff" }
    fn button_danger_bg(&self) -> &str { "rgba(255,80,80,0.15)" }
    fn button_danger_bg_hover(&self) -> &str { "rgba(255,80,80,0.35)" }
    fn button_danger_border(&self) -> &str { "rgba(239,83,80,0.5)" }
    fn button_danger_border_hover(&self) -> &str { "rgba(239,83,80,0.75)" }
    fn button_danger_text(&self) -> &str { "#ef5350" }
    fn button_secondary_hover_bg(&self) -> &str { "rgba(255,255,255,0.12)" }
    fn button_secondary_text_muted(&self) -> &str { "rgba(254,255,238,0.7)" }
    fn button_secondary_text(&self) -> &str { "rgba(254,255,238,0.95)" }
    fn button_ghost_idle_bg(&self) -> &str { "#1e222d" }
    fn button_utility_bg(&self) -> &str { "#2a2e39" }
    fn button_utility_bg_hover(&self) -> &str { "#363a45" }
}

// Sidebar style: same as default but without the header divider line.
struct NoDividerSidebarStyle(DefaultSidebarStyle);
impl SidebarStyle for NoDividerSidebarStyle {
    fn header_height(&self) -> f64       { self.0.header_height() }
    fn tab_strip_height(&self) -> f64    { self.0.tab_strip_height() }
    fn padding(&self) -> f64             { self.0.padding() }
    fn resize_zone_width(&self) -> f64   { self.0.resize_zone_width() }
    fn border_width(&self) -> f64        { self.0.border_width() }
    fn min_width(&self) -> f64           { self.0.min_width() }
    fn max_width(&self) -> f64           { self.0.max_width() }
    fn default_width(&self) -> f64       { self.0.default_width() }
    fn scrollbar_width(&self) -> f64     { self.0.scrollbar_width() }
    fn show_header_divider(&self) -> bool { false }
}

// Chrome style with bottom border enabled (default: true, but explicit for clarity).
struct ChromeWithBottomBorder;
impl ChromeStyle for ChromeWithBottomBorder {
    fn show_bottom_border(&self) -> bool { true }
}

// Horizontal toolbar style with bottom edge border enabled.
struct HorizToolbarWithBorder;
impl ToolbarStyle for HorizToolbarWithBorder {
    fn height(&self)                -> f64 { HorizontalToolbarStyle.height() }
    fn width(&self)                 -> f64 { HorizontalToolbarStyle.width() }
    fn item_size(&self)             -> f64 { HorizontalToolbarStyle.item_size() }
    fn icon_size(&self)             -> f64 { HorizontalToolbarStyle.icon_size() }
    fn item_spacing(&self)          -> f64 { HorizontalToolbarStyle.item_spacing() }
    fn section_gap(&self)           -> f64 { HorizontalToolbarStyle.section_gap() }
    fn padding(&self)               -> f64 { HorizontalToolbarStyle.padding() }
    fn item_radius(&self)           -> f64 { HorizontalToolbarStyle.item_radius() }
    fn separator_thickness(&self)   -> f64 { HorizontalToolbarStyle.separator_thickness() }
    fn separator_padding(&self)     -> f64 { HorizontalToolbarStyle.separator_padding() }
    fn scroll_chevron_size(&self)   -> f64 { HorizontalToolbarStyle.scroll_chevron_size() }
    fn split_chevron_width(&self)   -> f64 { HorizontalToolbarStyle.split_chevron_width() }
    fn color_swatch_size(&self)     -> f64 { HorizontalToolbarStyle.color_swatch_size() }
    fn color_swatch_border_width(&self) -> f64 { HorizontalToolbarStyle.color_swatch_border_width() }
    fn font_size(&self)             -> f64 { HorizontalToolbarStyle.font_size() }
    fn font_size_small(&self)       -> f64 { HorizontalToolbarStyle.font_size_small() }
    fn background_fill(&self) -> ToolbarBackgroundFill { ToolbarBackgroundFill::Solid }
    fn show_edge_border(&self) -> bool { true }
}

// Vertical toolbar style with right edge border enabled.
struct VertToolbarWithBorder;
impl ToolbarStyle for VertToolbarWithBorder {
    fn height(&self)                -> f64 { VerticalToolbarStyle.height() }
    fn width(&self)                 -> f64 { VerticalToolbarStyle.width() }
    fn item_size(&self)             -> f64 { VerticalToolbarStyle.item_size() }
    fn icon_size(&self)             -> f64 { VerticalToolbarStyle.icon_size() }
    fn item_spacing(&self)          -> f64 { VerticalToolbarStyle.item_spacing() }
    fn section_gap(&self)           -> f64 { VerticalToolbarStyle.section_gap() }
    fn padding(&self)               -> f64 { VerticalToolbarStyle.padding() }
    fn item_radius(&self)           -> f64 { VerticalToolbarStyle.item_radius() }
    fn separator_thickness(&self)   -> f64 { VerticalToolbarStyle.separator_thickness() }
    fn separator_padding(&self)     -> f64 { VerticalToolbarStyle.separator_padding() }
    fn scroll_chevron_size(&self)   -> f64 { VerticalToolbarStyle.scroll_chevron_size() }
    fn split_chevron_width(&self)   -> f64 { VerticalToolbarStyle.split_chevron_width() }
    fn color_swatch_size(&self)     -> f64 { VerticalToolbarStyle.color_swatch_size() }
    fn color_swatch_border_width(&self) -> f64 { VerticalToolbarStyle.color_swatch_border_width() }
    fn font_size(&self)             -> f64 { VerticalToolbarStyle.font_size() }
    fn font_size_small(&self)       -> f64 { VerticalToolbarStyle.font_size_small() }
    fn show_edge_border(&self) -> bool { true }
}

struct VisibleCheckboxTheme;
impl CheckboxTheme for VisibleCheckboxTheme {
    fn checkbox_bg_checked(&self) -> &str { "#2962ff" }
    fn checkbox_bg_unchecked(&self) -> &str { "#1a1a1f" }
    fn checkbox_border(&self) -> &str { "#3a3a45" }
    fn checkbox_checkmark(&self) -> &str { "#ffffff" }
    fn checkbox_notification_inner(&self) -> &str { "#ffffff" }
    fn checkbox_label_text(&self) -> &str { "#d1d4dc" }
}

// L2 row styles
struct RowStyleRoboto;
impl ItemStyle for RowStyleRoboto {
    fn font(&self) -> &str { "13px Roboto" }
}
struct RowStyleJetBrains;
impl ItemStyle for RowStyleJetBrains {
    fn font(&self) -> &str { "13px JetBrainsMono" }
}
struct RowStylePtRoot;
impl ItemStyle for RowStylePtRoot {
    fn font(&self) -> &str { "13px PT-Root-UI" }
}
struct RowStyleRobotoBold;
impl ItemStyle for RowStyleRobotoBold {
    fn font(&self) -> &str { "bold 14px Roboto" }
}

// =============================================================================
// TabConfig active_if extension
// =============================================================================

trait TabConfigExt {
    fn active_if(self, cond: bool) -> Self;
}

impl TabConfigExt for TabConfig {
    fn active_if(mut self, cond: bool) -> Self {
        self.active = cond;
        self
    }
}

// =============================================================================
// PanelKind — 6 panel types
// =============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
enum PanelKind {
    Watchlist,
    Spreadsheet,
    Notes,
    Inbox,
    Tasks,
    Calendar,
}

impl PanelKind {
    fn title(&self) -> &str {
        match self {
            Self::Watchlist   => "Watchlist",
            Self::Spreadsheet => "Spreadsheet",
            Self::Notes       => "Notes",
            Self::Inbox       => "Inbox",
            Self::Tasks       => "Tasks",
            Self::Calendar    => "Calendar",
        }
    }
    fn all() -> &'static [PanelKind] {
        &[
            Self::Watchlist,
            Self::Spreadsheet,
            Self::Notes,
            Self::Inbox,
            Self::Tasks,
            Self::Calendar,
        ]
    }
}

// =============================================================================
// SplitKind selection for spawn UI
// =============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
enum SpawnSplit {
    SplitRight,
    SplitBottom,
    Grid2x2,
}

// =============================================================================
// Panel body render dispatch
// =============================================================================

fn render_panel_body(
    kind: &PanelKind,
    render: &mut dyn uzor::render::RenderContext,
    rect: uzor::types::Rect,
    watchlist_state: &watchlist_blackbox::WatchlistState,
) {
    match kind {
        PanelKind::Watchlist   => watchlist_blackbox::render(watchlist_state, render, rect),
        PanelKind::Spreadsheet => render_spreadsheet_stub(render, rect),
        PanelKind::Notes       => render_notes_stub(render, rect),
        PanelKind::Inbox       => render_inbox_stub(render, rect),
        PanelKind::Tasks       => render_tasks_stub(render, rect),
        PanelKind::Calendar    => render_calendar_stub(render, rect),
    }
}

fn render_header_strip(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect, title: &str, accent: &str) {
    render.set_fill_color(accent);
    render.fill_rect(rect.x, rect.y, rect.width, 28.0);
    label(render, Rect::new(rect.x, rect.y, rect.width, 28.0), title, TextAlign::Center, "#ffffff");
}

fn render_spreadsheet_stub(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect) {
    render.set_fill_color("#1a1f2e");
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render_header_strip(render, rect, "Spreadsheet", "#1e6b3a");

    let col_labels = ["A", "B", "C", "D", "E", "F"];
    let rows = 8_usize;
    let cols = 6_usize;
    let cell_w = (rect.width - 40.0) / cols as f64;
    let cell_h = 22.0_f64;
    let table_x = rect.x + 8.0;
    let table_y = rect.y + 36.0;

    // Header row
    for (c, lbl) in col_labels.iter().enumerate() {
        let cx = table_x + 32.0 + c as f64 * cell_w;
        render.set_fill_color("#252535");
        render.fill_rect(cx, table_y, cell_w - 1.0, cell_h - 1.0);
        label(render, Rect::new(cx, table_y, cell_w - 1.0, cell_h - 1.0), lbl, TextAlign::Center, "#a0a0b8");
    }

    // Row number + data cells
    let values = [
        ["12", "345", "67.8", "9", "100", "22"],
        ["AB", "CD", "3.14", "0", "999", "1"],
        ["7",  "88", "0.5",  "42", "5",  "77"],
        ["X",  "Y",  "Z",    "1",  "2",  "3"],
        ["4",  "56", "7890", "11", "22", "33"],
        ["AA", "BB", "CC",   "44", "55", "66"],
        ["99", "0",  "1",    "2",  "3",  "4"],
        ["55", "66", "77",   "88", "99", "0"],
    ];
    for r in 0..rows {
        let ry = table_y + cell_h + r as f64 * cell_h;
        if ry + cell_h > rect.y + rect.height { break; }
        render.set_fill_color("#242432");
        render.fill_rect(table_x, ry, 30.0, cell_h - 1.0);
        {
            let row_num = format!("{}", r + 1);
            label(render, Rect::new(table_x, ry, 30.0, cell_h - 1.0), &row_num, TextAlign::Center, "#606078");
        }
        for c in 0..cols {
            let cx = table_x + 32.0 + c as f64 * cell_w;
            render.set_fill_color(if (r + c) % 2 == 0 { "#1d2236" } else { "#1a1f2e" });
            render.fill_rect(cx, ry, cell_w - 1.0, cell_h - 1.0);
            label(render, Rect::new(cx, ry, cell_w - 1.0, cell_h - 1.0), values[r][c], TextAlign::Left, "#c8c8dc");
        }
    }
}

fn render_notes_stub(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect) {
    render.set_fill_color("#1c1c28");
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render_header_strip(render, rect, "Notes", "#4a3f6b");

    let lines = [
        "Meeting notes — 2026-05-01",
        "",
        "Discussed Q2 roadmap with team. Key items:",
        "  • Finalize connector V5 API",
        "  • Deploy dashboard update",
        "  • Review performance metrics",
        "",
        "Action items:",
        "  1. Update documentation by Friday",
        "  2. Schedule follow-up call next week",
        "  3. Review open PRs before merge",
    ];
    let mut ly = rect.y + 44.0;
    for line in &lines {
        if ly + 16.0 > rect.y + rect.height { break; }
        if line.is_empty() { ly += 8.0; continue; }
        label(render, Rect::new(rect.x + 12.0, ly, rect.width - 12.0, 18.0), line, TextAlign::Left, "#b0b8d0");
        ly += 18.0;
    }
}

fn render_inbox_stub(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect) {
    render.set_fill_color("#1a1c24");
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render_header_strip(render, rect, "Inbox", "#2a5680");

    let emails = [
        ("Alice Chen",    "Q2 budget approval",        "09:14"),
        ("Bob Martinez",  "Re: connector update",      "08:55"),
        ("Carol Wang",    "Dashboard feedback",         "Yesterday"),
        ("David Kim",     "Weekly sync agenda",         "Mon"),
        ("Eve Johnson",   "License renewal reminder",   "Sun"),
        ("Frank Liu",     "Server maintenance window",  "Fri"),
    ];
    let row_h = 42.0_f64;
    let mut ry = rect.y + 36.0;
    for (sender, subject, time) in &emails {
        if ry + row_h > rect.y + rect.height { break; }
        render.set_fill_color("rgba(255,255,255,0.04)");
        render.fill_rect(rect.x + 4.0, ry, rect.width - 8.0, row_h - 2.0);
        label(render, Rect::new(rect.x + 12.0, ry, rect.width - 60.0, 18.0), sender,  TextAlign::Left,  "#d1d4dc");
        label(render, Rect::new(rect.x + 12.0, ry + 18.0, rect.width - 60.0, 18.0), subject, TextAlign::Left,  "#8890a8");
        label(render, Rect::new(rect.x + 4.0, ry, rect.width - 8.0, 18.0),  time,    TextAlign::Right, "#606880");
        ry += row_h;
    }
}

fn render_tasks_stub(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect) {
    render.set_fill_color("#1b1f1a");
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render_header_strip(render, rect, "Tasks", "#2d6e3e");

    let tasks = [
        (true,  "Deploy connector V5",         "Apr 30"),
        (true,  "Code review: dashboard PR",   "May 1"),
        (false, "Write integration tests",     "May 2"),
        (false, "Update API documentation",    "May 3"),
        (false, "Performance benchmarks",      "May 5"),
    ];
    let row_h = 32.0_f64;
    let mut ry = rect.y + 36.0;
    for (done, task, due) in &tasks {
        if ry + row_h > rect.y + rect.height { break; }
        render.set_fill_color("rgba(255,255,255,0.03)");
        render.fill_rect(rect.x + 4.0, ry, rect.width - 8.0, row_h - 2.0);
        // Checkbox
        if *done {
            render.set_fill_color("#10b981");
            render.fill_rounded_rect(rect.x + 10.0, ry + 8.0, 14.0, 14.0, 3.0);
            label(render, Rect::new(rect.x + 10.0, ry + 8.0, 14.0, 14.0), "✓", TextAlign::Center, "#ffffff");
        } else {
            render.set_fill_color("rgba(255,255,255,0.15)");
            render.fill_rounded_rect(rect.x + 10.0, ry + 8.0, 14.0, 14.0, 3.0);
        }
        label(render, Rect::new(rect.x + 32.0, ry, rect.width - 80.0, row_h - 2.0), task, TextAlign::Left,  if *done { "#606878" } else { "#d1d4dc" });
        label(render, Rect::new(rect.x + 4.0, ry, rect.width - 8.0, row_h - 2.0),   due,  TextAlign::Right, "#606880");
        ry += row_h;
    }
}

fn render_calendar_stub(render: &mut dyn uzor::render::RenderContext, rect: uzor::types::Rect) {
    render.set_fill_color("#1c1a22");
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render_header_strip(render, rect, "Calendar — May 2026", "#5a2d6e");

    let day_headers = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cell_w = (rect.width - 16.0) / 7.0;
    let cell_h = ((rect.height - 72.0) / 6.0).min(32.0);
    let grid_x = rect.x + 8.0;
    let grid_y = rect.y + 36.0;

    // Day headers
    for (i, h) in day_headers.iter().enumerate() {
        let cx = grid_x + i as f64 * cell_w;
        label(render, Rect::new(cx, grid_y, cell_w, 20.0), h, TextAlign::Center, "#7070a0");
    }

    // Day cells — May 2026 starts on Friday (col 4)
    let start_col = 4_usize;
    let days = 31_usize;
    let today = 1_usize; // highlight day 1 as "today" for demo
    for d in 1..=days {
        let idx = d - 1 + start_col;
        let col = idx % 7;
        let row = idx / 7;
        let cx = grid_x + col as f64 * cell_w;
        let cy = grid_y + 20.0 + row as f64 * cell_h;
        if cy + cell_h > rect.y + rect.height { break; }
        let day_color = if d == today {
            render.set_fill_color("#2962ff");
            render.fill_rounded_rect(cx + 2.0, cy + 2.0, cell_w - 4.0, cell_h - 4.0, 4.0);
            "#ffffff"
        } else if col >= 5 {
            render.set_fill_color("#6a3050");
            render.fill_rect(cx + 2.0, cy + 2.0, cell_w - 4.0, cell_h - 4.0);
            "#d8a0c0"
        } else {
            "#d1d4dc"
        };
        {
            let day_str = format!("{d}");
            label(render, Rect::new(cx, cy, cell_w, cell_h), &day_str, TextAlign::Center, day_color);
        }
    }
}

// =============================================================================
// DockPanel impl
// =============================================================================

#[derive(Clone, Debug)]
struct DemoPanel {
    title: String,
    kind: PanelKind,
}

impl DockPanel for DemoPanel {
    fn title(&self) -> &str { &self.title }
    fn type_id(&self) -> &'static str { "demo-panel" }
}

// =============================================================================
// Which modal variant to open
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalKind {
    L2,
    L1,
    Settings,
    Tags,
}

// =============================================================================
// Drag target for L2 sliders
// =============================================================================

enum DragTarget {
    L2Slider(f64),
    L2RangeMin(f64),
    L2RangeMax(f64),
    L2Scroll(f64),
    L2Splitter(f64),
    /// Modal header drag — stores cursor-relative-to-modal offset so modal moves smoothly.
    ModalDrag,
    /// Dock separator drag — stores separator index and start mouse position.
    SeparatorDrag { sep_idx: usize, start_x: f64, start_y: f64 },
}

// =============================================================================
// App state
// =============================================================================

struct AppState {
    window: Arc<Window>,
    render_cx: VelloRenderCx,
    surface: RenderSurface<'static>,
    renderer: Renderer,
    scene: Scene,

    // Layout + input
    layout: LayoutManager<DemoPanel>,
    bridge: WinitInputBridge,
    start: Instant,
    last_clock_tick: Instant,
    clock_str: String,

    // Composite widget states
    chrome_state: ChromeState,
    top_toolbar_state: ToolbarState,
    left_vtoolbar_state: ToolbarState,
    sidebar_state: SidebarState,
    modal_state: ModalState,
    popup_state: PopupState,
    ctx_menu_state: ContextMenuState,
    dropdown_file_state: DropdownState,
    dropdown_view_state: DropdownState,
    dropdown_help_state: DropdownState,
    dropdown_addpanel_state: DropdownState,

    // App interaction state
    active_view: usize, // 0=Dashboard,1=Charts,2=Settings
    sidebar_open: bool,
    left_toolbar_visible: bool,
    modal_open: bool,
    modal_kind: ModalKind,

    // popup: which toolbar item is hovered
    popup_item: Option<String>,

    // L2 widget state (inside modal)
    l2_connected: bool,
    l2_checked: bool,
    l2_toggled: bool,
    l2_radio_sel: usize,
    l2_slider_val: f64,
    l2_range_min: f64,
    l2_range_max: f64,
    l2_range_drag_handle: Option<DualSliderHandle>,
    l2_scroll_off: f64,
    l2_swatch_sel: usize,
    l2_active_tab: usize,
    l2_active_sub_tab: usize,
    l2_right_panel_w: f64,
    l2_hovered: Option<String>,
    l2_pressed: Option<String>,

    // Mouse tracking
    last_mouse: (f64, f64),
    mouse_down: bool,
    drag_origin: Option<(f64, f64)>,
    drag_target: Option<DragTarget>,

    // Per-tab docking trees (3 tabs)
    tab_trees: [uzor::docking::panels::DockingTree<DemoPanel>; 3],

    // Spawn UI state
    spawn_kind: PanelKind,
    spawn_split: SpawnSplit,

    // Need-exit flag set by close button
    exit_requested: bool,

    // Fix 2: L1 custom button hover/press state
    l1_btn_hovered: bool,
    l1_btn_pressed: bool,

    // Fix 3: Watchlist blackbox state
    watchlist: watchlist_blackbox::WatchlistState,
}

impl AppState {
    fn time_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn time_ms(&self) -> f64 {
        self.start.elapsed().as_millis() as f64
    }

    fn switch_tab(&mut self, new_tab: usize) {
        if new_tab == self.active_view { return; }
        let old_tree = std::mem::take(self.layout.panels_mut().tree_mut());
        self.tab_trees[self.active_view] = old_tree;
        let new_tree = std::mem::take(&mut self.tab_trees[new_tab]);
        *self.layout.panels_mut().tree_mut() = new_tree;
        self.active_view = new_tab;
    }

    fn l2_sb_x(&self) -> f64 {
        L2_WIN_W - SB_W - 8.0
    }

    fn l2_right_panel_x(&self) -> f64 {
        L2_WIN_W - self.l2_right_panel_w
    }

    fn l2_splitter_rect(&self) -> Rect {
        let rx = self.l2_right_panel_x();
        Rect::new(rx - SPLITTER_W / 2.0, 12.0, SPLITTER_W, L2_WIN_H - 24.0)
    }

    fn l2_tab_rect(&self, i: usize) -> Rect {
        let rx = self.l2_right_panel_x();
        let rw = self.l2_right_panel_w;
        let tab_w = ((rw - 16.0) / 3.0).floor();
        let tab_x = rx + 8.0 + i as f64 * (tab_w + 4.0);
        Rect::new(tab_x, TAB_STRIP_Y, tab_w, TAB_STRIP_H)
    }

    fn l2_sub_tab_rect(&self, i: usize) -> Rect {
        let rx = self.l2_right_panel_x() + 8.0;
        Rect::new(rx, CONTENT_START_Y + 8.0 + i as f64 * 36.0, 90.0, 30.0)
    }

    fn l2_thumb_h() -> f64 {
        let ratio = (SB_H / CONTENT_H).clamp(0.0, 1.0);
        (ratio * SB_H).max(30.0)
    }

    // Hit-test inside L2 modal body (offsets relative to body_rect origin)
    fn l2_hovered_at(&self, mx: f64, my: f64) -> Option<String> {
        fn contains(r: Rect, x: f64, y: f64) -> bool {
            x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height
        }
        if contains(BTN_RECT,   mx, my) { return Some("l2-btn-connect".into()); }
        if contains(CLOSE_RECT, mx, my) { return Some("l2-btn-close".into()); }
        if contains(CB_RECT,    mx, my) { return Some("l2-cb".into()); }
        if contains(TOG_RECT,   mx, my) { return Some("l2-tog".into()); }
        if contains(SLID_RECT,  mx, my) { return Some("l2-slider".into()); }
        if contains(RANGE_RECT, mx, my) { return Some("l2-range".into()); }
        if contains(TI_RECT,    mx, my) { return Some("l2-text".into()); }
        let sp = self.l2_splitter_rect();
        let sp_hit = Rect::new(sp.x - 4.0, sp.y, sp.width + 8.0, sp.height);
        if contains(sp_hit, mx, my) { return Some("l2-splitter".into()); }
        for i in 0..3_usize {
            let cx = 28.0 + i as f64 * 40.0;
            if contains(Rect::new(cx, 175.0, 28.0, 28.0), mx, my) {
                return Some(format!("l2-radio-{i}"));
            }
        }
        for i in 0..4_usize {
            let sx = 28.0 + i as f64 * 34.0;
            if contains(Rect::new(sx, 344.0, 26.0, 26.0), mx, my) {
                return Some(format!("l2-swatch-{i}"));
            }
        }
        for i in 0..3_usize {
            if contains(self.l2_tab_rect(i), mx, my) {
                return Some(format!("l2-tab-{i}"));
            }
        }
        if self.l2_active_tab == 2 {
            for i in 0..3_usize {
                if contains(self.l2_sub_tab_rect(i), mx, my) {
                    return Some(format!("l2-sub-tab-{i}"));
                }
            }
        }
        if self.l2_active_tab == 0 {
            let sb_track = Rect::new(self.l2_sb_x(), 52.0, SB_W, SB_H);
            if contains(sb_track, mx, my) { return Some("l2-sb".into()); }
        }
        None
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };
        let win_rect = Rect::new(0.0, 0.0, width as f64, height as f64);

        // ── 1. Update clock ───────────────────────────────────────────────────
        if self.last_clock_tick.elapsed() >= Duration::from_secs(1) {
            let elapsed = self.start.elapsed().as_secs();
            let h = (elapsed / 3600) % 24;
            let m = (elapsed / 60) % 60;
            let s = elapsed % 60;
            self.clock_str = format!("{:02}:{:02}:{:02}", h, m, s);
            self.last_clock_tick = Instant::now();
        }

        // ── 2. Setup layout ───────────────────────────────────────────────────
        // Chrome (30px)
        self.layout.chrome_mut().visible = true;
        self.layout.chrome_mut().height = CHROME_H as f32;

        // Top toolbar
        self.layout.edges_mut().add(EdgeSlot {
            id: "top-toolbar".to_string(),
            side: EdgeSide::Top,
            thickness: TOP_TOOLBAR_H as f32,
            visible: true,
            order: 0,
        });

        // Left vertical toolbar (toggled via View → Show Toolbar)
        self.layout.edges_mut().add(EdgeSlot {
            id: "left-vtoolbar".to_string(),
            side: EdgeSide::Left,
            thickness: LEFT_VTOOLBAR_W as f32,
            visible: self.left_toolbar_visible,
            order: 0,
        });

        // Sidebar (slide-out, toggled): always registered, visible toggled
        self.layout.edges_mut().add(EdgeSlot {
            id: "sidebar".to_string(),
            side: EdgeSide::Left,
            thickness: SIDEBAR_OPEN_W as f32,
            visible: self.sidebar_open,
            order: 1,
        });

        // Clear overlays at the START of frame (before push_overlay calls).
        // If we cleared at the end, rect_for_overlay would return None for
        // outside-click handlers running between frames.
        self.layout.clear_overlays();

        self.layout.solve(win_rect);

        // ── 3. Build InputState ───────────────────────────────────────────────
        let (mx, my) = self.bridge.last_mouse_pos;
        let input = InputState {
            pointer: PointerState {
                pos: Some((mx, my)),
                ..PointerState::default()
            },
            time: self.time_secs(),
            ..InputState::default()
        };
        self.layout.ctx_mut().input.begin_frame(input);

        // Fix 4: register L2 text field at its actual screen-space rect.
        // Compute modal rect using the same formula as the rendering pass below
        // so the text field hit-zone matches the drawn widget.
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            let modal_w = L2_WIN_W + 24.0;
            let modal_h = L2_WIN_H + 80.0;
            let modal_x = (width as f64 / 2.0 - modal_w / 2.0).max(0.0);
            let modal_y = (height as f64 / 2.0 - modal_h / 2.0).max(0.0);
            // If the modal has been dragged, its render position differs.
            let (frame_x, frame_y) = if self.modal_state.position != (0.0, 0.0) {
                self.modal_state.position
            } else {
                (modal_x, modal_y)
            };
            // ox/oy as used in the render pass: ox = frame_x, oy = frame_y + 44.0 (header)
            let ti_screen_rect = Rect::new(
                frame_x + TI_RECT.x,
                frame_y + 44.0 + TI_RECT.y,
                TI_RECT.width,
                TI_RECT.height,
            );
            self.layout.ctx_mut().input.register_text_field(
                "l2-text",
                ti_screen_rect,
                TextFieldConfig::text(),
            );
        }

        // ── 4. Scene ──────────────────────────────────────────────────────────
        self.scene.reset();
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, BG, None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        let time_ms = self.time_ms();
        let clock = self.clock_str.clone();

        let mut render = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);
        // ── Chrome ────────────────────────────────────────────────────────────
        let tab_ids = ["tab-0", "tab-1", "tab-2"];
        let chrome_tabs = [
            ChromeTabConfig { id: "tab-0", label: "Dashboard",  icon: None, color_tag: None, closable: false, active: self.active_view == 0 },
            ChromeTabConfig { id: "tab-1", label: "Panels",     icon: None, color_tag: None, closable: false, active: self.active_view == 1 },
            ChromeTabConfig { id: "tab-2", label: "Monitoring", icon: None, color_tag: None, closable: false, active: self.active_view == 2 },
        ];
        let chrome_view = ChromeView {
            tabs: &chrome_tabs,
            active_tab_id: Some(tab_ids[self.active_view]),
            show_new_tab_btn: false,
            show_menu_btn: false,
            show_new_window_btn: false,
            show_close_window_btn: false,
            is_maximized: self.window.is_maximized(),
            cursor_x: mx,
            cursor_y: my,
            time_ms,
        };
        let chrome_settings = ChromeSettings {
            theme: Box::<uzor::ui::widgets::composite::chrome::theme::DefaultChromeTheme>::default(),
            style: Box::new(ChromeWithBottomBorder),
        };
        let chrome_kind = ChromeRenderKind::Default;
        register_layout_manager_chrome(
            &mut self.layout,
            &mut render,
            LayoutNodeId::ROOT,
            "chrome",
            &mut self.chrome_state,
            &chrome_view,
            &chrome_settings,
            &chrome_kind,
        );

        // ── Top toolbar ───────────────────────────────────────────────────────
        let file_btn_active = self.dropdown_file_state.open;
        let view_btn_active = self.dropdown_view_state.open;
        let modals_btn_active = self.dropdown_help_state.open;
        // Suppress unused warnings — file dropdown removed; keep state field
        // for now to avoid larger refactor.
        let _ = file_btn_active;
        let top_toolbar_items = [
            ToolbarItem::TextButton { id: "tb-view", text: "View",   active: view_btn_active,   tooltip: Some("View menu") },
            ToolbarItem::TextButton { id: "tb-help", text: "Modals", active: modals_btn_active, tooltip: Some("Modals menu") },
        ];
        let clock_items = [
            ToolbarItem::Clock { id: "top-clock", time_text: clock.as_str() },
        ];
        let top_toolbar_view = ToolbarView {
            start: ToolbarSection { items: &top_toolbar_items },
            center: ToolbarSection::empty(),
            end: ToolbarSection { items: &clock_items },
            chrome: None,
        };
        register_layout_manager_toolbar(
            &mut self.layout,
            &mut render,
            LayoutNodeId::ROOT,
            "top-toolbar",
            "top-toolbar-widget",
            &mut self.top_toolbar_state,
            &top_toolbar_view,
            &ToolbarSettings::new(
                Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                Box::new(HorizToolbarWithBorder),
            ),
            &ToolbarRenderKind::Horizontal,
        );

        // ── Left vertical toolbar ─────────────────────────────────────────────
        // Fix #5: use TextButton with SVG symbols — IconButton requires a
        // populated icon registry which may be empty in examples.
        let sidebar_open = self.sidebar_open;
        let left_items = [
            ToolbarItem::TextButton { id: "lt-toggle-sidebar", text: "☰", active: sidebar_open, tooltip: Some("Toggle sidebar") },
        ];
        if self.left_toolbar_visible {
            let left_toolbar_view = ToolbarView {
                start: ToolbarSection { items: &left_items },
                center: ToolbarSection::empty(),
                end: ToolbarSection::empty(),
                chrome: None,
            };
            register_layout_manager_toolbar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "left-vtoolbar",
                "left-vtoolbar-widget",
                &mut self.left_vtoolbar_state,
                &left_toolbar_view,
                &ToolbarSettings::new(
                    Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                    Box::new(VertToolbarWithBorder),
                ),
                &ToolbarRenderKind::Vertical,
            );
        }

        // ── Sidebar ───────────────────────────────────────────────────────────
        // Sidebar shows dock panel list with close buttons + "Add Panel" button.
        if self.sidebar_open {
            let sidebar_actions: &[HeaderAction<'_>] = &[];
            let sidebar_header = SidebarHeader { icon: None, title: "Dock Panels", actions: sidebar_actions };
            let mut sidebar_view = SidebarView {
                header: sidebar_header,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                content_height: 200.0,
            };
            let _sidebar_node = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "sidebar",
                "sidebar-widget",
                &mut self.sidebar_state,
                &mut sidebar_view,
                &{
                    let mut s = SidebarSettings::default();
                    s.style = Box::new(NoDividerSidebarStyle(DefaultSidebarStyle));
                    s
                },
                &SidebarRenderKind::Left,
            );
            // Sidebar body — spawn UI + panel list.
            if let Some(body_rect) = self.layout.rect_for_edge_slot("sidebar") {
                use uzor::input::core::sense::Sense;
                use uzor::input::core::widget_kind::WidgetKind;

                const SIDEBAR_HEADER_H: f64 = 40.0;
                let mut y = body_rect.y + SIDEBAR_HEADER_H + 8.0;
                let bx = body_rect.x + 8.0;
                let bw = body_rect.width - 16.0;

                // ── NEW PANEL section header
                label(&mut render, Rect::new(bx, y, bw, 22.0), "NEW PANEL", TextAlign::Left, "rgba(255,255,255,0.4)");
                y += 22.0;

                // ── Type label
                label(&mut render, Rect::new(bx, y, bw, 20.0), "Type:", TextAlign::Left, "rgba(255,255,255,0.55)");
                y += 20.0;

                // Radio buttons for panel kind
                for kind in PanelKind::all() {
                    let radio_id = format!("spawn-kind-{}", kind.title().to_lowercase());
                    let selected = &self.spawn_kind == kind;
                    let rx = bx + 6.0;
                    let ry = y;
                    // radio dot
                    if selected {
                        render.set_fill_color("#2962ff");
                        render.fill_rounded_rect(rx, ry + 3.0, 10.0, 10.0, 5.0);
                    } else {
                        render.set_fill_color("rgba(255,255,255,0.18)");
                        render.fill_rounded_rect(rx, ry + 3.0, 10.0, 10.0, 5.0);
                    }
                    label(&mut render, Rect::new(rx + 16.0, ry, bw - 22.0, 20.0), kind.title(), TextAlign::Left, if selected { "#ffffff" } else { "#a0a0b0" });
                    self.layout.ctx_mut().input.register_atomic(
                        WidgetId::new(radio_id),
                        WidgetKind::Button,
                        Rect::new(bx, ry, bw, 20.0),
                        Sense::CLICK | Sense::HOVER,
                        &LayerId::main(),
                    );
                    y += 22.0;
                }

                y += 6.0;
                // ── Split label
                label(&mut render, Rect::new(bx, y, bw, 20.0), "Split:", TextAlign::Left, "rgba(255,255,255,0.55)");
                y += 20.0;

                let splits = [
                    ("Split right",  "spawn-split-horiz"),
                    ("Split bottom", "spawn-split-vert"),
                    ("Grid 2×2",     "spawn-split-grid"),
                ];
                for (split_lbl, id) in &splits {
                    let selected = match *id {
                        "spawn-split-horiz" => self.spawn_split == SpawnSplit::SplitRight,
                        "spawn-split-vert"  => self.spawn_split == SpawnSplit::SplitBottom,
                        _                   => self.spawn_split == SpawnSplit::Grid2x2,
                    };
                    let rx = bx + 6.0;
                    if selected {
                        render.set_fill_color("#2962ff");
                        render.fill_rounded_rect(rx, y + 3.0, 10.0, 10.0, 5.0);
                    } else {
                        render.set_fill_color("rgba(255,255,255,0.18)");
                        render.fill_rounded_rect(rx, y + 3.0, 10.0, 10.0, 5.0);
                    }
                    label(&mut render, Rect::new(rx + 16.0, y, bw - 22.0, 20.0), split_lbl, TextAlign::Left, if selected { "#ffffff" } else { "#a0a0b0" });
                    self.layout.ctx_mut().input.register_atomic(
                        WidgetId::new(*id),
                        WidgetKind::Button,
                        Rect::new(bx, y, bw, 20.0),
                        Sense::CLICK | Sense::HOVER,
                        &LayerId::main(),
                    );
                    y += 22.0;
                }

                y += 8.0;

                // ── Spawn button
                render.set_fill_color("#2962ff");
                render.fill_rounded_rect(bx, y, bw, 28.0, 4.0);
                label(&mut render, Rect::new(bx, y, bw, 28.0), "Spawn", TextAlign::Center, "#ffffff");
                self.layout.ctx_mut().input.register_atomic(
                    WidgetId::new("sidebar-spawn"),
                    WidgetKind::Button,
                    Rect::new(bx, y, bw, 28.0),
                    Sense::CLICK | Sense::HOVER,
                    &LayerId::main(),
                );
                y += 36.0;

                // ── Divider
                render.set_fill_color("rgba(255,255,255,0.08)");
                render.fill_rect(bx, y, bw, 1.0);
                y += 10.0;

                // ── PANELS section header
                label(&mut render, Rect::new(bx, y, bw, 22.0), "PANELS", TextAlign::Left, "rgba(255,255,255,0.4)");
                y += 22.0;

                // Collect current tab's leaves
                let leaf_entries: Vec<(uzor::docking::panels::LeafId, String)> = {
                    let mut entries: Vec<(uzor::docking::panels::LeafId, String)> = self
                        .layout
                        .panels()
                        .panel_rects()
                        .keys()
                        .map(|&id| {
                            let title = self
                                .layout
                                .panels()
                                .tree()
                                .leaf(id)
                                .and_then(|l| l.panels.first())
                                .map(|p| p.title().to_string())
                                .unwrap_or_else(|| format!("Panel {}", id.0));
                            (id, title)
                        })
                        .collect();
                    entries.sort_by_key(|(id, _)| id.0);
                    entries
                };

                for (idx, (leaf_id, title)) in leaf_entries.iter().enumerate() {
                    let is_active = self.layout.panels().active_leaf() == Some(*leaf_id);
                    render.set_fill_color(if is_active { "rgba(41,98,255,0.18)" } else { "rgba(255,255,255,0.05)" });
                    render.fill_rounded_rect(bx, y, bw, 26.0, 3.0);
                    label(&mut render, Rect::new(bx + 10.0, y, bw - 36.0, 26.0), title.as_str(), TextAlign::Left, if is_active { "#4d90fe" } else { "#d1d4dc" });

                    let close_x = bx + bw - 22.0;
                    let close_id = format!("dock-leaf-close-{idx}");
                    self.layout.ctx_mut().input.register_atomic(
                        WidgetId::new(close_id),
                        WidgetKind::Button,
                        Rect::new(close_x, y + 5.0, 16.0, 16.0),
                        Sense::CLICK | Sense::HOVER,
                        &LayerId::main(),
                    );
                    label(&mut render, Rect::new(close_x, y + 5.0, 16.0, 16.0), "×", TextAlign::Center, "rgba(255,80,80,0.5)");

                    y += 30.0;
                }

                // (keep y alive so borrow checker doesn't warn on unused assignment)
                let _ = y;
            }
        }

        // ── Main content — iterate ALL leaves of the current dock tree ──────────
        {
            use uzor::input::core::sense::Sense;
            use uzor::input::core::widget_kind::WidgetKind;

            let active_leaf = self.layout.panels().active_leaf();

            // Snapshot leaf data to avoid borrow conflicts during render
            let leaf_data: Vec<(uzor::docking::panels::LeafId, uzor::docking::panels::PanelRect, PanelKind)> = {
                self.layout.panels().panel_rects().iter()
                    .map(|(&id, &rect)| {
                        let kind = self.layout.panels().tree().leaf(id)
                            .and_then(|l| l.panels.first())
                            .map(|p| p.kind.clone())
                            .unwrap_or(PanelKind::Notes);
                        (id, rect, kind)
                    })
                    .collect()
            };

            for (leaf_id, panel_rect, kind) in &leaf_data {
                let rect = Rect::new(
                    panel_rect.x as f64,
                    panel_rect.y as f64,
                    panel_rect.width as f64,
                    panel_rect.height as f64,
                );

                // Register hit zone for this leaf as a Blackbox composite —
                // coord won't recurse into it, app owns the entire rect.
                let leaf_widget_id = format!("dock-leaf-{}", leaf_id.0);
                let _composite_id = self.layout.ctx_mut().input.register_composite(
                    WidgetId::new(leaf_widget_id),
                    WidgetKind::BlackboxPanel,
                    rect,
                    Sense::CLICK | Sense::HOVER | Sense::DRAG | Sense::SCROLL,
                    &LayerId::main(),
                );

                // Render panel body by kind
                render_panel_body(kind, &mut render, rect, &self.watchlist);

                // Active leaf border (fix 4)
                if Some(*leaf_id) == active_leaf {
                    render.set_stroke_color("#2962ff");
                    render.set_stroke_width(2.0);
                    render.stroke_rect(rect.x, rect.y, rect.width, rect.height);
                }
            }
        }

        // ── Dock separators (fix 5/6) ─────────────────────────────────────────
        {
            use uzor::docking::panels::SeparatorOrientation as DockSepOrient;
            let separators: Vec<_> = self.layout.panels().separators().iter().enumerate().map(|(i, s)| {
                let thickness = s.thickness_for_state() as f64;
                let (sx, sy, sw, sh) = match s.orientation {
                    DockSepOrient::Vertical => {
                        // position = x, start = y, length = height
                        (s.position as f64 - thickness / 2.0, s.start as f64, thickness, s.length as f64)
                    }
                    DockSepOrient::Horizontal => {
                        // position = y, start = x, length = width
                        (s.start as f64, s.position as f64 - thickness / 2.0, s.length as f64, thickness)
                    }
                };
                (i, sx, sy, sw, sh, s.orientation)
            }).collect();

            let dragging_sep = if let Some(DragTarget::SeparatorDrag { sep_idx, .. }) = self.drag_target {
                Some(sep_idx)
            } else {
                None
            };

            for (i, sx, sy, sw, sh, _orient) in &separators {
                let color = if dragging_sep == Some(*i) {
                    "rgba(100,160,255,0.7)"
                } else {
                    "rgba(80,80,100,0.5)"
                };
                render.set_fill_color(color);
                render.fill_rect(*sx, *sy, *sw, *sh);
            }
        }

        // ── Modal ─────────────────────────────────────────────────────────────
        if self.modal_open {
            let modal_w = match self.modal_kind {
                ModalKind::L2       => L2_WIN_W + 24.0,
                ModalKind::L1       => 320.0,
                ModalKind::Settings => 400.0,
                ModalKind::Tags     => 480.0,
            };
            let modal_h = match self.modal_kind {
                ModalKind::L2       => L2_WIN_H + 80.0,
                ModalKind::L1       => 200.0,
                ModalKind::Settings => 300.0,
                ModalKind::Tags     => 360.0,
            };
            // Fix #10/#11: use modal_state.position (dragged) instead of always centering.
            let default_x = (width as f64 / 2.0 - modal_w / 2.0).max(0.0);
            let default_y = (height as f64 / 2.0 - modal_h / 2.0).max(0.0);
            let (frame_x, frame_y) = if self.modal_state.position != (0.0, 0.0) {
                self.modal_state.position
            } else {
                (default_x, default_y)
            };
            let modal_rect = Rect::new(frame_x, frame_y, modal_w, modal_h);
            self.layout.push_overlay(OverlayEntry {
                id: "modal-overlay".to_string(),
                kind: OverlayKind::Modal,
                rect: modal_rect,
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::modal(), 10, true);

            let modal_kind = self.modal_kind;
            let l2_connected = self.l2_connected;
            let l2_checked = self.l2_checked;
            let l2_toggled = self.l2_toggled;
            let l2_radio_sel = self.l2_radio_sel;
            let l2_slider_val = self.l2_slider_val;
            let l2_range_min = self.l2_range_min;
            let l2_range_max = self.l2_range_max;
            let l2_range_drag_handle = self.l2_range_drag_handle;
            let l2_scroll_off = self.l2_scroll_off;
            let l2_swatch_sel = self.l2_swatch_sel;
            let l2_active_tab = self.l2_active_tab;
            let l2_active_sub_tab = self.l2_active_sub_tab;
            let l2_right_panel_w = self.l2_right_panel_w;
            let l2_hovered = self.l2_hovered.clone();
            let l2_pressed = self.l2_pressed.clone();
            let start_time = self.start;
            let l2_sb_x = L2_WIN_W - SB_W - 8.0;
            let l2_right_panel_x = L2_WIN_W - l2_right_panel_w;

            let title = match modal_kind {
                ModalKind::L2       => "L2 Widget Set",
                ModalKind::L1       => "L1 Custom Button",
                ModalKind::Settings => "Settings",
                ModalKind::Tags     => "Dock Panels",
            };
            let footer_btns = [
                FooterBtn { label: "Close", style: FooterBtnStyle::Ghost },
                FooterBtn { label: "Apply", style: FooterBtnStyle::Primary },
            ];
            let mut modal_view = ModalView {
                title: Some(title),
                tabs: &[],
                footer_buttons: &footer_btns,
                wizard_pages: &[],
                backdrop: BackdropKind::Dim,
            };
            let _modal_node = register_layout_manager_modal(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "modal-overlay",
                "modal-widget",
                &mut self.modal_state,
                &mut modal_view,
                &ModalSettings::default(),
                &ModalRenderKind::WithHeaderFooter,
            );
            // Draw modal body content inline
            if let Some(body_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let layer = LayerId::modal();
                match modal_kind {
                    ModalKind::L1 => {
                        // Fix 2 + Fix 5-6: custom button with hover/press colour animation.
                        let btn_w = 200.0_f64;
                        let btn_h = 60.0_f64;
                        let cx = body_rect.x + body_rect.width / 2.0;
                        let cy = body_rect.y + body_rect.height / 2.0;
                        let btn_r = Rect::new(cx - btn_w / 2.0, cy - btn_h / 2.0, btn_w, btn_h);
                        use uzor::input::core::sense::Sense;
                        use uzor::input::core::widget_kind::WidgetKind;
                        self.layout.ctx_mut().input.register_atomic(
                            WidgetId::new("l1-mybtn"),
                            WidgetKind::Custom,
                            btn_r,
                            Sense::CLICK | Sense::HOVER,
                            &layer,
                        );
                        // Fix 2: three-state colour: pressed > hovered > normal
                        let btn_color = if self.l1_btn_pressed {
                            "#1a40c8"   // pressed — darker blue
                        } else if self.l1_btn_hovered {
                            "#4080ff"   // hovered — lighter blue
                        } else {
                            "#3769af"   // normal
                        };
                        render.set_fill_color(btn_color);
                        render.fill_rounded_rect(btn_r.x, btn_r.y, btn_r.width, btn_r.height, 6.0);
                        label(&mut render, btn_r, "Click me (L1 custom)", TextAlign::Center, "#ffffff");
                    }
                    ModalKind::Settings => {
                        label(&mut render, Rect::new(body_rect.x, body_rect.y, body_rect.width, 40.0), "Settings content", TextAlign::Center, "rgba(255,255,255,0.55)");
                        let items = [
                            ("Enable dark mode", true),
                            ("Show tooltips",    true),
                            ("Auto-save",        false),
                        ];
                        for (i, (label, checked)) in items.iter().enumerate() {
                            let r = Rect::new(body_rect.x + 16.0, body_rect.y + 48.0 + i as f64 * 36.0, body_rect.width - 32.0, 28.0);
                            let cb_id = format!("settings-cb-{i}");
                            register_context_manager_checkbox(
                                self.layout.ctx_mut(), &mut render,
                                cb_id.as_str(), r, &layer,
                                WidgetState::Normal,
                                &CheckboxView { checked: *checked, label: Some(label) },
                                &CheckboxSettings::default().with_theme(Box::new(VisibleCheckboxTheme)),
                                &CheckboxRenderKind::Standard,
                                "13px sans-serif",
                            );
                        }
                    }
                    ModalKind::Tags => {
                        // "Panels" modal — lists real dock leaves of the active tab,
                        // clipped to body_rect so nothing overflows.
                        const MODAL_HEADER_H: f64 = 44.0;
                        let body_inner = Rect::new(
                            body_rect.x + 16.0,
                            body_rect.y + MODAL_HEADER_H + 8.0,
                            body_rect.width - 32.0,
                            body_rect.height - MODAL_HEADER_H - 16.0,
                        );

                        render.save();
                        render.clip_rect(body_inner.x, body_inner.y, body_inner.width, body_inner.height);

                        // Section header
                        {
                            let hdr = format!("PANELS — Tab: {}", ["Dashboard", "Panels", "Monitoring"][self.active_view]);
                            label(&mut render, Rect::new(body_inner.x, body_inner.y, body_inner.width, 20.0), &hdr, TextAlign::Left, "rgba(255,255,255,0.5)");
                        }

                        // Real leaves of currently active dock
                        let leaves: Vec<(String, uzor::docking::panels::LeafId)> = {
                            let mut entries: Vec<(uzor::docking::panels::LeafId, String)> = self
                                .layout.panels().panel_rects().keys()
                                .map(|&id| {
                                    let title = self.layout.panels().tree().leaf(id)
                                        .and_then(|l| l.panels.first())
                                        .map(|p| p.title().to_string())
                                        .unwrap_or_else(|| format!("Panel {}", id.0));
                                    (id, title)
                                })
                                .collect();
                            entries.sort_by_key(|(id, _)| id.0);
                            entries.into_iter().map(|(id, t)| (t, id)).collect()
                        };

                        let mut row_y = body_inner.y + 28.0;
                        for (idx, (title, leaf_id)) in leaves.iter().enumerate() {
                            if row_y + 32.0 > body_inner.y + body_inner.height { break; }
                            render.set_fill_color("rgba(255,255,255,0.05)");
                            render.fill_rounded_rect(body_inner.x, row_y, body_inner.width, 30.0, 4.0);
                            {
                                let idx_str  = format!("#{}", idx + 1);
                                let leaf_str = format!("leaf {}", leaf_id.0);
                                label(&mut render, Rect::new(body_inner.x + 8.0, row_y, 32.0, 30.0),               &idx_str,        TextAlign::Left,  "#a6adc8");
                                label(&mut render, Rect::new(body_inner.x + 40.0, row_y, body_inner.width - 120.0, 30.0), title.as_str(),   TextAlign::Left,  "#d1d4dc");
                                label(&mut render, Rect::new(body_inner.x, row_y, body_inner.width - 8.0, 30.0),   &leaf_str,       TextAlign::Right, "rgba(255,255,255,0.35)");
                            }
                            row_y += 36.0;
                        }

                        if leaves.is_empty() {
                            label(&mut render, body_inner, "(no panels in this tab)", TextAlign::Center, "rgba(255,255,255,0.4)");
                        }

                        render.restore();
                    }
                    ModalKind::L2 => {
                        // Full L2 widget set rendered inside modal body.
                        // body_rect from rect_for_overlay is the full modal rect;
                        // actual content starts after the 44px header.
                        // WithHeaderFooter: header=44, footer=52 → body height = total - 96
                        const MODAL_HEADER_H: f64 = 44.0;
                        const MODAL_FOOTER_H: f64 = 52.0;
                        let left_panel_w = l2_right_panel_x - LEFT_PANEL_X - SPLITTER_W / 2.0;
                        let ox = body_rect.x;
                        let oy = body_rect.y + MODAL_HEADER_H; // skip modal header (Fix 2)
                        let body_h = body_rect.height - MODAL_HEADER_H - MODAL_FOOTER_H;

                        let text_id = WidgetId::new("l2-text");
                        let text_str = self.layout.ctx_mut().input.text_fields().text(&text_id).to_owned();
                        let text_cursor = self.layout.ctx_mut().input.text_fields().cursor(&text_id);
                        let text_sel = self.layout.ctx_mut().input.text_fields().selection_range(&text_id);
                        let text_focused = self.layout.ctx_mut().input.text_fields().is_focused(&text_id);
                        let now_ms = start_time.elapsed().as_millis() as u64;
                        let cursor_vis = text_focused && self.layout.ctx_mut().input.text_fields().cursor_visible(now_ms);

                        // Draw panel BGs via RenderContext (Fix 2: use body_h, not L2_WIN_H)
                        let panel_inner_h = body_h - 24.0;
                        render.set_fill_color("#1e222d");
                        render.fill_rounded_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, panel_inner_h, 8.0);
                        render.fill_rounded_rect(ox + l2_right_panel_x, oy + 12.0, L2_WIN_W - l2_right_panel_x - 12.0, panel_inner_h, 8.0);

                        // ctx_l2 removed — use self.layout.ctx_mut() directly so widgets
                        // register into the real coordinator and clicks are dispatched.

                        // ── Left panel (clipped) ──────────────────────────
                        render.save();
                        render.clip_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, panel_inner_h);

                        // 1. Button
                        let btn_state = if l2_hovered.as_deref() == Some("l2-btn-connect") { WidgetState::Hovered } else if l2_connected { WidgetState::Active } else { WidgetState::Normal };
                        register_context_manager_button(
                            self.layout.ctx_mut(), &mut render,
                            "l2-btn-connect", Rect::new(BTN_RECT.x + ox, BTN_RECT.y + oy, BTN_RECT.width, BTN_RECT.height), &layer,
                            btn_state,
                            &ButtonView { text: Some(if l2_connected { "Disconnect" } else { "Connect" }), icon: None, active: l2_connected, disabled: false, active_border: None },
                            &ButtonSettings::default().with_theme(Box::new(VisibleButtonTheme)),
                        );
                        // 2. Checkbox
                        register_context_manager_checkbox(
                            self.layout.ctx_mut(), &mut render,
                            "l2-cb", Rect::new(CB_RECT.x + ox, CB_RECT.y + oy, CB_RECT.width, CB_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-cb") { WidgetState::Hovered } else { WidgetState::Normal },
                            &CheckboxView { checked: l2_checked, label: Some("Setting A") },
                            &CheckboxSettings::default().with_theme(Box::new(VisibleCheckboxTheme)),
                            &CheckboxRenderKind::Standard, "13px sans-serif",
                        );
                        // 3. Toggle
                        register_context_manager_toggle(
                            self.layout.ctx_mut(), &mut render,
                            "l2-tog", Rect::new(TOG_RECT.x + ox, TOG_RECT.y + oy, TOG_RECT.width, TOG_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-tog") { WidgetState::Hovered } else { WidgetState::Normal },
                            &ToggleView { toggled: l2_toggled, label: Some("ON"), disabled: false },
                            &ToggleSettings::default(), &ToggleRenderKind::Switch,
                        );
                        // 4. Radio
                        for (i, cx_off) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                            let rid = format!("l2-radio-{i}");
                            register_context_manager_radio(
                                self.layout.ctx_mut(), &mut render,
                                rid.as_str(), Rect::new(cx_off + ox, 175.0 + oy, 28.0, 28.0), &layer,
                                if l2_hovered.as_deref() == Some(rid.as_str()) { WidgetState::Hovered } else { WidgetState::Normal },
                                &RadioSettings::default(),
                                &RadioRenderKind::Dot { shape: DotShape::Circle, cx: cx_off + 14.0 + ox, cy: 175.0 + 14.0 + oy, view: RadioDotView { selected: l2_radio_sel == i } },
                            );
                        }
                        // 5. Slider
                        register_context_manager_slider(
                            self.layout.ctx_mut(), &mut render,
                            "l2-slider", Rect::new(SLID_RECT.x + ox, SLID_RECT.y + oy, SLID_RECT.width, SLID_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-slider") { WidgetState::Hovered } else { WidgetState::Normal },
                            &SliderView { kind: SliderType::Single { value: l2_slider_val, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: None },
                            &SliderSettings::default(),
                        );
                        // 6. Range slider
                        register_context_manager_slider(
                            self.layout.ctx_mut(), &mut render,
                            "l2-range", Rect::new(RANGE_RECT.x + ox, RANGE_RECT.y + oy, RANGE_RECT.width, RANGE_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-range") { WidgetState::Hovered } else { WidgetState::Normal },
                            &SliderView { kind: SliderType::Dual { min_value: l2_range_min, max_value: l2_range_max, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: l2_range_drag_handle },
                            &SliderSettings::default(),
                        );
                        // 7. Separator
                        register_context_manager_separator(
                            self.layout.ctx_mut(), &mut render,
                            "l2-sep", Rect::new(28.0 + ox, 260.0 + oy, 260.0, 2.0), SeparatorKind::Divider, &layer,
                            &SeparatorView { kind: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal }, hovered: false, dragging: false },
                            &SeparatorSettings::default(),
                        );
                        // 8. Text input
                        let ti_state = if text_focused { WidgetState::Active } else if l2_hovered.as_deref() == Some("l2-text") { WidgetState::Hovered } else { WidgetState::Normal };
                        let ti_settings = TextInputSettings::with_config(uzor::ui::widgets::atomic::text_input::state::TextFieldConfig::text());
                        let ti_view = InputView { text: text_str.as_str(), placeholder: "Search...", cursor: text_cursor, selection: text_sel, focused: text_focused, disabled: false, input_type: InputType::Search };
                        let ti_rect = Rect::new(TI_RECT.x + ox, TI_RECT.y + oy, TI_RECT.width, TI_RECT.height);
                        let ir = draw_input(&mut render, ti_rect, ti_state, &ti_view, &ti_settings);
                        if cursor_vis {
                            draw_input_cursor(&mut render, ir.cursor_x, ir.cursor_y, ir.cursor_height, 1.5, [220, 220, 220, 255]);
                        }
                        // 9. Color swatches
                        let swatch_colors: [[u8; 4]; 4] = [[41,98,255,255],[16,185,129,255],[245,158,11,255],[239,83,80,255]];
                        for (i, color) in swatch_colors.iter().enumerate() {
                            let sid = format!("l2-swatch-{i}");
                            register_context_manager_color_swatch(
                                self.layout.ctx_mut(), &mut render,
                                sid.as_str(), Rect::new(28.0 + i as f64 * 34.0 + ox, 344.0 + oy, 26.0, 26.0), &layer,
                                if l2_hovered.as_deref() == Some(sid.as_str()) { WidgetState::Hovered } else { WidgetState::Normal },
                                &ColorSwatchView { color: *color, hovered: false, selected: l2_swatch_sel == i, show_transparency: false, border_color_override: None },
                                &ColorSwatchSettings::default(), &ColorSwatchRenderKind::Simple,
                            );
                        }

                        render.restore();

                        // ── Right panel (clipped) ─────────────────────────
                        render.save();
                        render.clip_rect(ox + l2_right_panel_x, oy + 12.0, L2_WIN_W - l2_right_panel_x - 12.0, panel_inner_h);

                        let tab_labels = ["List", "Empty", "Sub-tabs"];
                        for (i, lbl) in tab_labels.iter().enumerate() {
                            let tab_w = ((l2_right_panel_w - 16.0) / 3.0).floor();
                            let tab_x = l2_right_panel_x + 8.0 + i as f64 * (tab_w + 4.0);
                            let tab_rect = Rect::new(tab_x + ox, TAB_STRIP_Y + oy, tab_w, TAB_STRIP_H);
                            let tab_id = format!("l2-tab-{i}");
                            let tab_cfg = TabConfig::new(tab_id.as_str(), *lbl).active_if(l2_active_tab == i);
                            register_context_manager_tab(
                                self.layout.ctx_mut(), &mut render,
                                tab_id.as_str(), tab_rect, None, &layer,
                                &TabView { tab: &tab_cfg, hovered: l2_hovered.as_deref() == Some(tab_id.as_str()), pressed: l2_pressed.as_deref() == Some(tab_id.as_str()), close_btn_hovered: false },
                                &TabSettings::default(),
                            );
                        }
                        if l2_active_tab == 0 {
                            let sb_x = l2_sb_x + ox;
                            let sb_track = Rect::new(sb_x, 52.0 + oy, SB_W, SB_H);
                            let viewport_h = SB_H;
                            let thumb_ratio = (viewport_h / CONTENT_H).clamp(0.0, 1.0);
                            let thumb_h = (thumb_ratio * sb_track.height).max(30.0);
                            let scroll_range = sb_track.height - thumb_h;
                            let thumb_y = sb_track.y + (l2_scroll_off / (CONTENT_H - viewport_h).max(1.0)) * scroll_range;
                            let sb_thumb = Rect::new(sb_x, thumb_y, SB_W, thumb_h);
                            register_context_manager_scrollbar(self.layout.ctx_mut(), &mut render, "l2-sb-track", "l2-sb-thumb", sb_track, sb_thumb, 5.0, &layer, CONTENT_H, viewport_h, l2_scroll_off, &ScrollbarSettings::default());

                            let row_labels = ["★ Roboto regular","Sans-serif clean","→ arrow + ✓ check","Quick brown fox","✨ ★ ☀ ☂ ❤","fn main() { ... }","let x: u32 = 42;","if let Some(v) = opt","// monospace code","0xCAFE_BABE","PT Root UI light","вариативный шрифт","12345 67890","Кириллица OK","ƒ unicode glyphs","Bold Roboto bold","❗ Heads up ❗","✓ Done · 14 items","🌍 globe · 🌟 star","═══ end of list ═══"];
                            let content_x = l2_right_panel_x + 8.0 + ox;
                            let content_w = l2_right_panel_w - SB_W - 20.0;
                            for row in 0..CONTENT_ROWS {
                                let row_y = 52.0 + oy + row as f64 * ROW_H - l2_scroll_off;
                                if row_y + ROW_H < 52.0 + oy || row_y > 52.0 + oy + SB_H { continue; }
                                let row_rect = Rect::new(content_x, row_y, content_w, ROW_H - 2.0);
                                let row_id = format!("l2-row-{row}");
                                let row_settings = match row { 0..=4 => ItemSettings::default().with_style(Box::new(RowStyleRoboto)), 5..=9 => ItemSettings::default().with_style(Box::new(RowStyleJetBrains)), 10..=14 => ItemSettings::default().with_style(Box::new(RowStylePtRoot)), _ => ItemSettings::default().with_style(Box::new(RowStyleRobotoBold)) };
                                register_context_manager_item(self.layout.ctx_mut(), &mut render, row_id.as_str(), row_rect, &layer, WidgetState::Normal, &ItemView { label: Some(row_labels[row]), icon: None, svg: None }, &row_settings, &ItemRenderKind::Label);
                            }
                        }
                        if l2_active_tab == 2 {
                            for (i, lbl) in ["Alpha","Beta","Gamma"].iter().enumerate() {
                                let sub_rect = Rect::new(l2_right_panel_x + 8.0 + ox, CONTENT_START_Y + 8.0 + i as f64 * 36.0 + oy, 90.0, 30.0);
                                let sub_id = format!("l2-sub-tab-{i}");
                                let sub_cfg = TabConfig::new(sub_id.as_str(), *lbl).active_if(l2_active_sub_tab == i);
                                register_context_manager_tab(self.layout.ctx_mut(), &mut render, sub_id.as_str(), sub_rect, None, &layer, &TabView { tab: &sub_cfg, hovered: l2_hovered.as_deref() == Some(sub_id.as_str()), pressed: false, close_btn_hovered: false }, &TabSettings::default());
                            }
                            // Fix 6: SVG icon below the sub-tab buttons, changes with active sub-tab
                            let sub_content_y = CONTENT_START_Y + 8.0 + 3.0 * 36.0 + 8.0;
                            let sub_cx = l2_right_panel_x + l2_right_panel_w / 2.0 + ox;
                            let icon_size = 64.0_f64;
                            let ix = sub_cx - icon_size / 2.0;
                            let iy = sub_content_y + oy + 8.0;
                            let (sub_svg, sub_color) = match l2_active_sub_tab {
                                0 => (SVG_CIRCLE,   "#2962ff"),
                                1 => (SVG_TRIANGLE, "#f59e0b"),
                                _ => (SVG_DIAMOND,  "#ef5350"),
                            };
                            draw_svg_icon(&mut render, sub_svg, ix, iy, icon_size, icon_size, sub_color);
                        }
                        if l2_active_tab == 1 {
                            let icon_size = 64.0_f64;
                            let gap = 16.0_f64;
                            let content_cx = l2_right_panel_x + l2_right_panel_w / 2.0 + ox;
                            let content_cy = CONTENT_START_Y + (L2_WIN_H - CONTENT_START_Y - 12.0) / 2.0 + oy;
                            let grid_x0 = content_cx - (icon_size * 2.0 + gap) / 2.0;
                            let grid_y0 = content_cy - (icon_size * 2.0 + gap) / 2.0;
                            for (idx, (svg, color)) in [(SVG_CIRCLE,"#2962ff"),(SVG_SQUARE,"#10b981"),(SVG_TRIANGLE,"#f59e0b"),(SVG_DIAMOND,"#ef5350")].iter().enumerate() {
                                let col = idx % 2;
                                let row = idx / 2;
                                draw_svg_icon(&mut render, svg, grid_x0 + col as f64 * (icon_size + gap), grid_y0 + row as f64 * (icon_size + gap), icon_size, icon_size, color);
                            }
                        }

                        render.restore();

                        // Splitter drag handle (no clip needed) — height matches body panels
                        let dh_rect = Rect::new(l2_right_panel_x - SPLITTER_W / 2.0 + ox, 12.0 + oy, SPLITTER_W, panel_inner_h);
                        register_context_manager_drag_handle(
                            self.layout.ctx_mut(), &mut render,
                            "l2-splitter", dh_rect, &layer,
                            &DragHandleView { rect: dh_rect }, &DragHandleSettings::default(), &DragHandleRenderKind::GripDots,
                        );
                    }
                }
            }
        }

        // ── Context menu ──────────────────────────────────────────────────────
        if self.ctx_menu_state.is_open {
            let items = [
                ContextMenuItem { action: "ctx-copy",     label: "Copy",       icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-paste",    label: "Paste",      icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-delete",   label: "Delete",     icon: None, danger: true,  separator_after: true,  enabled: true },
                ContextMenuItem { action: "ctx-props",    label: "Properties", icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-settings", label: "Settings",   icon: None, danger: false, separator_after: false, enabled: true },
            ];
            let menu_h = items.len() as f64 * 28.0 + 16.0;
            self.layout.push_overlay(OverlayEntry {
                id: "ctx-menu-overlay".to_string(),
                kind: OverlayKind::ContextMenu,
                rect: Rect::new(self.ctx_menu_state.x, self.ctx_menu_state.y, 170.0, menu_h),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 20, false);
            let mut ctx_menu_view = ContextMenuView { items: &items, target_id: None, title: None };
            register_layout_manager_context_menu(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "ctx-menu-overlay",
                "ctx-menu-widget",
                &mut self.ctx_menu_state,
                &mut ctx_menu_view,
                &ContextMenuSettings::default(),
                &ContextMenuRenderKind::Minimal,
            );
        }

        // ── Dropdown menus (File / View / Help) ───────────────────────────────
        let file_items = [
            DropdownItem::Item { id: "file-new",  label: "New",  icon: None, right: DropdownItemRight::Shortcut("Ctrl+N"), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "file-open", label: "Open", icon: None, right: DropdownItemRight::Shortcut("Ctrl+O"), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "file-save", label: "Save", icon: None, right: DropdownItemRight::Shortcut("Ctrl+S"), disabled: false, danger: false, accent_color: None },
            DropdownItem::Separator,
            DropdownItem::Item { id: "file-quit", label: "Quit", icon: None, right: DropdownItemRight::None, disabled: false, danger: true, accent_color: None },
        ];
        if self.dropdown_file_state.open {
            let hovered_id = self.dropdown_file_state.hovered_id.clone();
            let origin = self.dropdown_file_state.effective_origin();
            self.layout.push_overlay(OverlayEntry {
                id: "dd-file-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, 200.0, 160.0),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 25, false);
            let mut dd_file_view = DropdownView {
                anchor: self.dropdown_file_state.anchor_rect,
                position_override: self.dropdown_file_state.open_position_override,
                open: true,
                kind: DropdownViewKind::Flat { items: &file_items, hovered_id: hovered_id.as_deref(), submenu_items: None, submenu_hovered_id: None },
            };
            register_layout_manager_dropdown(
                &mut self.layout, &mut render,
                LayoutNodeId::ROOT, "dd-file-overlay", "dd-file-widget",
                &mut self.dropdown_file_state,
                &mut dd_file_view,
                &DropdownSettings::default(),
                DropdownRenderKind::Flat,
            );
        }

        let view_items = [
            DropdownItem::Item { id: "view-sidebar", label: "Toggle Sidebar", icon: None, right: DropdownItemRight::Toggle(self.sidebar_open),         disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "view-toolbar", label: "Show Toolbar",   icon: None, right: DropdownItemRight::Toggle(self.left_toolbar_visible), disabled: false, danger: false, accent_color: None },
        ];
        if self.dropdown_view_state.open {
            let hovered_id = self.dropdown_view_state.hovered_id.clone();
            let origin = self.dropdown_view_state.effective_origin();
            let (vw, vh) = measure_flat(&view_items, &DropdownSettings::default());
            self.layout.push_overlay(OverlayEntry {
                id: "dd-view-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, vw, vh),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 25, false);
            let mut dd_view_view = DropdownView {
                anchor: self.dropdown_view_state.anchor_rect,
                position_override: self.dropdown_view_state.open_position_override,
                open: true,
                kind: DropdownViewKind::Flat { items: &view_items, hovered_id: hovered_id.as_deref(), submenu_items: None, submenu_hovered_id: None },
            };
            register_layout_manager_dropdown(
                &mut self.layout, &mut render,
                LayoutNodeId::ROOT, "dd-view-overlay", "dd-view-widget",
                &mut self.dropdown_view_state,
                &mut dd_view_view,
                &DropdownSettings::default(),
                DropdownRenderKind::Flat,
            );
        }

        let help_items = [
            DropdownItem::Item { id: "modals-l2",     label: "L2 Modal",     icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-l1",     label: "L1 Modal",     icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-panels", label: "Panels Modal", icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
        ];
        if self.dropdown_help_state.open {
            let hovered_id = self.dropdown_help_state.hovered_id.clone();
            let origin = self.dropdown_help_state.effective_origin();
            let (hw, hh) = measure_flat(&help_items, &DropdownSettings::default());
            self.layout.push_overlay(OverlayEntry {
                id: "dd-help-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, hw, hh),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 25, false);
            let mut dd_help_view = DropdownView {
                anchor: self.dropdown_help_state.anchor_rect,
                position_override: self.dropdown_help_state.open_position_override,
                open: true,
                kind: DropdownViewKind::Flat { items: &help_items, hovered_id: hovered_id.as_deref(), submenu_items: None, submenu_hovered_id: None },
            };
            register_layout_manager_dropdown(
                &mut self.layout, &mut render,
                LayoutNodeId::ROOT, "dd-help-overlay", "dd-help-widget",
                &mut self.dropdown_help_state,
                &mut dd_help_view,
                &DropdownSettings::default(),
                DropdownRenderKind::Flat,
            );
        }

        // ── Add Panel split-kind dropdown ─────────────────────────────────────
        let addpanel_items = [
            DropdownItem::Item { id: "split-horiz",       label: "Split Horizontal",  icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "split-vert",        label: "Split Vertical",    icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "split-grid",        label: "Grid 2×2",          icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "split-2left1right", label: "2 Left + 1 Right",  icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "split-1left2right", label: "1 Left + 2 Right",  icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
        ];
        if self.dropdown_addpanel_state.open {
            let hovered_id = self.dropdown_addpanel_state.hovered_id.clone();
            let origin = self.dropdown_addpanel_state.effective_origin();
            self.layout.push_overlay(OverlayEntry {
                id: "dd-addpanel-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, 200.0, 160.0),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 25, false);
            let mut dd_addpanel_view = DropdownView {
                anchor: self.dropdown_addpanel_state.anchor_rect,
                position_override: self.dropdown_addpanel_state.open_position_override,
                open: true,
                kind: DropdownViewKind::Flat { items: &addpanel_items, hovered_id: hovered_id.as_deref(), submenu_items: None, submenu_hovered_id: None },
            };
            register_layout_manager_dropdown(
                &mut self.layout, &mut render,
                LayoutNodeId::ROOT, "dd-addpanel-overlay", "dd-addpanel-widget",
                &mut self.dropdown_addpanel_state,
                &mut dd_addpanel_view,
                &DropdownSettings::default(),
                DropdownRenderKind::Flat,
            );
        }

        // ── Popup (hover on toolbar items) ────────────────────────────────────
        if let Some(ref item) = self.popup_item.clone() {
            let popup_text = match item.as_str() {
                "tb-file" => "Open file menu (Alt+F)",
                "tb-view" => "Toggle view options",
                "tb-help" => "Help and documentation",
                "tb-new"  => "Create new chart (Ctrl+N)",
                _         => "Toolbar item",
            };
            let popup_origin = (mx, my + 20.0);
            self.layout.push_overlay(OverlayEntry {
                id: "popup-overlay".to_string(),
                kind: OverlayKind::Popup,
                rect: Rect::new(popup_origin.0, popup_origin.1, 220.0, 32.0),
                anchor: None,
            });
            self.layout.ctx_mut().input.push_layer(LayerId::popup(), 15, false);
            let text_for_popup = popup_text;
            let mut popup_view = PopupView {
                origin: popup_origin,
                anchor: None,
                backdrop: PopupBackdrop::None,
                kind: PopupViewKind::Plain,
            };
            let _popup_node = register_layout_manager_popup(
                &mut self.layout, &mut render,
                LayoutNodeId::ROOT, "popup-overlay", "popup-widget",
                &mut self.popup_state,
                &mut popup_view,
                &PopupSettings::default(),
                PopupRenderKind::Plain,
            );
            // Draw popup body inline
            if let Some(body_rect) = self.layout.rect_for_overlay("popup-overlay") {
                render.set_fill_color("#1e222d");
                render.fill_rounded_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height, 4.0);
                label(&mut render, body_rect, text_for_popup, TextAlign::Center, "#d1d4dc");
            }
        }

        // ── end_frame ─────────────────────────────────────────────────────────
        let responses = self.layout.ctx_mut().input.end_frame();

        // Debug: only print non-hover responses (hover spams every frame)
        if !responses.is_empty() {
            let interesting: Vec<_> = responses.iter()
                .filter(|(_, r)| r.clicked || r.scrolled || r.dragged)
                .collect();
            if !interesting.is_empty() {
                eprintln!("[END_FRAME] {} responses ({} interesting)", responses.len(), interesting.len());
                for (id, resp) in &interesting {
                    eprintln!("  - {} clicked={} hovered={} scrolled={} dragged={}",
                        id.0, resp.clicked, resp.hovered, resp.scrolled, resp.dragged);
                }
            }
        }

        // Process coordinator responses
        for (id, resp) in &responses {
            let ids = id.0.as_str();
            if resp.scrolled && (ids == "l2-sb-track" || ids == "l2-sb-thumb") {
                let dy = resp.scroll_delta.1;
                self.l2_scroll_off = (self.l2_scroll_off + dy * 20.0)
                    .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
            }
        }

        // Update popup based on hovered widget
        let hovered_id = self.layout.ctx_mut().input.hovered_widget().map(|id| id.0.clone());
        let toolbar_items_with_popup = ["tb-file", "tb-view", "tb-help", "tb-new"];
        self.popup_item = hovered_id.as_deref()
            .and_then(|id| if toolbar_items_with_popup.contains(&id) { Some(id.to_string()) } else { None });

        // ── GPU submit ────────────────────────────────────────────────────────
        let dev = &self.render_cx.devices[self.surface.dev_id];
        let render_params = RenderParams {
            base_color: BG,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer
            .render_to_texture(&dev.device, &dev.queue, &self.scene, &self.surface.target_view, &render_params)
            .unwrap_or_default();
        let surface_texture = match self.surface.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = dev.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("l3-blit") });
        self.surface.blitter.copy(&dev.device, &mut encoder, &self.surface.target_view, &surface_view);
        dev.queue.submit([encoder.finish()]);
        surface_texture.present();

        // (overlays cleared at start of next frame, not here, so they remain
        // queryable via rect_for_overlay between frames for outside-click)

        if self.exit_requested {
            event_loop.exit();
        }

        self.window.request_redraw();
    }

    fn on_left_up(
        &mut self,
        x: f64,
        y: f64,
        clicked_id: Option<WidgetId>,
        event_loop: &ActiveEventLoop,
    ) {
        eprintln!("[LEFT_UP] pos=({:.1},{:.1}) clicked_id={:?} modal_open={} dropdown_open=(file:{} view:{} help:{}) ctx_menu_open={}",
            x, y, clicked_id.as_ref().map(|id| id.0.as_str()),
            self.modal_open,
            self.dropdown_file_state.open,
            self.dropdown_view_state.open,
            self.dropdown_help_state.open,
            self.ctx_menu_state.is_open,
        );

        // ── Priority 1: coord-resolved widget id ──────────────────────────────
        // If InputCoordinator identified the click as landing on a registered
        // widget, dispatch by id BEFORE the manual geometry fallback.  This
        // makes every register_*-ed widget click-able without per-button hit
        // testing in the example.
        if let Some(id) = clicked_id.as_ref() {
            let id_str = id.0.as_str();

            // Bug 4: Modal close affordances registered as "modal-widget:close" /
            // "modal-widget:footer:0" / "modal-widget:footer:1"
            if id_str == "modal-widget:close"
                || id_str == "modal-widget:footer:0"
                || id_str == "modal-widget:footer:1"
            {
                eprintln!("[DISPATCH] modal close via {id_str}");
                self.modal_open = false;
                println!("[L3] modal closed via {id_str}");
                return;
            }
            // Legacy pattern fallback (other modals that use "-close" suffix)
            if id_str.starts_with("modal-") && (
                id_str.ends_with("-close") ||
                id_str.ends_with("-apply") ||
                id_str.ends_with("-cancel")
            ) {
                eprintln!("[DISPATCH] modal close (legacy) via {id_str}");
                self.modal_open = false;
                println!("[L3] modal closed via {id_str}");
                return;
            }

            // Toolbar dropdown triggers
            match id_str {
                "tb-file" => {
                    eprintln!("[DISPATCH] tb-file toolbar button");
                    if let Some(toolbar_rect) = self.layout.rect_for_edge_slot("top-toolbar") {
                        let was_open = self.dropdown_file_state.open;
                        self.dropdown_view_state.close();
                        self.dropdown_help_state.close();
                        if was_open {
                            self.dropdown_file_state.close();
                        } else {
                            let file_x = toolbar_rect.x + 4.0;
                            self.dropdown_file_state.open_at(file_x, toolbar_rect.y + toolbar_rect.height);
                        }
                    }
                    return;
                }
                "tb-view" => {
                    eprintln!("[DISPATCH] tb-view toolbar button");
                    if let Some(toolbar_rect) = self.layout.rect_for_edge_slot("top-toolbar") {
                        let was_open = self.dropdown_view_state.open;
                        self.dropdown_file_state.close();
                        self.dropdown_help_state.close();
                        if was_open {
                            self.dropdown_view_state.close();
                        } else {
                            let view_x = toolbar_rect.x + 4.0 + 44.0;
                            self.dropdown_view_state.open_at(view_x, toolbar_rect.y + toolbar_rect.height);
                        }
                    }
                    return;
                }
                "tb-help" => {
                    eprintln!("[DISPATCH] tb-help toolbar button");
                    if let Some(toolbar_rect) = self.layout.rect_for_edge_slot("top-toolbar") {
                        let was_open = self.dropdown_help_state.open;
                        self.dropdown_file_state.close();
                        self.dropdown_view_state.close();
                        if was_open {
                            self.dropdown_help_state.close();
                        } else {
                            let help_x = toolbar_rect.x + 4.0 + 88.0;
                            self.dropdown_help_state.open_at(help_x, toolbar_rect.y + toolbar_rect.height);
                        }
                    }
                    return;
                }
                _ => {}
            }

            // Dropdown items — registered as "{dropdown_widget_id}:item:{item_id}"
            if let Some(item_id) = id_str.strip_prefix("dd-file-widget:item:") {
                eprintln!("[DISPATCH] dropdown file item → {item_id}");
                match item_id {
                    "file-quit" => { event_loop.exit(); return; }
                    "file-new"  => println!("[L3] File → New"),
                    "file-open" => println!("[L3] File → Open"),
                    "file-save" => println!("[L3] File → Save"),
                    _ => {}
                }
                self.dropdown_file_state.close();
                return;
            }
            if let Some(item_id) = id_str.strip_prefix("dd-view-widget:item:") {
                eprintln!("[DISPATCH] dropdown view item → {item_id}");
                // Toggle items: keep dropdown OPEN so user sees the new state.
                match item_id {
                    "view-sidebar" => { self.sidebar_open = !self.sidebar_open; }
                    "view-toolbar" => { self.left_toolbar_visible = !self.left_toolbar_visible; }
                    other          => {
                        println!("[L3] View → {other}");
                        self.dropdown_view_state.close();
                    }
                }
                return;
            }
            if let Some(item_id) = id_str.strip_prefix("dd-help-widget:item:") {
                eprintln!("[DISPATCH] dropdown modals item → {item_id}");
                let open_modal = |kind: ModalKind, this: &mut AppState| {
                    this.modal_open = true;
                    this.modal_kind = kind;
                    this.modal_state.position = (0.0, 0.0);
                };
                match item_id {
                    "modals-l2"     => open_modal(ModalKind::L2,   self),
                    "modals-l1"     => open_modal(ModalKind::L1,   self),
                    "modals-panels" => open_modal(ModalKind::Tags, self),
                    other           => println!("[L3] Modals → {other}"),
                }
                self.dropdown_help_state.close();
                return;
            }
            // Context menu items — registered as "{menu_widget_id}:item:{IDX}"
            if let Some(idx_str) = id_str.strip_prefix("ctx-menu-widget:item:") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    eprintln!("[DISPATCH] ctxmenu item idx={idx}");
                    match idx {
                        0 => println!("[L3] ctx → Copy"),
                        1 => println!("[L3] ctx → Paste"),
                        2 => println!("[L3] ctx → Delete"),
                        3 => println!("[L3] ctx → Properties"),
                        4 => {
                            // Fix #12: Settings via context menu
                            if self.modal_open && self.modal_kind == ModalKind::Settings {
                                self.modal_open = false;
                            } else {
                                self.modal_open = true;
                                self.modal_kind = ModalKind::Settings;
                                self.modal_state.position = (0.0, 0.0);
                            }
                        }
                        _ => {}
                    }
                    self.ctx_menu_state.close();
                    return;
                }
            }
            // Chrome tabs — registered as "tab-N" (composite chrome registers tabs)
            if let Some(n_str) = id_str.strip_prefix("tab-") {
                if let Ok(n) = n_str.parse::<usize>() {
                    eprintln!("[DISPATCH] chrome tab → {n}");
                    self.switch_tab(n);
                    println!("[L3] tab → {n}");
                    return;
                }
            }
            // ── L2-modal widgets (clicked inside L2 modal body) ──────────────
            match id_str {
                "l2-btn-connect" => {
                    eprintln!("[DISPATCH] l2-btn-connect");
                    self.l2_connected = !self.l2_connected;
                    return;
                }
                "l2-btn-close" => {
                    eprintln!("[DISPATCH] l2-btn-close");
                    self.modal_open = false;
                    return;
                }
                "l2-cb" => {
                    eprintln!("[DISPATCH] l2-cb");
                    self.l2_checked = !self.l2_checked;
                    return;
                }
                "l2-tog" => {
                    eprintln!("[DISPATCH] l2-tog");
                    self.l2_toggled = !self.l2_toggled;
                    return;
                }
                _ => {}
            }
            if let Some(n_str) = id_str.strip_prefix("l2-radio-") {
                if let Ok(n) = n_str.parse::<usize>() {
                    eprintln!("[DISPATCH] l2-radio → {n}");
                    self.l2_radio_sel = n;
                    return;
                }
            }
            if let Some(n_str) = id_str.strip_prefix("l2-swatch-") {
                if let Ok(n) = n_str.parse::<usize>() {
                    eprintln!("[DISPATCH] l2-swatch → {n}");
                    self.l2_swatch_sel = n;
                    return;
                }
            }
            if let Some(n_str) = id_str.strip_prefix("l2-sub-tab-") {
                if let Ok(n) = n_str.parse::<usize>() {
                    eprintln!("[DISPATCH] l2-sub-tab → {n}");
                    self.l2_active_sub_tab = n;
                    return;
                }
            }
            if let Some(n_str) = id_str.strip_prefix("l2-tab-") {
                if let Ok(n) = n_str.parse::<usize>() {
                    eprintln!("[DISPATCH] l2-tab → {n}");
                    self.l2_active_tab = n;
                    self.l2_scroll_off = 0.0;
                    return;
                }
            }
            // Sidebar: spawn kind radio buttons
            if let Some(kind_str) = id_str.strip_prefix("spawn-kind-") {
                eprintln!("[DISPATCH] spawn-kind → {kind_str}");
                self.spawn_kind = match kind_str {
                    "watchlist"   => PanelKind::Watchlist,
                    "spreadsheet" => PanelKind::Spreadsheet,
                    "notes"       => PanelKind::Notes,
                    "inbox"       => PanelKind::Inbox,
                    "tasks"       => PanelKind::Tasks,
                    "calendar"    => PanelKind::Calendar,
                    _             => PanelKind::Notes,
                };
                return;
            }
            // Sidebar: spawn split radio buttons
            match id_str {
                "spawn-split-horiz" => { self.spawn_split = SpawnSplit::SplitRight;  return; }
                "spawn-split-vert"  => { self.spawn_split = SpawnSplit::SplitBottom; return; }
                "spawn-split-grid"  => { self.spawn_split = SpawnSplit::Grid2x2;     return; }
                _ => {}
            }
            // Sidebar: Spawn button
            if id_str == "sidebar-spawn" {
                eprintln!("[DISPATCH] sidebar-spawn kind={:?} split={:?}", self.spawn_kind, self.spawn_split);
                let split_kind = match self.spawn_split {
                    SpawnSplit::SplitRight  => SplitKind::SplitRight,
                    SpawnSplit::SplitBottom => SplitKind::SplitBottom,
                    SpawnSplit::Grid2x2     => SplitKind::Grid2x2,
                };
                let new_panel = DemoPanel {
                    title: self.spawn_kind.title().to_string(),
                    kind: self.spawn_kind.clone(),
                };
                let active = self.layout.panels().active_leaf();
                if let Some(active_leaf) = active {
                    let new_ids = self.layout.panels_mut().tree_mut()
                        .split_leaf(active_leaf, split_kind, 0.0, 0.0);
                    if let Some(&new_id) = new_ids.last() {
                        if let Some(leaf) = self.layout.panels_mut().tree_mut().leaf_mut(new_id) {
                            leaf.panels.clear();
                            leaf.panels.push(new_panel);
                        }
                    }
                } else {
                    self.layout.panels_mut().tree_mut().add_leaf(new_panel);
                }
                return;
            }
            // Dock leaf click — set active leaf
            if let Some(idx_str) = id_str.strip_prefix("dock-leaf-") {
                if let Ok(n) = idx_str.parse::<u64>() {
                    eprintln!("[DISPATCH] dock-leaf-{n} → set active");
                    self.layout.panels_mut().set_active_leaf(uzor::docking::panels::LeafId(n));
                    return;
                }
            }
            // Sidebar: × close leaf button
            if let Some(idx_str) = id_str.strip_prefix("dock-leaf-close-") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    eprintln!("[DISPATCH] dock-leaf-close-{idx}");
                    let mut leaves: Vec<uzor::docking::panels::LeafId> = self
                        .layout
                        .panels()
                        .panel_rects()
                        .keys()
                        .copied()
                        .collect();
                    leaves.sort_by_key(|id| id.0);
                    if leaves.len() > 1 {
                        if let Some(&leaf_id) = leaves.get(idx) {
                            self.layout.panels_mut().tree_mut().remove_leaf(leaf_id);
                        }
                    }
                }
                return;
            }
            // L1-modal custom button
            if id_str == "l1-mybtn" {
                eprintln!("[DISPATCH] l1-mybtn");
                println!("[L3] L1 custom button clicked");
                return;
            }
            // Watchlist blackbox — row clicks dispatched by leaf id pattern
            if let Some(leaf_n_str) = id_str.strip_prefix("dock-leaf-") {
                // If leaf_n_str is a pure integer, it's a leaf click (handled above).
                // However "watchlist-bb" was the old id — keep compatibility by checking
                // if the clicked leaf contains a Watchlist kind.
                if let Ok(n) = leaf_n_str.parse::<u64>() {
                    let leaf_id = uzor::docking::panels::LeafId(n);
                    let is_watchlist = self.layout.panels().tree().leaf(leaf_id)
                        .and_then(|l| l.panels.first())
                        .map(|p| p.kind == PanelKind::Watchlist)
                        .unwrap_or(false);
                    if is_watchlist {
                        if let Some(&rect) = self.layout.panels().panel_rects().get(&leaf_id) {
                            let r = Rect::new(rect.x as f64, rect.y as f64, rect.width as f64, rect.height as f64);
                            let symbols = ["BTC/USDT", "ETH/USDT", "SOL/USDT", "BNB/USDT", "ADA/USDT"];
                            if let watchlist_blackbox::Hit::Row(row) = watchlist_blackbox::hit_test(&self.watchlist, r, x, y) {
                                if let Some(sym) = symbols.get(row) {
                                    println!("[L3] watchlist row clicked: {sym}");
                                }
                            }
                        }
                    }
                    self.layout.panels_mut().set_active_leaf(leaf_id);
                    return;
                }
            }
            // Left vertical toolbar items — registered as "left-vtoolbar-widget:<id>"
            if let Some(item) = id_str.strip_prefix("left-vtoolbar-widget:") {
                eprintln!("[DISPATCH] left toolbar → {item}");
                match item {
                    "lt-toggle-sidebar" => {
                        self.sidebar_open = !self.sidebar_open;
                        println!("[L3] sidebar → {}", self.sidebar_open);
                    }
                    _ => {}
                }
                return;
            }
            // Top toolbar items registered as "top-toolbar-widget:<id>"
            if let Some(item) = id_str.strip_prefix("top-toolbar-widget:") {
                eprintln!("[DISPATCH] top toolbar → {item}");
                if let Some(toolbar_rect) = self.layout.rect_for_edge_slot("top-toolbar") {
                    let dd_y = toolbar_rect.y + toolbar_rect.height;
                    match item {
                        "tb-file" => {
                            let was = self.dropdown_file_state.open;
                            self.dropdown_view_state.close();
                            self.dropdown_help_state.close();
                            if was { self.dropdown_file_state.close(); }
                            else   { self.dropdown_file_state.open_at(toolbar_rect.x + 4.0, dd_y); }
                        }
                        "tb-view" => {
                            let was = self.dropdown_view_state.open;
                            self.dropdown_file_state.close();
                            self.dropdown_help_state.close();
                            if was { self.dropdown_view_state.close(); }
                            else   { self.dropdown_view_state.open_at(toolbar_rect.x + 48.0, dd_y); }
                        }
                        "tb-help" => {
                            let was = self.dropdown_help_state.open;
                            self.dropdown_file_state.close();
                            self.dropdown_view_state.close();
                            if was { self.dropdown_help_state.close(); }
                            else   { self.dropdown_help_state.open_at(toolbar_rect.x + 92.0, dd_y); }
                        }
                        "tb-new" => println!("[L3] new"),
                        "tb-l2" => {
                            if self.modal_open && self.modal_kind == ModalKind::L2 {
                                self.modal_open = false;
                            } else {
                                self.modal_open = true;
                                self.modal_kind = ModalKind::L2;
                                self.modal_state.position = (0.0, 0.0);
                            }
                            return;
                        }
                        "tb-l1" => {
                            if self.modal_open && self.modal_kind == ModalKind::L1 {
                                self.modal_open = false;
                            } else {
                                self.modal_open = true;
                                self.modal_kind = ModalKind::L1;
                                self.modal_state.position = (0.0, 0.0);
                            }
                            return;
                        }
                        "tb-tags" => {
                            if self.modal_open && self.modal_kind == ModalKind::Tags {
                                self.modal_open = false;
                            } else {
                                self.modal_open = true;
                                self.modal_kind = ModalKind::Tags;
                                self.modal_state.position = (0.0, 0.0);
                            }
                            return;
                        }
                        _ => {}
                    }
                }
                return;
            }
            // Sidebar/chrome composite frames — no-op
            if id_str == "chrome" || id_str == "sidebar-widget"
                || id_str == "top-toolbar-widget" || id_str == "left-vtoolbar-widget"
            {
                eprintln!("[DISPATCH] composite frame click — no-op");
                return;
            }
            // Modal frame click WITHOUT hitting any child — modal stays open
            if id_str == "modal-widget" {
                eprintln!("[DISPATCH] modal frame click (kept open)");
                return;
            }
            // Else fall through to manual dispatch
            eprintln!("[DISPATCH] id={id_str} unmatched — falling through to manual geometry");
        } else {
            eprintln!("[FALLBACK] no priority match (clicked_id=None), trying manual geometry");
        }
        let _ = clicked_id; // silence unused warning if no fall-through uses it

        // ── Chrome hit ────────────────────────────────────────────────────────
        let tab_ids = ["tab-0", "tab-1", "tab-2"];
        let chrome_tabs = [
            ChromeTabConfig { id: "tab-0", label: "Dashboard",  icon: None, color_tag: None, closable: false, active: self.active_view == 0 },
            ChromeTabConfig { id: "tab-1", label: "Panels",     icon: None, color_tag: None, closable: false, active: self.active_view == 1 },
            ChromeTabConfig { id: "tab-2", label: "Monitoring", icon: None, color_tag: None, closable: false, active: self.active_view == 2 },
        ];
        let chrome_view = ChromeView { tabs: &chrome_tabs, active_tab_id: Some(tab_ids[self.active_view]), show_new_tab_btn: false, show_menu_btn: false, show_new_window_btn: false, show_close_window_btn: false, is_maximized: self.window.is_maximized(), cursor_x: x, cursor_y: y, time_ms: self.time_ms() };
        if let Some(chrome_rect) = self.layout.rect_for_chrome() {
            let hit = chrome_hit_test(&self.chrome_state, &chrome_view, &ChromeSettings::default(), &ChromeRenderKind::Default, chrome_rect, (x, y));
            match handle_chrome_action(hit) {
                ChromeAction::SelectTab(i) => {
                    self.switch_tab(i);
                    println!("[L3] tab → {i}");
                }
                ChromeAction::CloseApp => {
                    event_loop.exit();
                    return;
                }
                _ => {}
            }
        }

        // ── Toolbar buttons ───────────────────────────────────────────────────
        if let Some(toolbar_rect) = self.layout.rect_for_edge_slot("top-toolbar") {
            if x >= toolbar_rect.x && y >= toolbar_rect.y && y <= toolbar_rect.y + toolbar_rect.height {
                // Determine approximate button areas from toolbar left
                let file_x = toolbar_rect.x + 4.0;
                let view_x = file_x + 44.0;
                let help_x = view_x + 44.0;

                if x >= file_x && x < view_x {
                    let was_open = self.dropdown_file_state.open;
                    self.dropdown_view_state.close();
                    self.dropdown_help_state.close();
                    if was_open { self.dropdown_file_state.close(); } else { self.dropdown_file_state.open_at(file_x, toolbar_rect.y + toolbar_rect.height); }
                    return;
                }
                if x >= view_x && x < help_x {
                    let was_open = self.dropdown_view_state.open;
                    self.dropdown_file_state.close();
                    self.dropdown_help_state.close();
                    if was_open { self.dropdown_view_state.close(); } else { self.dropdown_view_state.open_at(view_x, toolbar_rect.y + toolbar_rect.height); }
                    return;
                }
                if x >= help_x && x < help_x + 44.0 {
                    let was_open = self.dropdown_help_state.open;
                    self.dropdown_file_state.close();
                    self.dropdown_view_state.close();
                    if was_open { self.dropdown_help_state.close(); } else { self.dropdown_help_state.open_at(help_x, toolbar_rect.y + toolbar_rect.height); }
                    return;
                }
            }
        }

        // ── Left toolbar: sidebar toggle ──────────────────────────────────────
        if let Some(lt_rect) = self.layout.rect_for_edge_slot("left-vtoolbar") {
            if x >= lt_rect.x && x <= lt_rect.x + lt_rect.width {
                // First button area (top ~44px after chrome + top toolbar)
                let btn_y = lt_rect.y + 4.0;
                if y >= btn_y && y <= btn_y + 36.0 {
                    self.sidebar_open = !self.sidebar_open;
                    println!("[L3] sidebar → {}", self.sidebar_open);
                    return;
                }
            }
        }

        // ── Dropdown item clicks ──────────────────────────────────────────────
        let any_dd_open = self.dropdown_file_state.open || self.dropdown_view_state.open || self.dropdown_help_state.open || self.dropdown_addpanel_state.open;
        if any_dd_open {
            let clicked_dd =
                (self.dropdown_file_state.open && self.layout.rect_for_overlay("dd-file-overlay").map(|r| r.contains(x, y)).unwrap_or(false)) ||
                (self.dropdown_view_state.open && self.layout.rect_for_overlay("dd-view-overlay").map(|r| r.contains(x, y)).unwrap_or(false)) ||
                (self.dropdown_help_state.open && self.layout.rect_for_overlay("dd-help-overlay").map(|r| r.contains(x, y)).unwrap_or(false)) ||
                (self.dropdown_addpanel_state.open && self.layout.rect_for_overlay("dd-addpanel-overlay").map(|r| r.contains(x, y)).unwrap_or(false));
            if !clicked_dd {
                self.dropdown_file_state.close();
                self.dropdown_view_state.close();
                self.dropdown_help_state.close();
                self.dropdown_addpanel_state.close();
            } else {
                // Handle specific items via hovered_id
                if let Some(ref hid) = self.dropdown_file_state.hovered_id.clone() {
                    match hid.as_str() {
                        "file-quit" => event_loop.exit(),
                        "file-new"  => println!("[L3] New"),
                        "file-open" => println!("[L3] Open"),
                        "file-save" => println!("[L3] Save"),
                        _ => {}
                    }
                    self.dropdown_file_state.close();
                }
                if let Some(ref hid) = self.dropdown_view_state.hovered_id.clone() {
                    // Toggle items — keep dropdown OPEN.
                    match hid.as_str() {
                        "view-sidebar" => { self.sidebar_open = !self.sidebar_open; }
                        "view-toolbar" => { self.left_toolbar_visible = !self.left_toolbar_visible; }
                        _ => {
                            println!("[L3] view item: {hid}");
                            self.dropdown_view_state.close();
                        }
                    }
                }
                if let Some(ref hid) = self.dropdown_help_state.hovered_id.clone() {
                    let open_modal = |kind: ModalKind, this: &mut AppState| {
                        this.modal_open = true;
                        this.modal_kind = kind;
                        this.modal_state.position = (0.0, 0.0);
                    };
                    match hid.as_str() {
                        "modals-l2"     => open_modal(ModalKind::L2,   self),
                        "modals-l1"     => open_modal(ModalKind::L1,   self),
                        "modals-panels" => open_modal(ModalKind::Tags, self),
                        _               => println!("[L3] modals item: {hid}"),
                    }
                    self.dropdown_help_state.close();
                }
            }
            return;
        }

        // ── Modal: dismiss on outside click ───────────────────────────────────
        if self.modal_open {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                if !modal_rect.contains(x, y) {
                    self.modal_open = false;
                }
            }
        }

        // ── Context menu: dismiss on outside click ────────────────────────────
        if self.ctx_menu_state.is_open {
            if let Some(menu_rect) = self.layout.rect_for_overlay("ctx-menu-overlay") {
                if !menu_rect.contains(x, y) {
                    self.ctx_menu_state.close();
                } else {
                    // Item dispatch is handled via the coordinator dispatch above
                    self.ctx_menu_state.close();
                }
            }
        }

        // ── L2 modal widget clicks ────────────────────────────────────────────
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                // body starts after header (~40px) + inside padding
                let body_y = modal_rect.y + 44.0; // modal header height = 44px (Fix 2)
                let rel_x = x - modal_rect.x;
                let rel_y = y - body_y;
                match self.l2_hovered_at(rel_x, rel_y) {
                    Some(ref id) if id == "l2-btn-connect" => { self.l2_connected = !self.l2_connected; }
                    Some(ref id) if id == "l2-cb" => { self.l2_checked = !self.l2_checked; }
                    Some(ref id) if id == "l2-tog" => { self.l2_toggled = !self.l2_toggled; }
                    Some(ref id) if id.starts_with("l2-radio-") => {
                        if let Ok(n) = id["l2-radio-".len()..].parse::<usize>() { self.l2_radio_sel = n; }
                    }
                    Some(ref id) if id.starts_with("l2-swatch-") => {
                        if let Ok(n) = id["l2-swatch-".len()..].parse::<usize>() { self.l2_swatch_sel = n; }
                    }
                    Some(ref id) if id.starts_with("l2-tab-") => {
                        if let Ok(n) = id["l2-tab-".len()..].parse::<usize>() {
                            self.l2_active_tab = n;
                            self.l2_scroll_off = 0.0;
                        }
                    }
                    Some(ref id) if id.starts_with("l2-sub-tab-") => {
                        if let Ok(n) = id["l2-sub-tab-".len()..].parse::<usize>() { self.l2_active_sub_tab = n; }
                    }
                    _ => {}
                }
            }
        }
    }

    fn on_right_up(&mut self, x: f64, y: f64) {
        eprintln!("[RIGHT_UP] pos=({:.1},{:.1})", x, y);
        let (w, h) = { let s = &self.surface; (s.config.width as f64, s.config.height as f64) };
        self.ctx_menu_state.open_smart(x, y, w, h, 170.0, 156.0, None);
        self.dropdown_file_state.close();
        self.dropdown_view_state.close();
        self.dropdown_help_state.close();
    }

    fn on_mouse_down(&mut self, x: f64, y: f64) {
        self.last_mouse = (x, y);
        self.mouse_down = true;
        self.drag_origin = Some((x, y));

        // Fix 3: modal header drag — start drag when clicking in modal header zone.
        // Header zone: modal top .. modal top + 44px, minus close-button right 34px.
        if self.modal_open {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let header_rect = Rect::new(
                    modal_rect.x,
                    modal_rect.y,
                    modal_rect.width - 34.0, // leave close-button area (24px + 10px padding)
                    44.0,
                );
                if header_rect.contains(x, y) {
                    // resolve current modal origin (may have been dragged before)
                    let origin = if self.modal_state.position != (0.0, 0.0) {
                        self.modal_state.position
                    } else {
                        (modal_rect.x, modal_rect.y)
                    };
                    self.modal_state.start_drag((x, y), origin);
                    self.drag_target = Some(DragTarget::ModalDrag);
                    return;
                }
            }
        }

        // Dock separator drag (fix 6)
        if !self.modal_open {
            let sep_hit: Option<usize> = self.layout.panels().separators().iter().enumerate()
                .find(|(_, s)| s.hit_test(x as f32, y as f32))
                .map(|(i, _)| i);
            if let Some(sep_idx) = sep_hit {
                self.drag_target = Some(DragTarget::SeparatorDrag { sep_idx, start_x: x, start_y: y });
                return;
            }
        }

        // Fix 2: track L1 button pressed state
        if self.modal_open && self.modal_kind == ModalKind::L1 {
            let hovered_id = self.layout.ctx_mut().input.hovered_widget().map(|id| id.0.clone());
            self.l1_btn_pressed = hovered_id.as_deref() == Some("l1-mybtn");
        }

        // Route mouse-down to watchlist blackbox if the active leaf is a Watchlist
        if !self.modal_open {
            let watchlist_leaf = self.layout.panels().panel_rects().iter()
                .find(|(&id, _)| {
                    self.layout.panels().tree().leaf(id)
                        .and_then(|l| l.panels.first())
                        .map(|p| p.kind == PanelKind::Watchlist)
                        .unwrap_or(false)
                })
                .map(|(&id, &rect)| (id, rect));
            if let Some((_leaf_id, panel_rect)) = watchlist_leaf {
                let r = Rect::new(panel_rect.x as f64, panel_rect.y as f64, panel_rect.width as f64, panel_rect.height as f64);
                watchlist_blackbox::on_mouse_down(&mut self.watchlist, r, x, y);
                if self.watchlist.drag_sep.is_some() {
                    return;
                }
            }
        }

        // L2 drag targets
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let body_y = modal_rect.y + 44.0; // modal header height = 44px (Fix 2)
                let rel_x = x - modal_rect.x;
                let rel_y = y - body_y;
                self.l2_pressed = self.l2_hovered_at(rel_x, rel_y);
                self.l2_hovered = self.l2_pressed.clone();
                let target = match self.l2_hovered.as_deref() {
                    Some("l2-slider")   => Some(DragTarget::L2Slider(self.l2_slider_val)),
                    Some("l2-splitter") => Some(DragTarget::L2Splitter(self.l2_right_panel_w)),
                    Some("l2-range") => {
                        let x_min = RANGE_RECT.x + (self.l2_range_min / 100.0) * RANGE_RECT.width;
                        let x_max = RANGE_RECT.x + (self.l2_range_max / 100.0) * RANGE_RECT.width;
                        if (rel_x - x_min).abs() <= (rel_x - x_max).abs() {
                            self.l2_range_drag_handle = Some(DualSliderHandle::Min);
                            Some(DragTarget::L2RangeMin(self.l2_range_min))
                        } else {
                            self.l2_range_drag_handle = Some(DualSliderHandle::Max);
                            Some(DragTarget::L2RangeMax(self.l2_range_max))
                        }
                    }
                    Some("l2-sb") | Some("l2-sb-track") | Some("l2-sb-thumb") => Some(DragTarget::L2Scroll(self.l2_scroll_off)),
                    _ => None,
                };
                self.drag_target = target;
            }
        }
    }

    fn on_mouse_move(&mut self, x: f64, y: f64) {
        self.last_mouse = (x, y);

        // Fix #2/#13: update chrome hover state so buttons visually highlight
        {
            let tab_ids = ["tab-0", "tab-1", "tab-2"];
            let chrome_tabs_mv = [
                ChromeTabConfig { id: "tab-0", label: "Dashboard",  icon: None, color_tag: None, closable: false, active: self.active_view == 0 },
                ChromeTabConfig { id: "tab-1", label: "Panels",     icon: None, color_tag: None, closable: false, active: self.active_view == 1 },
                ChromeTabConfig { id: "tab-2", label: "Monitoring", icon: None, color_tag: None, closable: false, active: self.active_view == 2 },
            ];
            let chrome_view_mv = ChromeView {
                tabs: &chrome_tabs_mv,
                active_tab_id: Some(tab_ids[self.active_view]),
                show_new_tab_btn: false,
                show_menu_btn: false,
                show_new_window_btn: false,
                show_close_window_btn: false,
                is_maximized: self.window.is_maximized(),
                cursor_x: x,
                cursor_y: y,
                time_ms: self.time_ms(),
            };
            if let Some(chrome_rect) = self.layout.rect_for_chrome() {
                let hit = chrome_hit_test(
                    &self.chrome_state,
                    &chrome_view_mv,
                    &ChromeSettings::default(),
                    &ChromeRenderKind::Default,
                    chrome_rect,
                    (x, y),
                );
                // Update per-tab hover state before storing hit
                let tab_hovered = if let ChromeHit::Tab(j) = hit { Some(j) } else { None };
                let tab_close_hovered = if let ChromeHit::CloseTab(j) = hit { Some(j) } else { None };
                for (i, ts) in self.chrome_state.tabs_state.iter_mut().enumerate() {
                    ts.hovered = tab_hovered == Some(i);
                    ts.close_hovered = tab_close_hovered == Some(i);
                }
                self.chrome_state.hovered = hit;
            }
        }

        // Fix 2: update L1 button hover state from coordinator hovered widget
        {
            let hovered_id = self.layout.ctx_mut().input.hovered_widget().map(|id| id.0.clone());
            self.l1_btn_hovered = hovered_id.as_deref() == Some("l1-mybtn");
        }

        // Route mouse-move into watchlist blackbox
        if !self.modal_open {
            let watchlist_rect = self.layout.panels().panel_rects().iter()
                .find(|(&id, _)| {
                    self.layout.panels().tree().leaf(id)
                        .and_then(|l| l.panels.first())
                        .map(|p| p.kind == PanelKind::Watchlist)
                        .unwrap_or(false)
                })
                .map(|(_, &r)| Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64));
            if let Some(r) = watchlist_rect {
                watchlist_blackbox::on_mouse_move(&mut self.watchlist, r, x, y);
            }
        }

        // Fix #13: update toolbar hover state (strip parent prefix so render matches plain item id)
        {
            let hovered_id = self.layout.ctx_mut().input.hovered_widget().map(|id| id.0.clone());
            self.top_toolbar_state.hovered_item_id = hovered_id
                .as_ref()
                .and_then(|id| id.strip_prefix("top-toolbar-widget:"))
                .map(|s| s.to_string());
            self.left_vtoolbar_state.hovered_item_id = hovered_id
                .as_ref()
                .and_then(|id| id.strip_prefix("left-vtoolbar-widget:"))
                .map(|s| s.to_string());
            let hovered_id = hovered_id;
            if self.dropdown_file_state.open || self.dropdown_view_state.open || self.dropdown_help_state.open {
                self.dropdown_file_state.hovered_id = hovered_id.clone()
                    .filter(|id| id.starts_with("dd-file-widget:item:"))
                    .map(|id| id["dd-file-widget:item:".len()..].to_owned());
                self.dropdown_view_state.hovered_id = hovered_id.clone()
                    .filter(|id| id.starts_with("dd-view-widget:item:"))
                    .map(|id| id["dd-view-widget:item:".len()..].to_owned());
                self.dropdown_help_state.hovered_id = hovered_id.clone()
                    .filter(|id| id.starts_with("dd-help-widget:item:"))
                    .map(|id| id["dd-help-widget:item:".len()..].to_owned());
            }
            if self.dropdown_addpanel_state.open {
                self.dropdown_addpanel_state.hovered_id = hovered_id.clone()
                    .filter(|id| id.starts_with("dd-addpanel-widget:item:"))
                    .map(|id| id["dd-addpanel-widget:item:".len()..].to_owned());
            }
            if self.ctx_menu_state.is_open {
                if let Some(ref id) = hovered_id {
                    if let Some(idx_str) = id.strip_prefix("ctx-menu-widget:item:") {
                        self.ctx_menu_state.hovered_index = idx_str.parse().ok();
                    } else {
                        self.ctx_menu_state.hovered_index = None;
                    }
                } else {
                    self.ctx_menu_state.hovered_index = None;
                }
            }
        }

        // Fix 3: modal drag — update position while dragging modal header
        if matches!(self.drag_target, Some(DragTarget::ModalDrag)) {
            let (sw, sh) = { let s = &self.surface; (s.config.width as f64, s.config.height as f64) };
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                handle_modal_drag(&mut self.modal_state, (x, y), (sw, sh), (modal_rect.width, modal_rect.height));
            }
            return;
        }

        // L2 hover tracking
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let body_y = modal_rect.y + 44.0; // modal header height = 44px (Fix 2)
                let rel_x = x - modal_rect.x;
                let rel_y = y - body_y;
                self.l2_hovered = self.l2_hovered_at(rel_x, rel_y);
            }
        }

        // Drag
        // Separator drag (fix 6) — handle before the shared drag block to avoid borrow conflicts
        if let Some(DragTarget::SeparatorDrag { sep_idx, start_x: ref mut sx, start_y: ref mut sy }) = self.drag_target {
            let delta_x = (x - *sx) as f32;
            let delta_y = (y - *sy) as f32;
            *sx = x;
            *sy = y;
            if let Some(dock_area) = self.layout.rect_for_dock_area() {
                let cw = dock_area.width as f32;
                let ch = dock_area.height as f32;
                use uzor::docking::panels::SeparatorOrientation as DockSepOrient;
                let orient = self.layout.panels().separators().get(sep_idx).map(|s| s.orientation);
                let delta = match orient {
                    Some(DockSepOrient::Vertical)   => delta_x,
                    Some(DockSepOrient::Horizontal) => delta_y,
                    None => 0.0,
                };
                self.layout.panels_mut().drag_separator(sep_idx, delta, cw, ch);
            }
        }

        if let (Some((ox, oy)), Some(ref target)) = (self.drag_origin, self.drag_target.as_ref()) {
            let dx = x - ox;
            let dy = y - oy;
            match target {
                DragTarget::ModalDrag => {} // handled above
                DragTarget::SeparatorDrag { .. } => {} // handled above
                DragTarget::L2Slider(v0) => {
                    let frac = dx / SLID_RECT.width;
                    self.l2_slider_val = (v0 + frac * 100.0).clamp(0.0, 100.0);
                }
                DragTarget::L2RangeMin(v0) => {
                    let frac = dx / RANGE_RECT.width;
                    self.l2_range_min = (v0 + frac * 100.0).clamp(0.0, self.l2_range_max);
                }
                DragTarget::L2RangeMax(v0) => {
                    let frac = dx / RANGE_RECT.width;
                    self.l2_range_max = (v0 + frac * 100.0).clamp(self.l2_range_min, 100.0);
                }
                DragTarget::L2Scroll(v0) => {
                    let sr = SB_H - AppState::l2_thumb_h();
                    if sr > 0.0 {
                        self.l2_scroll_off = (v0 + dy / sr * (CONTENT_H - SB_H)).clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                    }
                }
                DragTarget::L2Splitter(w0) => {
                    self.l2_right_panel_w = (w0 - dx).clamp(200.0, L2_WIN_W - 100.0);
                }
            }
        }
    }

    fn on_mouse_up(&mut self) {
        self.mouse_down = false;
        self.drag_origin = None;
        // Fix 3: end modal drag
        self.modal_state.end_drag();
        self.drag_target = None;
        self.l2_range_drag_handle = None;
        self.l2_pressed = None;
        // Fix 2: clear L1 press state
        self.l1_btn_pressed = false;
        // Fix 3: clear watchlist drag
        watchlist_blackbox::on_mouse_up(&mut self.watchlist);
    }
}

// =============================================================================
// Layout setup
// =============================================================================

/// Build initial dock layout for tab 0 (Dashboard): Watchlist split with Notes.
/// Returns (tab_0_tree, tab_1_tree, tab_2_tree).
fn build_initial_trees() -> [uzor::docking::panels::DockingTree<DemoPanel>; 3] {
    use uzor::docking::panels::DockingTree;

    // Tab 0: Watchlist | Notes side-by-side
    let mut tree0: DockingTree<DemoPanel> = DockingTree::new();
    let leaf_a = tree0.add_leaf(DemoPanel { title: "Watchlist".into(), kind: PanelKind::Watchlist });
    let ids = tree0.split_leaf(leaf_a, SplitKind::SplitRight, 0.0, 0.0);
    if let Some(&new_id) = ids.last() {
        if let Some(leaf) = tree0.leaf_mut(new_id) {
            leaf.panels.clear();
            leaf.panels.push(DemoPanel { title: "Notes".into(), kind: PanelKind::Notes });
        }
    }

    // Tab 1: single Spreadsheet leaf
    let mut tree1: DockingTree<DemoPanel> = DockingTree::new();
    tree1.add_leaf(DemoPanel { title: "Spreadsheet".into(), kind: PanelKind::Spreadsheet });

    // Tab 2: single Metrics leaf
    let mut tree2: DockingTree<DemoPanel> = DockingTree::new();
    tree2.add_leaf(DemoPanel { title: "Metrics".into(), kind: PanelKind::Inbox });

    [tree0, tree1, tree2]
}

fn setup_dock(layout: &mut LayoutManager<DemoPanel>, trees: &mut [uzor::docking::panels::DockingTree<DemoPanel>; 3]) {
    // Install tab 0 tree into the layout manager
    let tree0 = std::mem::take(&mut trees[0]);
    *layout.panels_mut().tree_mut() = tree0;
}

// =============================================================================
// winit ApplicationHandler
// =============================================================================

struct Handler {
    state: Option<AppState>,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }

        let attrs = Window::default_attributes()
            .with_title("uzor L3 — Dashboard")
            .with_inner_size(winit::dpi::LogicalSize::new(WIN_W, WIN_H))
            .with_resizable(true)
            .with_decorations(false); // uzor chrome draws its own titlebar
        let window = Arc::new(event_loop.create_window(attrs).expect("window creation should succeed"));

        let mut render_cx = VelloRenderCx::new();
        let size = window.inner_size();
        let surface: RenderSurface<'static> = pollster::block_on(async {
            render_cx.create_surface(Arc::clone(&window), size.width.max(1), size.height.max(1), wgpu::PresentMode::AutoVsync)
                .await.expect("vello surface creation should succeed")
        });
        let renderer = Renderer::new(
            &render_cx.devices[surface.dev_id].device,
            RendererOptions { antialiasing_support: vello::AaSupport::area_only(), num_init_threads: None, ..RendererOptions::default() },
        ).expect("renderer creation should succeed");

        let mut layout = LayoutManager::<DemoPanel>::new();
        let mut tab_trees = build_initial_trees();
        setup_dock(&mut layout, &mut tab_trees);

        let mut chrome_state = ChromeState::new();
        chrome_state.sync_tabs(&["tab-0", "tab-1", "tab-2"]);
        chrome_state.active_tab_id = Some("tab-0".into());

        let mut sidebar_state = SidebarState::default();
        sidebar_state.width = SIDEBAR_OPEN_W;

        window.request_redraw();

        self.state = Some(AppState {
            window,
            render_cx,
            surface,
            renderer,
            scene: Scene::new(),
            layout,
            bridge: WinitInputBridge::new(),
            start: Instant::now(),
            last_clock_tick: Instant::now(),
            clock_str: "00:00:00".into(),
            chrome_state,
            top_toolbar_state: ToolbarState::default(),
            left_vtoolbar_state: ToolbarState::default(),
            sidebar_state,
            modal_state: ModalState::default(),
            popup_state: PopupState::default(),
            ctx_menu_state: ContextMenuState::default(),
            dropdown_file_state: DropdownState::default(),
            dropdown_view_state: DropdownState::default(),
            dropdown_help_state: DropdownState::default(),
            dropdown_addpanel_state: DropdownState::default(),
            active_view: 0,
            sidebar_open: true,
            left_toolbar_visible: true,
            modal_open: false,
            modal_kind: ModalKind::L2,
            popup_item: None,
            l2_connected: false,
            l2_checked: true,
            l2_toggled: true,
            l2_radio_sel: 1,
            l2_slider_val: 40.0,
            l2_range_min: 20.0,
            l2_range_max: 80.0,
            l2_range_drag_handle: None,
            l2_scroll_off: 0.0,
            l2_swatch_sel: 0,
            l2_active_tab: 0,
            l2_active_sub_tab: 0,
            l2_right_panel_w: 330.0,
            l2_hovered: None,
            l2_pressed: None,
            last_mouse: (0.0, 0.0),
            mouse_down: false,
            drag_origin: None,
            drag_target: None,
            tab_trees,
            spawn_kind: PanelKind::Notes,
            spawn_split: SpawnSplit::SplitRight,
            exit_requested: false,
            l1_btn_hovered: false,
            l1_btn_pressed: false,
            watchlist: watchlist_blackbox::WatchlistState::default(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(ref mut app) = self.state else { return };

        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                app.render_cx.resize_surface(&mut app.surface, w, h);
                app.window.request_redraw();
                return;
            }
            WindowEvent::RedrawRequested => {
                app.render(event_loop);
                return;
            }
            _ => {}
        }

        // Fix 1: chromeless chrome — handle drag/min/max/close on LMB press
        // Must run BEFORE the bridge so drag_window() is called while the button
        // is still held (winit requires it to be called within the press handler).
        if let WindowEvent::MouseInput { state: ElementState::Pressed, button: winit::event::MouseButton::Left, .. } = &event {
            let (mx, my) = app.bridge.last_mouse_pos;
            let tab_ids = ["tab-0", "tab-1", "tab-2"];
            let chrome_tabs_tmp = [
                ChromeTabConfig { id: "tab-0", label: "Dashboard",  icon: None, color_tag: None, closable: false, active: app.active_view == 0 },
                ChromeTabConfig { id: "tab-1", label: "Panels",     icon: None, color_tag: None, closable: false, active: app.active_view == 1 },
                ChromeTabConfig { id: "tab-2", label: "Monitoring", icon: None, color_tag: None, closable: false, active: app.active_view == 2 },
            ];
            let chrome_view_tmp = ChromeView { tabs: &chrome_tabs_tmp, active_tab_id: Some(tab_ids[app.active_view]), show_new_tab_btn: false, show_menu_btn: false, show_new_window_btn: false, show_close_window_btn: false, is_maximized: app.window.is_maximized(), cursor_x: mx, cursor_y: my, time_ms: app.time_ms() };
            if let Some(chrome_rect) = app.layout.rect_for_chrome() {
                let hit = chrome_hit_test(&app.chrome_state, &chrome_view_tmp, &ChromeSettings::default(), &ChromeRenderKind::Default, chrome_rect, (mx, my));
                match handle_chrome_action(hit) {
                    ChromeAction::WindowDragStart => {
                        let _ = app.window.drag_window();
                        return;
                    }
                    ChromeAction::Minimize => {
                        app.window.set_minimized(true);
                        return;
                    }
                    ChromeAction::MaximizeRestore => {
                        app.window.set_maximized(!app.window.is_maximized());
                        return;
                    }
                    ChromeAction::CloseApp => {
                        app.exit_requested = true;
                        app.window.request_redraw();
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Bridge handles text-field key routing + clipboard
        let focused = app.layout.ctx_mut().input.focused_widget().cloned();

        // Debug: print MouseInput / MouseWheel before bridge
        match &event {
            WindowEvent::MouseInput { state, button, .. } => {
                eprintln!("[WINIT] MouseInput state={:?} button={:?} pos={:?}", state, button, app.bridge.last_mouse_pos);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                eprintln!("[WINIT] MouseWheel delta={:?} pos={:?}", delta, app.bridge.last_mouse_pos);
            }
            _ => {}
        }

        let out = app.bridge.handle_event(&mut app.layout.ctx_mut().input, focused.as_ref(), &event);

        // Debug: print bridge output on mouse events
        if out.left_down.is_some() || out.left_up.is_some() || out.right_up.is_some() || out.wheel.is_some() {
            eprintln!("[BRIDGE] ldown={:?} lup={:?} rup={:?} wheel={:?}",
                out.left_down,
                out.left_up.as_ref().map(|((x, y), id)| (x, y, id.as_ref().map(|i| i.0.as_str().to_owned()))),
                out.right_up,
                out.wheel,
            );
        }

        if out.cursor_moved.is_some() || out.text_changed || out.focus_cleared {
            app.window.request_redraw();
        }

        // Left mouse down
        if let Some((x, y)) = out.left_down {
            app.on_mouse_down(x, y);
            app.window.request_redraw();
        }

        // Left mouse up
        if let Some(((x, y), clicked_id)) = out.left_up {
            app.on_mouse_up();
            app.on_left_up(x, y, clicked_id, event_loop);
            app.window.request_redraw();
        }

        // Right mouse up
        if let Some((x, y)) = out.right_up {
            app.on_right_up(x, y);
            app.window.request_redraw();
        }

        // Cursor move
        if let Some((x, y)) = out.cursor_moved {
            app.on_mouse_move(x, y);
            app.window.request_redraw();
        }

        // Watchlist blackbox scroll
        if let Some(((cx, cy), (_, dy))) = out.wheel {
            if !app.modal_open {
                let watchlist_rect = app.layout.panels().panel_rects().iter()
                    .find(|(&id, _)| {
                        app.layout.panels().tree().leaf(id)
                            .and_then(|l| l.panels.first())
                            .map(|p| p.kind == PanelKind::Watchlist)
                            .unwrap_or(false)
                    })
                    .map(|(_, &r)| Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64));
                if let Some(r) = watchlist_rect {
                    if r.contains(cx, cy) {
                        watchlist_blackbox::on_wheel(&mut app.watchlist, r, dy);
                        app.window.request_redraw();
                    }
                }
            }
        }

        // Fix 5: Wheel routing per sub-panel — only scroll right panel if cursor
        // is over the right panel. Left panel wheel is ignored.
        if let Some(((cx, cy), (_, dy))) = out.wheel {
            if app.modal_open && app.modal_kind == ModalKind::L2 && app.l2_active_tab == 0 {
                if let Some(modal_rect) = app.layout.rect_for_overlay("modal-overlay") {
                    let frame_x = if app.modal_state.position != (0.0, 0.0) { app.modal_state.position.0 } else { modal_rect.x };
                    let frame_y = if app.modal_state.position != (0.0, 0.0) { app.modal_state.position.1 } else { modal_rect.y };
                    let body_y  = frame_y + 44.0; // header height
                    let body_h  = modal_rect.height - 44.0 - 52.0;
                    // right panel starts at frame_x + (L2_WIN_W - l2_right_panel_w)
                    let rp_screen_x = frame_x + L2_WIN_W - app.l2_right_panel_w;
                    let right_panel_rect = Rect::new(
                        rp_screen_x,
                        body_y + 12.0,
                        app.l2_right_panel_w - 12.0,
                        body_h - 24.0,
                    );
                    if right_panel_rect.contains(cx, cy) {
                        app.l2_scroll_off = (app.l2_scroll_off - dy * 20.0)
                            .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                    }
                }
            }
            app.window.request_redraw();
        }

        // ESC closes modal / dropdown / context menu
        if let WindowEvent::KeyboardInput { event: ke, .. } = &event {
            if ke.state == ElementState::Pressed {
                use winit::keyboard::{Key, NamedKey};
                if let Key::Named(NamedKey::Escape) = ke.logical_key {
                    if app.modal_open {
                        app.modal_open = false;
                    } else if app.ctx_menu_state.is_open {
                        app.ctx_menu_state.close();
                    } else {
                        app.dropdown_file_state.close();
                        app.dropdown_view_state.close();
                        app.dropdown_help_state.close();
                        app.dropdown_addpanel_state.close();
                    }
                    app.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref app) = self.state {
            app.window.request_redraw();
        }
    }
}

// =============================================================================
// main
// =============================================================================

// On Windows, allocate a console window so eprintln debug output is visible
// when running the example via `cargo run` from a non-console context (or
// when launched as a GUI app).  No-op on non-Windows.
#[cfg(target_os = "windows")]
fn ensure_debug_console() {
    use std::os::raw::c_int;
    extern "system" {
        fn AllocConsole() -> c_int;
        fn AttachConsole(process_id: u32) -> c_int;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    unsafe {
        // Try to attach to parent's console first; if none, allocate a new one.
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            AllocConsole();
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ensure_debug_console() {}

// =============================================================================
// === BLACKBOX PANEL DEMO === Watchlist
// === Everything about this panel is self-contained: state, rendering,
// === hit-testing, dispatch. uzor sees it only as a single Blackbox rect.
// =============================================================================

mod watchlist_blackbox {
    use uzor::render::{RenderContext, TextAlign, TextBaseline};
    use uzor::types::Rect;

    pub struct WatchlistState {
        pub col_widths:  [f64; 4],
        pub scroll_off:  f64,
        pub hovered_row: Option<usize>,
        pub hovered_sep: Option<usize>,
        /// (sep_idx, start_frac, widths_at_drag_start) — start_frac is cursor x as fraction of rect.width
        pub drag_sep: Option<(usize, f64, [f64; 4])>,
    }

    impl Default for WatchlistState {
        fn default() -> Self {
            Self {
                col_widths:  [0.25, 0.30, 0.20, 0.25],
                scroll_off:  0.0,
                hovered_row: None,
                hovered_sep: None,
                drag_sep:    None,
            }
        }
    }

    // ── Render ─────────────────────────────────────────────────────────────────

    /// One sub-container inside the blackbox panel.
    /// Owns its own rect. Text rendered inside is positioned relative to THIS rect,
    /// never relative to the parent blackbox rect.
    struct Cell {
        rect:  Rect,
        text:  &'static str,
        color: &'static str,
    }

    impl Cell {
        fn draw(&self, render: &mut dyn RenderContext) {
            const PAD_X: f64 = 8.0;
            render.save();
            render.clip_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height);
            render.set_fill_color(self.color);
            render.set_text_align(TextAlign::Left);
            render.set_text_baseline(TextBaseline::Middle);
            // Text anchored to THIS cell's rect — pad from cell's own left edge,
            // baseline at cell's own vertical center.
            render.fill_text(
                self.text,
                self.rect.x + PAD_X,
                self.rect.y + self.rect.height / 2.0,
            );
            render.restore();
        }
    }

    pub fn render(state: &WatchlistState, render: &mut dyn RenderContext, rect: Rect) {
        const HEADER_H: f64 = 28.0;
        const ROW_H:    f64 = 24.0;

        // Background
        render.set_fill_color("#1a1a22");
        render.fill_rect(rect.x, rect.y, rect.width, rect.height);

        // Column header strip
        render.set_fill_color("#252530");
        render.fill_rect(rect.x, rect.y, rect.width, HEADER_H);

        let titles: [&str; 4] = ["SYMBOL", "PRICE", "Δ", "VOL"];
        // Compute column boundary x positions
        let mut col_xs = [rect.x; 5];
        let mut cx = rect.x;
        for i in 0..4 {
            col_xs[i] = cx;
            cx += rect.width * state.col_widths[i];
        }
        col_xs[4] = cx;

        // Build header sub-containers (one per column between separators).
        // Each Cell owns its own rect — text positions itself by THIS rect.
        for (i, title) in titles.iter().enumerate() {
            let cell = Cell {
                rect: Rect::new(
                    col_xs[i],
                    rect.y,
                    col_xs[i + 1] - col_xs[i],
                    HEADER_H,
                ),
                text:  title,
                color: "#a0a0a8",
            };
            cell.draw(render);
        }

        // Vertical separator lines (highlight on hover / drag)
        for i in 0..3 {
            let sep_x = col_xs[i + 1];
            let dragging = state.drag_sep.map(|(idx, _, _)| idx == i).unwrap_or(false);
            let color = if dragging || state.hovered_sep == Some(i) {
                "#4080ff"
            } else {
                "rgba(255,255,255,0.12)"
            };
            render.set_fill_color(color);
            render.fill_rect(sep_x - 0.5, rect.y, 1.0, rect.height);
        }

        // Column-header bottom divider
        render.set_fill_color("rgba(255,255,255,0.08)");
        render.fill_rect(rect.x, rect.y + HEADER_H - 1.0, rect.width, 1.0);

        // Rows
        let symbols: &[(&str, &str, &str, &str)] = &[
            ("BTC/USDT", "67,234.5", "+1.2%", "12.4M"),
            ("ETH/USDT",  "3,421.8", "-0.8%",  "8.2M"),
            ("SOL/USDT",   "182.3",  "+3.4%",  "5.1M"),
            ("BNB/USDT",   "612.7",  "+0.5%",  "2.8M"),
            ("ADA/USDT",     "0.45", "-1.1%",  "1.5M"),
        ];
        for (row, (sym, price, delta, vol)) in symbols.iter().enumerate() {
            let y = rect.y + HEADER_H + row as f64 * ROW_H - state.scroll_off;
            if y + ROW_H < rect.y + HEADER_H || y > rect.y + rect.height {
                continue;
            }
            // Hover highlight
            if state.hovered_row == Some(row) {
                render.set_fill_color("rgba(64,128,255,0.10)");
                render.fill_rect(rect.x, y, rect.width, ROW_H);
            }
            let cells: [&str; 4] = [sym, price, delta, vol];
            for (i, text) in cells.iter().enumerate() {
                let color = if i == 2 {
                    if delta.starts_with('+') { "#10b981" } else { "#ef5350" }
                } else {
                    "#d1d4dc"
                };
                let cell = Cell {
                    rect: Rect::new(
                        col_xs[i],
                        y,
                        col_xs[i + 1] - col_xs[i],
                        ROW_H,
                    ),
                    text:  text,
                    color,
                };
                cell.draw(render);
            }
        }
    }

    // ── Hit-test ───────────────────────────────────────────────────────────────

    pub enum Hit {
        Separator(usize),
        Row(usize),
        None,
    }

    pub fn hit_test(state: &WatchlistState, rect: Rect, x: f64, y: f64) -> Hit {
        if !rect.contains(x, y) {
            return Hit::None;
        }
        const HEADER_H: f64 = 28.0;
        const ROW_H:    f64 = 24.0;
        const SEP_HALF: f64 = 4.0;  // ±4 px sensitivity

        // Compute column boundary xs
        let mut cx = rect.x;
        let mut boundaries = [0.0_f64; 3];
        for i in 0..3 {
            cx += rect.width * state.col_widths[i];
            boundaries[i] = cx;
        }

        // Separator wins if within ±SEP_HALF of a boundary
        for (i, &bx) in boundaries.iter().enumerate() {
            if (x - bx).abs() <= SEP_HALF {
                return Hit::Separator(i);
            }
        }

        // Row (below header)
        if y >= rect.y + HEADER_H {
            let row_f = (y - rect.y - HEADER_H + state.scroll_off) / ROW_H;
            if row_f >= 0.0 {
                return Hit::Row(row_f as usize);
            }
        }

        Hit::None
    }

    // ── Input handlers ─────────────────────────────────────────────────────────

    pub fn on_mouse_move(state: &mut WatchlistState, rect: Rect, x: f64, y: f64) {
        // Normalize col_widths so they always sum to 1.0 (guards against drift).
        {
            let sum: f64 = state.col_widths.iter().sum();
            if (sum - 1.0).abs() > 1e-6 && sum > 0.0 {
                for w in &mut state.col_widths {
                    *w /= sum;
                }
            }
        }

        // Continue column drag if active — use fraction cursor position so
        // the delta is rect-width-independent, fixing resize when rect changes.
        if let Some((idx, start_frac, start_widths)) = state.drag_sep {
            if rect.width > 0.0 {
                let cur_frac = (x - rect.x) / rect.width;
                let delta_frac = cur_frac - start_frac;
                let new_left  = (start_widths[idx]     + delta_frac).clamp(0.05, 0.90);
                let new_right = (start_widths[idx + 1] - delta_frac).clamp(0.05, 0.90);
                let total_before = start_widths[idx] + start_widths[idx + 1];
                let total_after  = new_left + new_right;
                // Only apply when widths still sum correctly (both clamped independently)
                if (total_after - total_before).abs() < 1e-9 {
                    state.col_widths[idx]     = new_left;
                    state.col_widths[idx + 1] = new_right;
                }
            }
            return;
        }

        match hit_test(state, rect, x, y) {
            Hit::Separator(i) => {
                state.hovered_sep = Some(i);
                state.hovered_row = Option::None;
            }
            Hit::Row(r) => {
                state.hovered_row = Some(r);
                state.hovered_sep = Option::None;
            }
            Hit::None => {
                state.hovered_row = Option::None;
                state.hovered_sep = Option::None;
            }
        }
    }

    pub fn on_mouse_down(state: &mut WatchlistState, rect: Rect, x: f64, y: f64) {
        if let Hit::Separator(i) = hit_test(state, rect, x, y) {
            // Store cursor position as a fraction of rect.width so that drag
            // deltas remain correct even if rect.width changes later.
            let start_frac = if rect.width > 0.0 { (x - rect.x) / rect.width } else { 0.0 };
            state.drag_sep = Some((i, start_frac, state.col_widths));
        }
    }

    pub fn on_mouse_up(state: &mut WatchlistState) {
        state.drag_sep = Option::None;
    }

    pub fn on_wheel(state: &mut WatchlistState, _rect: Rect, dy: f64) {
        state.scroll_off = (state.scroll_off - dy * 20.0).max(0.0);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_debug_console();
    eprintln!("[L3] debug console attached — all clicks/events will print here");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut handler = Handler { state: None };
    event_loop.run_app(&mut handler)?;
    Ok(())
}
