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
//! cargo run -p uzor-examples --bin l3-dashboard
//! ```
//!
//! # ⚠️ "L2-INSIDE-L3" REGIONS
//!
//! Some sections of this example are deliberately written in the L2 manual
//! style — direct atomic registration, hand-rolled hit-tests, app-side drag
//! math — even though they live inside an L3 app. They are clearly framed
//! with `// ── L2-INSIDE-L3 BLOCK ──` / `// ── /L2-INSIDE-L3 BLOCK ──`
//! markers and an inline note explaining what is happening.
//!
//! The "L2 widget set" modal (`ModalKind::L2`) is the largest such region:
//! it carries a full catalog of low-level widgets (radio / slider / range /
//! swatch / scrollbar / splitter / sub-tabs) registered + driven manually so
//! the framework demo can show all atomic widgets in one place.
//!
//! ## Why this is OK
//!
//! After the planned **Phase D** refactor (see
//! `docs/uzor-widget-research/l3-architecture-plan-v2.md`), L2-inside-L3
//! becomes a **proper escape hatch** via `add_blackbox_panel(handler)`:
//! - L3-app declares a blackbox region with a rect.
//! - Inside the handler, the app does whatever it wants — hand-rolled paint,
//!   manual hit-test, custom drag math, even a parallel mini-L2 mini-runtime.
//! - The rest of the L3-app keeps using the typed builder API
//!   (`layout.modal_mut(h).body(|b| b.slider(..) b.radio_group(..))`).
//!
//! ## What NOT to do
//!
//! - **Do NOT migrate the math inside an L2-inside-L3 block to LayoutManager.**
//!   It does not belong there — LM owns L3 composite state, not arbitrary
//!   app drag math. The right answer for "L3-style typed slider/range/scroll"
//!   is the **lib-side body builder** (Phase D), not pulling app math into LM.
//! - **Do NOT promote magic constants from these blocks** (`SLID_RECT`,
//!   `RANGE_RECT`, `SB_H`, `CONTENT_H`, `TAB_STRIP_Y`, …) into the lib.
//!   They are private layout numbers of *this* example's hand-rolled L2
//!   region, not framework geometry.
//! - **Do NOT generalise `l2_*` state fields onto AppState as if they were
//!   first-class.** They belong inside the future blackbox handler struct.
//!
//! ## Migration target (decided)
//!
//! All L2-inside-L3 work survives — it does NOT get deleted.  It moves into
//! a `L2DemoBlackbox: BlackboxHandler` that owns the entire L2 catalog:
//!
//! 1. The L2 modal body becomes one `add_blackbox_panel(L2DemoBlackbox::new())`
//!    call.  `BlackboxHandler::render` paints the catalog; `handle_event`
//!    dispatches `PointerDown / PointerMove / PointerUp / Wheel` to the
//!    same hand-rolled code that lives in the marked blocks today.
//! 2. The same demo is duplicated next to it via the L3 body builder —
//!    `b.slider(..)`, `b.radio_group(..)` — for side-by-side comparison
//!    of the two paths.
//! 3. All `L2_*` constants and `l2_*` fields move into the blackbox handler
//!    struct.  `AppState` stops knowing the L2 demo exists; only the
//!    blackbox holds it.
//!
//! After Phase D `level3_dashboard.rs` no longer contains a single
//! `L2-INSIDE-L3 BLOCK` marker — every block has migrated wholesale into
//! `L2DemoBlackbox`.

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
use uzor::layout::{
    ContextMenuHandle, DropdownHandle, EdgeSide, EdgeSlot, LayoutManager, LayoutNodeId,
    ModalHandle, OverlayHandle, PopupHandle, SidebarHandle, ToolbarHandle,
};
use uzor::types::{Rect, WidgetId, WidgetState, unsafe_widget_id};

// ── composite widgets ─────────────────────────────────────────────────────────
use uzor::ui::widgets::composite::chrome::input::{
    chrome_hit_test, handle_chrome_action, register_layout_manager_chrome,
};
use uzor::ui::widgets::composite::chrome::settings::ChromeSettings;

use uzor::ui::widgets::composite::chrome::style::ChromeStyle;
use uzor::ui::widgets::composite::chrome::types::{
    ChromeAction, ChromeRenderKind, ChromeTabConfig, ChromeView,
};

use uzor::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu;
use uzor::ui::widgets::composite::context_menu::settings::ContextMenuSettings;
use uzor::ui::widgets::composite::context_menu::types::{
    ContextMenuItem, ContextMenuRenderKind, ContextMenuView,
};

use uzor::ui::widgets::composite::dropdown::input::{
    open_dropdown_flat, register_layout_manager_dropdown,
};
use uzor::ui::widgets::composite::dropdown::render::measure_flat;
use uzor::ui::widgets::composite::dropdown::settings::DropdownSettings;
use uzor::ui::widgets::composite::dropdown::types::{
    DropdownItem, DropdownItemRight, DropdownRenderKind, DropdownView, DropdownViewKind,
};

use uzor::ui::widgets::composite::modal::input::{
    self as modal_input, handle_modal_drag, register_layout_manager_modal,
};
use uzor::ui::widgets::composite::modal::render::measure_chrome as measure_modal_chrome;
use uzor::ui::widgets::composite::modal::settings::ModalSettings;
use uzor::ui::widgets::composite::modal::types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView, WizardPageInfo,
};


use uzor::ui::widgets::composite::popup::input::{
    self as popup_input, register_layout_manager_popup,
};
use uzor::ui::widgets::composite::popup::settings::PopupSettings;
use uzor::ui::widgets::composite::popup::types::{
    BackdropKind as PopupBackdrop, PopupRenderKind, PopupView, PopupViewKind,
};

use uzor::ui::widgets::composite::sidebar::input::{
    self as sidebar_input, register_layout_manager_sidebar,
};
use uzor::ui::widgets::composite::sidebar::render::measure as measure_sidebar;
use uzor::ui::widgets::composite::sidebar::settings::SidebarSettings;
use uzor::ui::widgets::composite::sidebar::style::{DefaultSidebarStyle, SidebarStyle};
use uzor::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarRenderKind, SidebarView,
};

use uzor::ui::widgets::composite::toolbar::input::{
    self as toolbar_input, register_layout_manager_toolbar,
};
use uzor::ui::widgets::composite::toolbar::render::{measure_horizontal as measure_toolbar_h, measure_vertical as measure_toolbar_v};
use uzor::ui::widgets::composite::toolbar::settings::ToolbarSettings;
use uzor::ui::widgets::composite::toolbar::style::{
    BackgroundFill as ToolbarBackgroundFill, HorizontalToolbarStyle, ToolbarStyle,
    VerticalToolbarStyle,
};
use uzor::ui::widgets::composite::toolbar::types::{
    ToolbarItem, ToolbarRenderKind, ToolbarSection, ToolbarView,
};

// ── atomic widgets (used inside modals) ──────────────────────────────────────
use uzor::ui::widgets::atomic::button::ButtonTheme;

use uzor::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox;
use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::theme::CheckboxTheme;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

use uzor::render::RenderContext;
use uzor::ui::widgets::atomic::text::{draw_text, TextSettings};
use uzor::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use uzor::render::{TextAlign, TextBaseline};

use uzor::ui::widgets::atomic::text_input::input::register_layout_manager_text_input;
use uzor::ui::widgets::atomic::text_input::render::InputView;
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::state::TextFieldConfig as TiTextFieldConfig;
use uzor::ui::widgets::atomic::text_input::types::InputType;
use uzor::input::text::store::TextFieldConfig as StoreTextFieldConfig;

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
// Layout-edge thicknesses are now derived from per-composite measure_*
// helpers (see render() — measure_toolbar_h / measure_toolbar_v / measure_sidebar).
// Only chrome height stays explicit since the chrome composite is wired
// differently (LayoutManager::chrome_mut().height).
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

// L2 widget catalog constants moved into mod l2_demo_blackbox below.

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
/// Custom L3 sidebar style: shows a faded centred 80%-wide header divider
/// at 60% opacity. Borders fall through to the default per-kind inner-edge.
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
    // Legacy kill-switch off so the new DividerConfig has effect.
    fn show_header_divider(&self) -> bool { true }
    fn header_divider(&self) -> uzor::ui::widgets::composite::sidebar::style::DividerConfig {
        uzor::ui::widgets::composite::sidebar::style::DividerConfig {
            visible: true,
            width:   1.0,
            opacity: 0.60,
            length_frac: 0.80,
        }
    }
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
    /// Existing — full L2 widget catalog (radio/slider/range/swatch/scroll/...)
    L2,
    /// Existing — bespoke big animated button
    L1,
    /// Existing — settings preview panel
    Settings,
    /// Existing — dock panel list (chrome tabs)
    Tags,
    /// Plain frame — no header, no footer. Demonstrates ModalRenderKind::Plain.
    PlainDemo,
    /// Header-only modal (no footer). Demonstrates ModalRenderKind::WithHeader.
    HeaderDemo,
    /// Top-tabs modal — three sample tabs across the top.
    TopTabsDemo,
    /// Side-tabs modal — vertical icon sidebar inside the modal.
    SideTabsDemo,
    /// Wizard modal — multi-step page indicator + Back/Next nav.
    WizardDemo,
}

// =============================================================================
// Drag target for L2 sliders
// =============================================================================

enum DragTarget {
    /// Sidebar scrollbar thumb drag.
    /// `track_rect` is captured at drag start; the scrollbar atomic API
    /// converts cursor Y into scroll offset using track height + content_h.
    SidebarScrollbar { track_rect: Rect, content_h: f64, viewport_h: f64 },
    /// Sidebar resize drag.
    /// Sidebar resize — math lives on `SidebarState::resize_drag`. `which`
    /// selects which sidebar's state to update.
    SidebarResize { which: &'static str },
    /// Modal header drag — stores cursor-relative-to-modal offset so modal moves smoothly.
    ModalDrag,
    /// Dock separator drag — stores separator index and start mouse position.
    SeparatorDrag { sep_idx: usize, start_x: f64, start_y: f64 },
    /// Toolbar resize — math lives on `ToolbarState`. `which` selects
    /// which toolbar's state to update.
    ToolbarResize { which: &'static str },
    /// Modal / popup resize — math lives on the composite's state.
    OverlayResize { which: &'static str },
    /// Modal body scrollbar thumb drag — `ModalState::update_body_scroll_drag`
    /// runs the math.
    ModalBodyScroll,
    /// Popup body scrollbar thumb drag.
    PopupBodyScroll,
}

/// Resolve a `sidebar_kind` index back to a fresh `SidebarRenderKind`.
/// We store the index because the enum has a closure variant and isn't Clone.
fn sidebar_kind_from_index(idx: u8) -> SidebarRenderKind {
    match idx {
        1 => SidebarRenderKind::Right,
        2 => SidebarRenderKind::WithTypeSelector,
        3 => SidebarRenderKind::Embedded,
        _ => SidebarRenderKind::Left,
    }
}

fn _sidebar_kind_label(idx: u8) -> &'static str {
    match idx {
        1 => "Right",
        2 => "WithTypeSelector",
        3 => "Embedded",
        _ => "Left",
    }
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

    /// User-resized override for top toolbar height (px). 0.0 = use measured.
    top_toolbar_height_override: f64,
    demo_toolbar_left2_w_override:  f64,
    demo_toolbar_right_w_override:  f64,
    demo_toolbar_bottom_h_override: f64,
    /// User-resized modal size override (w, h). (0.0, 0.0) = use measured.
    modal_size_override: (f64, f64),

    // App interaction state
    active_view: usize, // 0=Dashboard,1=Charts,2=Settings
    sidebar_open: bool,
    /// 0=Left, 1=Right, 2=WithTypeSelector, 3=Embedded.
    /// Stored as index because SidebarRenderKind isn't Clone (has a closure variant).
    sidebar_kind: u8,
    /// Which popup template is currently open: None / Some(PopupKindIdx).
    /// 0=ColorPickerGrid, 1=ColorPickerHsv, 2=SwatchGrid, 3=ItemList, 4=IndicatorStrip.
    popup_kind: Option<u8>,
    /// Toolbar render variant index (0=Horizontal, 1=Vertical, 2=ChromeStrip, 3=Inline).
    /// Affects only the demo toolbar — main top toolbar stays Horizontal.
    _toolbar_kind: u8,
    /// Toggle visibility for demo toolbars (extra to the main top one).
    /// Sidebar dropdown items toggle these bits — clicking spawns/hides.
    demo_toolbar_left2:  bool,
    demo_toolbar_right:  bool,
    demo_toolbar_bottom: bool,
    /// Toggle visibility for demo sidebars (extra to the main left one).
    demo_sidebar_right:  bool,
    demo_sidebar_top:    bool,
    demo_sidebar_bottom: bool,
    // demo sidebar states are now in layout.sidebars
    /// When true, demo edges use EdgePlacement::Overlay (float over dock area)
    /// instead of the default Compress (shrink dock area).
    demo_overlay_mode: bool,
    left_toolbar_visible: bool,
    modal_open: bool,
    modal_kind: ModalKind,

    // popup: which toolbar item is hovered
    popup_item: Option<String>,

    // L2 demo blackbox — owns all state, render, and input for ModalKind::L2.
    l2_demo: l2_demo_blackbox::L2DemoBlackbox,

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

    // Fix 3: Watchlist blackbox state
    watchlist: watchlist_blackbox::WatchlistState,

    // ── Phase A+C: typed composite handles ───────────────────────────────────
    modal_h:               ModalHandle,
    dd_file_h:             DropdownHandle,
    dd_view_h:             DropdownHandle,
    dd_help_h:             DropdownHandle,
    dd_sidebar_h:          DropdownHandle,
    dd_toolbar_h:          DropdownHandle,
    dd_popup_h:            DropdownHandle,
    ctx_menu_h:            ContextMenuHandle,
    top_toolbar_h:         ToolbarHandle,
    left_vtoolbar_h:       ToolbarHandle,
    demo_toolbar_left2_h:  ToolbarHandle,
    demo_toolbar_right_h:  ToolbarHandle,
    demo_toolbar_bottom_h: ToolbarHandle,
    sidebar_h:             SidebarHandle,
    demo_sidebar_right_h:  SidebarHandle,
    demo_sidebar_top_h:    SidebarHandle,
    demo_sidebar_bottom_h: SidebarHandle,
    demo_popup_h:          PopupHandle,
}

impl AppState {
    fn time_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn time_ms(&self) -> f64 {
        self.start.elapsed().as_millis() as f64
    }

    /// Handle a dropdown item click coming from the dispatcher.
    /// Replaces the strip_prefix("dd-X-widget:item:") cascade.
    fn handle_dropdown_item(&mut self, dropdown_id: &str, item_id: &str, event_loop: &ActiveEventLoop) {
        match dropdown_id {
            "dd-file-widget" => {
                match item_id {
                    "file-quit" => { event_loop.exit(); }
                    "file-new"  => println!("[L3] File → New"),
                    "file-open" => println!("[L3] File → Open"),
                    "file-save" => println!("[L3] File → Save"),
                    _ => {}
                }
                self.layout.dropdown_mut(&self.dd_file_h.clone()).close();
            }
            "dd-view-widget" => {
                // Toggle items: keep dropdown OPEN.
                match item_id {
                    "view-sidebar" => { self.sidebar_open = !self.sidebar_open; }
                    "view-toolbar" => { self.left_toolbar_visible = !self.left_toolbar_visible; }
                    other => {
                        println!("[L3] View → {other}");
                        self.layout.dropdown_mut(&self.dd_view_h.clone()).close();
                    }
                }
            }
            "dd-help-widget" => {
                let open_modal = |kind: ModalKind, this: &mut AppState| {
                    this.modal_open = true;
                    this.modal_kind = kind;
                    this.layout.modal_mut(&this.modal_h.clone()).position = (0.0, 0.0);
                };
                match item_id {
                    "modals-l2"       => open_modal(ModalKind::L2,           self),
                    "modals-l1"       => open_modal(ModalKind::L1,           self),
                    "modals-panels"   => open_modal(ModalKind::Tags,         self),
                    "modals-settings" => open_modal(ModalKind::Settings,     self),
                    "modals-plain"    => open_modal(ModalKind::PlainDemo,    self),
                    "modals-header"   => open_modal(ModalKind::HeaderDemo,   self),
                    "modals-toptabs"  => open_modal(ModalKind::TopTabsDemo,  self),
                    "modals-sidetabs" => open_modal(ModalKind::SideTabsDemo, self),
                    "modals-wizard"   => open_modal(ModalKind::WizardDemo,   self),
                    other             => println!("[L3] Modals → {other}"),
                }
                self.layout.dropdown_mut(&self.dd_help_h.clone()).close();
            }
            "dd-sidebar-widget" => {
                // Toggle items keep dropdown open.
                match item_id {
                    "sb-toggle-main"  => self.sidebar_open       = !self.sidebar_open,
                    "sb-spawn-right"  => self.demo_sidebar_right  = !self.demo_sidebar_right,
                    "sb-spawn-top"    => self.demo_sidebar_top    = !self.demo_sidebar_top,
                    "sb-spawn-bottom" => self.demo_sidebar_bottom = !self.demo_sidebar_bottom,
                    "sb-overlay-mode" => self.demo_overlay_mode   = !self.demo_overlay_mode,
                    _ => {}
                }
                println!("[L3] sidebars: main={} right={} top={} bottom={}",
                    self.sidebar_open, self.demo_sidebar_right, self.demo_sidebar_top, self.demo_sidebar_bottom);
            }
            "dd-toolbar-widget" => {
                // Toggle items keep dropdown open.
                match item_id {
                    "tb-toggle-main"  => {} // main toolbar is always on
                    "tb-toggle-left"  => self.left_toolbar_visible = !self.left_toolbar_visible,
                    "tb-spawn-left2"  => self.demo_toolbar_left2  = !self.demo_toolbar_left2,
                    "tb-spawn-right"  => self.demo_toolbar_right  = !self.demo_toolbar_right,
                    "tb-spawn-bottom" => self.demo_toolbar_bottom = !self.demo_toolbar_bottom,
                    _ => {}
                }
                println!("[L3] toolbars: top=on left={} left2={} right={} bottom={}",
                    self.left_toolbar_visible, self.demo_toolbar_left2,
                    self.demo_toolbar_right, self.demo_toolbar_bottom);
            }
            "dd-popup-widget" => {
                // L2 sub-items (under :sub-item: prefix) reach this branch.
                // 0 = Plain (text body), 1 = Custom (color grid 4×4 with
                // each cell registered as a Button child of the popup).
                self.popup_kind = match item_id {
                    "popup-plain"        => Some(0),
                    "popup-custom-grid"  => Some(1),
                    _                    => self.popup_kind,
                };
                self.layout.dropdown_mut(&self.dd_popup_h.clone()).close();
            }
            other => println!("[L3] dropdown {other} item: {item_id}"),
        }
    }

    /// Resolve the screen rect of a toolbar item from the InputCoordinator's
    /// last-frame widget table. Toolbar composite registers each item as
    /// "{toolbar_id}:{item_id}" — `widget_rect` looks it up directly.
    fn toolbar_item_rect(&self, toolbar_id: &str, item_id: &str) -> Option<Rect> {
        let full = unsafe_widget_id(format!("{toolbar_id}:{item_id}"));
        self.layout.ctx().input.widget_rect(&full)
    }

    /// Open a dropdown anchored to the bottom-left of a toolbar item's rect.
    /// Toggles closed if it was already open under that key.
    fn toggle_dropdown_at(&mut self, overlay_id: &'static str, item_rect: Rect) {
        // Map overlay_id → typed handle.
        let handle = match overlay_id {
            "dd-file-overlay"    => self.dd_file_h.clone(),
            "dd-view-overlay"    => self.dd_view_h.clone(),
            "dd-help-overlay"    => self.dd_help_h.clone(),
            "dd-sidebar-overlay" => self.dd_sidebar_h.clone(),
            "dd-toolbar-overlay" => self.dd_toolbar_h.clone(),
            "dd-popup-overlay"   => self.dd_popup_h.clone(),
            _ => return,
        };
        // Check if currently open.
        let was_open = self.layout.dropdown(&handle).open;
        // Close all dropdowns.
        let all = [
            self.dd_file_h.clone(), self.dd_view_h.clone(), self.dd_help_h.clone(),
            self.dd_sidebar_h.clone(), self.dd_toolbar_h.clone(), self.dd_popup_h.clone(),
        ];
        for h in &all {
            self.layout.dropdown_mut(h).close();
        }
        // If was closed, open this one.
        if !was_open {
            self.layout.dropdown_mut(&handle).open_below(item_rect, 0.0);
        }
    }

    /// Handle a toolbar item click coming from the dispatcher.
    /// Replaces the prefix-match strip_prefix("top-toolbar-widget:") cascade.
    fn handle_toolbar_item(&mut self, toolbar_id: &str, item_id: &str) {
        // Resolve the clicked button's rect from the coord — anchors the
        // dropdown to its bottom-left edge regardless of toolbar layout.
        let item_rect = self.toolbar_item_rect(toolbar_id, item_id);
        match (toolbar_id, item_id) {
            ("top-toolbar-widget", "tb-file")    => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-file-overlay",    r); },
            ("top-toolbar-widget", "tb-view")    => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-view-overlay",    r); },
            ("top-toolbar-widget", "tb-help")    => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-help-overlay",    r); },
            ("top-toolbar-widget", "tb-sidebar") => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-sidebar-overlay", r); },
            ("top-toolbar-widget", "tb-toolbar") => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-toolbar-overlay", r); },
            ("top-toolbar-widget", "tb-popup")   => if let Some(r) = item_rect { self.toggle_dropdown_at("dd-popup-overlay",   r); },
            ("left-vtoolbar-widget", "lt-toggle-sidebar") => {
                self.sidebar_open = !self.sidebar_open;
                println!("[L3] sidebar → {}", self.sidebar_open);
            }
            (tb, item) => {
                println!("[L3] toolbar {tb} item {item}");
            }
        }
    }

    /// Handle a context-menu item click coming from the dispatcher.
    /// Replaces the strip_prefix("ctx-menu-widget:item:") cascade.
    fn handle_ctx_menu_item(&mut self, item_index: usize) {
        match item_index {
            0 => println!("[L3] ctx → Copy"),
            1 => println!("[L3] ctx → Paste"),
            2 => println!("[L3] ctx → Delete"),
            3 => println!("[L3] ctx → Properties"),
            4 => {
                if self.modal_open && self.modal_kind == ModalKind::Settings {
                    self.modal_open = false;
                } else {
                    self.modal_open = true;
                    self.modal_kind = ModalKind::Settings;
                    self.layout.modal_mut(&self.modal_h.clone()).position = (0.0, 0.0);
                }
            }
            _ => {}
        }
        self.layout.context_menu_mut(&self.ctx_menu_h.clone()).close();
    }

    fn switch_tab(&mut self, new_tab: usize) {
        if new_tab == self.active_view { return; }
        let old_tree = std::mem::take(self.layout.panels_mut().tree_mut());
        self.tab_trees[self.active_view] = old_tree;
        let new_tree = std::mem::take(&mut self.tab_trees[new_tab]);
        *self.layout.panels_mut().tree_mut() = new_tree;
        self.active_view = new_tab;
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

        // Top toolbar — thickness from toolbar style via measure_horizontal.
        // Empty view returns (pad*2, style.height()) — we just need the height.
        let probe_view_h = ToolbarView {
            start: ToolbarSection::empty(), center: ToolbarSection::empty(),
            end: ToolbarSection::empty(), chrome: None,
            overflow: uzor::types::OverflowMode::Clip,
            resize_edge: None,
        };
        let probe_settings = ToolbarSettings::new(
            Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
            Box::new(HorizToolbarWithBorder),
        );
        let (_, top_h_measured) = measure_toolbar_h(&probe_view_h, &probe_settings);
        let top_h = if self.top_toolbar_height_override > 0.0 {
            self.top_toolbar_height_override
        } else {
            top_h_measured
        };
        // Wipe last frame's edge slots — the dropdowns spawn extra slots
        // conditionally on every frame and any slot we don't re-add now
        // should disappear (its space returns to the dock area).
        self.layout.edges_mut().clear();
        self.layout.edges_mut().add(EdgeSlot {
            id: "top-toolbar".to_string(),
            side: EdgeSide::Top,
            thickness: top_h as f32,
            visible: true,
            order: 0,
            ..Default::default()
        });

        // Left vertical toolbar (toggled via View → Show Toolbar) —
        // thickness from style via measure_vertical.
        let probe_view_v = ToolbarView {
            start: ToolbarSection::empty(), center: ToolbarSection::empty(),
            end: ToolbarSection::empty(), chrome: None,
            overflow: uzor::types::OverflowMode::Clip,
            resize_edge: None,
        };
        let probe_settings_v = ToolbarSettings::new(
            Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
            Box::new(VertToolbarWithBorder),
        );
        let (left_w, _) = measure_toolbar_v(&probe_view_v, &probe_settings_v);
        self.layout.edges_mut().add(EdgeSlot {
            id: "left-vtoolbar".to_string(),
            side: EdgeSide::Left,
            thickness: left_w as f32,
            visible: self.left_toolbar_visible,
            order: 0,
            ..Default::default()
        });

        // Sidebar (slide-out, toggled): always registered, visible toggled.
        // Width from sidebar style via measure(); side / kind chosen by sidebar_kind.
        let kind_value = sidebar_kind_from_index(self.sidebar_kind);
        let edge_side = match self.sidebar_kind {
            1 => EdgeSide::Right,           // Right
            _ => EdgeSide::Left,            // Left, WithTypeSelector, Embedded → left
        };
        let (default_sidebar_w, _chrome_h) = measure_sidebar(
            &SidebarSettings::default(),
            &kind_value,
        );
        // Sidebar width follows user resize (sidebar state.width). Initialised
        // from measure_sidebar()'s default; the resize-handle drag updates
        // state.width which then flows back into the edge slot.
        let sidebar_w = {
            let w = self.layout.sidebar(&self.sidebar_h).width;
            if w > 0.0 { w } else { default_sidebar_w }
        };
        self.layout.edges_mut().add(EdgeSlot {
            id: "sidebar".to_string(),
            side: edge_side,
            thickness: sidebar_w as f32,
            visible: self.sidebar_open,
            order: 1,
            ..Default::default()
        });

        // ── Demo toolbars / sidebars (toggleable from dropdowns) ──────────────
        // Placement follows the Sidebar dropdown's "Overlay mode" toggle:
        // Compress → shrink dock area; Overlay → float on top of dock.
        let demo_placement = if self.demo_overlay_mode {
            uzor::layout::EdgePlacement::Overlay
        } else {
            uzor::layout::EdgePlacement::Compress
        };
        if self.demo_toolbar_left2 {
            let t = if self.demo_toolbar_left2_w_override > 0.0 {
                self.demo_toolbar_left2_w_override as f32
            } else { left_w as f32 };
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-toolbar-left2".into(),
                side: EdgeSide::Left,
                thickness: t,
                visible: true,
                order: 2,
                placement: demo_placement,
            });
        }
        if self.demo_toolbar_right {
            let t = if self.demo_toolbar_right_w_override > 0.0 {
                self.demo_toolbar_right_w_override as f32
            } else { left_w as f32 };
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-toolbar-right".into(),
                side: EdgeSide::Right,
                thickness: t,
                visible: true,
                order: 0,
                placement: demo_placement,
            });
        }
        if self.demo_toolbar_bottom {
            let t = if self.demo_toolbar_bottom_h_override > 0.0 {
                self.demo_toolbar_bottom_h_override as f32
            } else { top_h as f32 };
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-toolbar-bottom".into(),
                side: EdgeSide::Bottom,
                thickness: t,
                visible: true,
                order: 0,
                placement: demo_placement,
            });
        }
        // EdgeSlot.thickness pulls from state.width — composite initialises it
        // via SidebarState::ensure_sized() on first register from viewport %.
        // Until that first register fires we seed it here too so the slot
        // reserves the right space on the very first frame.
        let viewport_w = width as f64;
        let viewport_h = height as f64;
        {
            let h = self.demo_sidebar_right_h.clone();
            let st = self.layout.sidebar_mut(&h);
            st.ensure_sized(viewport_w, viewport_h, true);
        }
        {
            let h = self.demo_sidebar_top_h.clone();
            let st = self.layout.sidebar_mut(&h);
            st.ensure_sized(viewport_w, viewport_h, false);
        }
        {
            let h = self.demo_sidebar_bottom_h.clone();
            let st = self.layout.sidebar_mut(&h);
            st.ensure_sized(viewport_w, viewport_h, false);
        }
        if self.demo_sidebar_right {
            let h = self.demo_sidebar_right_h.clone();
            let w = self.layout.sidebar(&h).width as f32;
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-sidebar-right".into(),
                side: EdgeSide::Right,
                thickness: w,
                visible: true,
                order: 1,
                placement: demo_placement,
            });
        }
        if self.demo_sidebar_top {
            let h = self.demo_sidebar_top_h.clone();
            let w = self.layout.sidebar(&h).width as f32;
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-sidebar-top".into(),
                side: EdgeSide::Top,
                thickness: w,
                visible: true,
                order: 1,
                placement: demo_placement,
            });
        }
        if self.demo_sidebar_bottom {
            let h = self.demo_sidebar_bottom_h.clone();
            let w = self.layout.sidebar(&h).width as f32;
            self.layout.edges_mut().add(EdgeSlot {
                id: "demo-sidebar-bottom".into(),
                side: EdgeSide::Bottom,
                thickness: w,
                visible: true,
                order: 1,
                placement: demo_placement,
            });
        }

        // Clear overlays at the START of frame (composites push them via
        // register_layout_manager_* calls below).
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
        // Wipe last frame's dispatcher patterns — composites re-register on
        // each register_layout_manager_* call below.
        self.layout.dispatcher_begin_frame();

        // ── 4. Scene ──────────────────────────────────────────────────────────
        self.scene.reset();
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, BG, None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        let time_ms = self.time_ms();
        // Push frame time into LayoutManager so atomics with animations
        // (text_input caret blink etc.) can read it.
        self.layout.set_frame_time_ms(time_ms as f64);
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
            &chrome_view,
            &chrome_settings,
            &chrome_kind,
        );

        // ── Top toolbar ───────────────────────────────────────────────────────
        let view_btn_active    = self.layout.dropdown(&self.dd_view_h).open;
        let modals_btn_active  = self.layout.dropdown(&self.dd_help_h).open;
        let sidebar_btn_active = self.layout.dropdown(&self.dd_sidebar_h).open;
        let toolbar_btn_active = self.layout.dropdown(&self.dd_toolbar_h).open;
        let popup_btn_active   = self.layout.dropdown(&self.dd_popup_h).open;
        let top_toolbar_items = [
            ToolbarItem::TextButton { id: "tb-view",    text: "View",    active: view_btn_active,    tooltip: Some("View menu"),         popup_on_hover: true },
            ToolbarItem::TextButton { id: "tb-help",    text: "Modals",  active: modals_btn_active,  tooltip: Some("Modals menu"),       popup_on_hover: true },
            ToolbarItem::Separator,
            ToolbarItem::TextButton { id: "tb-sidebar", text: "Sidebar", active: sidebar_btn_active, tooltip: Some("Sidebar variants"),  popup_on_hover: false },
            ToolbarItem::TextButton { id: "tb-toolbar", text: "Toolbar", active: toolbar_btn_active, tooltip: Some("Toolbar variants"),  popup_on_hover: false },
            ToolbarItem::TextButton { id: "tb-popup",   text: "Popup",   active: popup_btn_active,   tooltip: Some("Popup templates"),   popup_on_hover: false },
        ];
        let clock_items = [
            ToolbarItem::Clock { id: "top-clock", time_text: clock.as_str() },
        ];
        let top_toolbar_view = ToolbarView {
            start: ToolbarSection { items: &top_toolbar_items },
            center: ToolbarSection::empty(),
            end: ToolbarSection { items: &clock_items },
            chrome: None,
            overflow: uzor::types::OverflowMode::Clip,
            resize_edge: None,
        };
        register_layout_manager_toolbar(
            &mut self.layout,
            &mut render,
            LayoutNodeId::ROOT,
            "top-toolbar",
            &self.top_toolbar_h.clone(),
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
            ToolbarItem::TextButton { id: "lt-toggle-sidebar", text: "☰", active: sidebar_open, tooltip: Some("Toggle sidebar"), popup_on_hover: false },
        ];
        if self.left_toolbar_visible {
            let left_toolbar_view = ToolbarView {
                start: ToolbarSection { items: &left_items },
                center: ToolbarSection::empty(),
                end: ToolbarSection::empty(),
                chrome: None,
                overflow: uzor::types::OverflowMode::Clip,
                resize_edge: Some(uzor::layout::ResizeEdge::E),
            };
            register_layout_manager_toolbar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "left-vtoolbar",
                &self.left_vtoolbar_h.clone(),
                &left_toolbar_view,
                &ToolbarSettings::new(
                    Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                    Box::new(VertToolbarWithBorder),
                ),
                &ToolbarRenderKind::Vertical,
            );
        }

        // ── Spawned demo toolbars — REAL toolbar composite instances ─────────
        // Toolbar composite supports every side (Horizontal for Top/Bottom,
        // Vertical for Left/Right). Each spawned instance is registered the
        // same way as the main top toolbar.
        let demo_tb_specs: [(&str, bool, ToolbarRenderKind); 3] = [
            ("demo-toolbar-left2",  self.demo_toolbar_left2,  ToolbarRenderKind::Vertical),
            ("demo-toolbar-right",  self.demo_toolbar_right,  ToolbarRenderKind::Vertical),
            ("demo-toolbar-bottom", self.demo_toolbar_bottom, ToolbarRenderKind::Horizontal),
        ];
        // Build a fat item list once — demo toolbars use it to force overflow.
        // 30 buttons guarantee the strip is wider/taller than any reasonable
        // viewport, so the chevron paging mode actually has work to do.
        let demo_overflow_labels: [&str; 30] = [
            "A1","A2","A3","A4","A5","A6","A7","A8","A9","A10",
            "B1","B2","B3","B4","B5","B6","B7","B8","B9","B10",
            "C1","C2","C3","C4","C5","C6","C7","C8","C9","C10",
        ];
        let demo_overflow_items: Vec<ToolbarItem<'_>> = demo_overflow_labels
            .iter()
            .map(|lbl| ToolbarItem::TextButton {
                id: lbl, text: lbl, active: false, tooltip: None, popup_on_hover: false,
            })
            .collect();
        // Each demo toolbar uses its own stored ToolbarState so the overflow
        // chevron's scroll_offset persists between frames (otherwise paging
        // can't accumulate). Resize edge is per-side: Left toolbar drags its
        // E (right) edge, Right toolbar drags its W (left) edge, Bottom
        // toolbar drags its N (top) edge.
        let mk_demo = |edge: uzor::layout::ResizeEdge| ToolbarView {
            start: ToolbarSection { items: &demo_overflow_items },
            center: ToolbarSection::empty(),
            end: ToolbarSection::empty(),
            chrome: None,
            overflow: uzor::types::OverflowMode::Chevrons,
            resize_edge: Some(edge),
        };
        if self.demo_toolbar_left2 {
            let view = mk_demo(uzor::layout::ResizeEdge::E);
            register_layout_manager_toolbar(
                &mut self.layout, &mut render, LayoutNodeId::ROOT,
                "demo-toolbar-left2", &self.demo_toolbar_left2_h.clone(),
                &view,
                &ToolbarSettings::new(
                    Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                    Box::new(VertToolbarWithBorder),
                ),
                &ToolbarRenderKind::Vertical,
            );
        }
        if self.demo_toolbar_right {
            let view = mk_demo(uzor::layout::ResizeEdge::W);
            register_layout_manager_toolbar(
                &mut self.layout, &mut render, LayoutNodeId::ROOT,
                "demo-toolbar-right", &self.demo_toolbar_right_h.clone(),
                &view,
                &ToolbarSettings::new(
                    Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                    Box::new(VertToolbarWithBorder),
                ),
                &ToolbarRenderKind::Vertical,
            );
        }
        if self.demo_toolbar_bottom {
            let view = mk_demo(uzor::layout::ResizeEdge::N);
            register_layout_manager_toolbar(
                &mut self.layout, &mut render, LayoutNodeId::ROOT,
                "demo-toolbar-bottom", &self.demo_toolbar_bottom_h.clone(),
                &view,
                &ToolbarSettings::new(
                    Box::<uzor::ui::widgets::composite::toolbar::theme::DefaultToolbarTheme>::default(),
                    Box::new(HorizToolbarWithBorder),
                ),
                &ToolbarRenderKind::Horizontal,
            );
        }
        let _ = demo_tb_specs;

        // ── Spawned demo sidebars — REAL sidebar composite instances ─────────
        // Sidebar composite currently supports Left / Right (Top / Bottom are
        // not in SidebarRenderKind yet — they'd need a composite-level addition).
        // For Top / Bottom we register an empty edge slot and fall back to a
        // Horizontal toolbar frame so the strip is at least visibly present.
        if self.demo_sidebar_right {
            let actions: &[HeaderAction<'_>] = &[];
            let mut view = SidebarView {
                header: SidebarHeader { icon: None, title: "Right sidebar", actions },
                header_mode: uzor::ui::widgets::composite::sidebar::types::SidebarHeaderMode::Sticky,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                overflow: uzor::types::OverflowMode::Clip,
                content_height: 200.0,
            };
            let _ = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "demo-sidebar-right",
                &self.demo_sidebar_right_h.clone(),
                &mut view,
                &SidebarSettings::default(),
                &SidebarRenderKind::Right,
            );
        }
        // Top / Bottom sidebars now first-class composite kinds.
        if self.demo_sidebar_top {
            let actions: &[HeaderAction<'_>] = &[];
            let mut view = SidebarView {
                header: SidebarHeader { icon: None, title: "Top sidebar", actions },
                header_mode: uzor::ui::widgets::composite::sidebar::types::SidebarHeaderMode::Sticky,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                overflow: uzor::types::OverflowMode::Clip,
                content_height: 200.0,
            };
            let _ = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "demo-sidebar-top",
                &self.demo_sidebar_top_h.clone(),
                &mut view,
                &SidebarSettings::default(),
                &SidebarRenderKind::Top,
            );
        }
        if self.demo_sidebar_bottom {
            let actions: &[HeaderAction<'_>] = &[];
            let mut view = SidebarView {
                header: SidebarHeader { icon: None, title: "Bottom sidebar", actions },
                header_mode: uzor::ui::widgets::composite::sidebar::types::SidebarHeaderMode::Sticky,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                overflow: uzor::types::OverflowMode::Clip,
                content_height: 200.0,
            };
            let _ = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "demo-sidebar-bottom",
                &self.demo_sidebar_bottom_h.clone(),
                &mut view,
                &SidebarSettings::default(),
                &SidebarRenderKind::Bottom,
            );
        }

        // ── Sidebar ───────────────────────────────────────────────────────────
        // Sidebar shows dock panel list with close buttons + "Add Panel" button.
        if self.sidebar_open {
            let sidebar_actions: &[HeaderAction<'_>] = &[];
            let sidebar_header = SidebarHeader { icon: None, title: "Dock Panels", actions: sidebar_actions };
            // Estimated body height — header + spawn UI + per-leaf rows.
            // When a Top/Bottom sidebar shrinks the available body the
            // composite shows a scrollbar instead of clipping content.
            let est_panels = self.layout.panels().tree().leaves().len() as f64;
            let est_content_h = 480.0 + est_panels * 30.0;
            let mut sidebar_view = SidebarView {
                header: sidebar_header,
                header_mode: uzor::ui::widgets::composite::sidebar::types::SidebarHeaderMode::Sticky,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                overflow: uzor::types::OverflowMode::Scrollbar,
                content_height: est_content_h,
            };
            let sidebar_kind_value = sidebar_kind_from_index(self.sidebar_kind);
            let _sidebar_node = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "sidebar",
                &self.sidebar_h.clone(),
                &mut sidebar_view,
                &{
                    let mut s = SidebarSettings::default();
                    s.style = Box::new(NoDividerSidebarStyle(DefaultSidebarStyle));
                    s
                },
                &sidebar_kind_value,
            );
            // Sidebar body — spawn UI + panel list (via SidebarBodyBuilder).
            if let Some(body_rect) = self.layout.rect_for_edge_slot("sidebar") {
                // Collect leaf data before borrowing layout mutably for the builder.
                let leaf_entries: Vec<(uzor::docking::panels::LeafId, String, bool)> = {
                    let active = self.layout.panels().active_leaf();
                    let mut entries: Vec<_> = self
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
                            (id, title, active == Some(id))
                        })
                        .collect();
                    entries.sort_by_key(|(id, _, _)| id.0);
                    entries
                };

                let sidebar_state_snap = self.layout.sidebar(&self.sidebar_h).clone();
                let body_vp = uzor::ui::widgets::composite::sidebar::render::begin_body(
                    &mut render,
                    body_rect,
                    &sidebar_state_snap,
                    &sidebar_view,
                    &{
                        let mut s = SidebarSettings::default();
                        s.style = Box::new(NoDividerSidebarStyle(DefaultSidebarStyle));
                        s
                    },
                    &sidebar_kind_value,
                );

                // Build radio items from PanelKind::all()
                let spawn_kind = &self.spawn_kind;
                let kind_radio_ids: Vec<String> = PanelKind::all()
                    .iter()
                    .map(|k| format!("spawn-kind-{}", k.title().to_lowercase()))
                    .collect();
                let kind_radio_items: Vec<sidebar_input::SidebarRadioItem<'_>> = PanelKind::all()
                    .iter()
                    .zip(kind_radio_ids.iter())
                    .map(|(k, id)| sidebar_input::SidebarRadioItem {
                        id: id.as_str(),
                        label: k.title(),
                        selected: spawn_kind == k,
                    })
                    .collect();

                let spawn_split = &self.spawn_split;
                let split_radio_items = [
                    sidebar_input::SidebarRadioItem { id: "spawn-split-horiz", label: "Split right",  selected: *spawn_split == SpawnSplit::SplitRight  },
                    sidebar_input::SidebarRadioItem { id: "spawn-split-vert",  label: "Split bottom", selected: *spawn_split == SpawnSplit::SplitBottom },
                    sidebar_input::SidebarRadioItem { id: "spawn-split-grid",  label: "Grid 2×2",     selected: *spawn_split == SpawnSplit::Grid2x2     },
                ];

                let close_ids: Vec<String> = (0..leaf_entries.len())
                    .map(|i| format!("dock-leaf-close-{i}"))
                    .collect();
                let panel_entries: Vec<sidebar_input::SidebarPanelEntry<'_>> = leaf_entries
                    .iter()
                    .zip(close_ids.iter())
                    .map(|((_, title, active), close_id)| sidebar_input::SidebarPanelEntry {
                        close_id: close_id.as_str(),
                        title: title.as_str(),
                        active: *active,
                    })
                    .collect();

                let mut builder = sidebar_input::SidebarBodyBuilder::new(
                    &mut render,
                    &mut self.layout,
                    body_rect,
                    body_vp.content_origin_y,
                    LayerId::main(),
                );
                builder.add_section_header("NEW PANEL");
                builder.add_sub_label("Type:");
                builder.add_radio_group(&kind_radio_items);
                builder.add_spacer(6.0);
                builder.add_sub_label("Split:");
                builder.add_radio_group(&split_radio_items);
                builder.add_spacer(8.0);
                builder.add_action_button("sidebar-spawn", "Spawn");
                builder.add_divider();
                builder.add_section_header("PANELS");
                builder.add_panel_list(&panel_entries, "×");
                builder.finish();
            }
        }

        // ── Main content — iterate ALL leaves of the current dock tree ──────────
        {
            use uzor::input::core::sense::Sense;

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
                let leaf_widget_id = format!("dock-leaf-{}", leaf_id.0);

                if matches!(kind, PanelKind::Watchlist) {
                    // Watchlist — wired through the real BlackboxPanel
                    // composite. The lib owns hover routing: when an
                    // overlay (dropdown / popup / modal) is over the
                    // panel, the composite suppresses PointerMove.
                    use std::cell::RefCell;
                    use uzor::ui::widgets::composite::blackbox_panel::{
                        input::register_layout_manager_blackbox_panel,
                        settings::BlackboxPanelSettings,
                        state::BlackboxState,
                        types::{BlackboxHandler, BlackboxRenderKind, BlackboxView},
                    };
                    let mut taken = std::mem::take(&mut self.watchlist);
                    taken.set_panel_size((rect.width, rect.height));
                    let watchlist = RefCell::new(taken);
                    let mut bb_state = BlackboxState::default();
                    let mut view = BlackboxView {
                        title: None,
                        body: Box::new(|ctx, body_rect| {
                            BlackboxHandler::render(&*watchlist.borrow(), ctx, body_rect);
                        }),
                        handle_event: Box::new(|evt| {
                            BlackboxHandler::handle_event(&mut *watchlist.borrow_mut(), evt)
                        }),
                        sense: Sense::CLICK | Sense::HOVER | Sense::DRAG | Sense::SCROLL,
                    };
                    let slot = leaf_id.to_string();
                    let _ = register_layout_manager_blackbox_panel(
                        &mut self.layout, &mut render,
                        LayoutNodeId::ROOT, &slot, leaf_widget_id.clone(),
                        &mut bb_state, &mut view,
                        &BlackboxPanelSettings::default(),
                        &BlackboxRenderKind::Default,
                    );
                    // Drop the closures (and their borrows) before reclaiming watchlist.
                    drop(view);
                    self.watchlist = watchlist.into_inner();
                } else {
                    use uzor::ui::widgets::composite::blackbox_panel::input::register_layout_manager_stub_panel;
                    let _ = register_layout_manager_stub_panel(
                        &mut self.layout,
                        leaf_widget_id,
                        rect,
                        &LayerId::main(),
                    );
                    render_panel_body(kind, &mut render, rect, &self.watchlist);
                }

                if Some(*leaf_id) == active_leaf {
                    use uzor::ui::widgets::atomic::active_frame::render::draw_active_frame;
                    use uzor::ui::widgets::atomic::active_frame::types::{ActiveFrameKind, ActiveFrameView};
                    draw_active_frame(
                        &mut render,
                        &ActiveFrameView { rect, color: "#2962ff", width: 2.0 },
                        ActiveFrameKind::Stroke,
                    );
                }
            }
        }

        // ── Dock separators ──────────────────────────────────────────────────
        // Paint only — registration is owned by LayoutManager via
        // `register_dock_separators` (called after all composite registration
        // so overlays outrank separators in z-order hit-testing).
        {
            use uzor::docking::panels::SeparatorOrientation as DockSepOrient;
            let separators: Vec<_> = self.layout.panels().separators().iter().enumerate().map(|(i, s)| {
                let thickness = s.thickness_for_state() as f64;
                let (sx, sy, sw, sh) = match s.orientation {
                    DockSepOrient::Vertical => {
                        (s.position as f64 - thickness / 2.0, s.start as f64, thickness, s.length as f64)
                    }
                    DockSepOrient::Horizontal => {
                        (s.start as f64, s.position as f64 - thickness / 2.0, s.length as f64, thickness)
                    }
                };
                (i, sx, sy, sw, sh)
            }).collect();

            let dragging_sep = if let Some(DragTarget::SeparatorDrag { sep_idx, .. }) = self.drag_target {
                Some(sep_idx)
            } else {
                None
            };

            for (i, sx, sy, sw, sh) in &separators {
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
            // Body size per kind. Frame (modal_w, modal_h) = body + measure_chrome().
            let (body_w, body_h) = match self.modal_kind {
                ModalKind::L2             => (l2_demo_blackbox::L2_WIN_W, l2_demo_blackbox::L2_WIN_H),
                ModalKind::L1             => (320.0,    150.0),
                ModalKind::Settings       => (400.0,    250.0),
                ModalKind::Tags           => (480.0,    310.0),
                ModalKind::PlainDemo      => (380.0,    180.0),
                ModalKind::HeaderDemo     => (380.0,    180.0),
                ModalKind::TopTabsDemo    => (520.0,    320.0),
                ModalKind::SideTabsDemo   => (560.0,    320.0),
                ModalKind::WizardDemo     => (520.0,    320.0),
            };
            // Resolve render kind early so we can probe chrome overhead correctly.
            let probe_kind = match self.modal_kind {
                ModalKind::PlainDemo      => ModalRenderKind::Plain,
                ModalKind::HeaderDemo     => ModalRenderKind::WithHeader,
                ModalKind::TopTabsDemo    => ModalRenderKind::TopTabs,
                ModalKind::SideTabsDemo   => ModalRenderKind::SideTabs,
                ModalKind::WizardDemo     => ModalRenderKind::Wizard,
                _                         => ModalRenderKind::WithHeaderFooter,
            };
            let probe_btns = [
                FooterBtn { label: "Close", style: FooterBtnStyle::Ghost },
                FooterBtn { label: "Apply", style: FooterBtnStyle::Primary },
            ];
            let probe_view = ModalView {
                title: Some(""),
                tabs: &[],
                footer_buttons: &probe_btns,
                wizard_pages: &[],
                backdrop: BackdropKind::Dim,
                overflow: uzor::types::OverflowMode::Clip,
                resizable: false,
            };
            let (extra_w, extra_h) = measure_modal_chrome(
                &probe_view,
                &ModalSettings::default(),
                &probe_kind,
            );
            let measured_w = body_w + extra_w;
            let measured_h = body_h + extra_h;
            let modal_w = if self.modal_size_override.0 > 0.0 { self.modal_size_override.0 } else { measured_w };
            let modal_h = if self.modal_size_override.1 > 0.0 { self.modal_size_override.1 } else { measured_h };
            // Fix #10/#11: use modal state.position (dragged) instead of always centering.
            let default_x = (width as f64 / 2.0 - modal_w / 2.0).max(0.0);
            let default_y = (height as f64 / 2.0 - modal_h / 2.0).max(0.0);
            let modal_pos = self.layout.modal(&self.modal_h).position;
            let (frame_x, frame_y) = if modal_pos != (0.0, 0.0) {
                modal_pos
            } else {
                (default_x, default_y)
            };
            let modal_rect = Rect::new(frame_x, frame_y, modal_w, modal_h);

            let modal_kind = self.modal_kind;

            let title = match modal_kind {
                ModalKind::L2             => "L2 Widget Set",
                ModalKind::L1             => "L1 Custom Button",
                ModalKind::Settings       => "Settings",
                ModalKind::Tags           => "Dock Panels",
                ModalKind::PlainDemo      => "Plain (frame only)",
                ModalKind::HeaderDemo     => "WithHeader (no footer)",
                ModalKind::TopTabsDemo    => "TopTabs",
                ModalKind::SideTabsDemo   => "SideTabs",
                ModalKind::WizardDemo     => "Wizard",
            };
            let footer_btns = [
                FooterBtn { label: "Close", style: FooterBtnStyle::Ghost },
                FooterBtn { label: "Apply", style: FooterBtnStyle::Primary },
            ];
            // Per-kind tabs (for TopTabs / SideTabs).
            let toptabs_tabs   = ["General", "Network", "Storage", "Advanced"];
            let sidetabs_tabs  = ["Profile", "Account", "Privacy", "Notifications"];
            let wizard_pages_data: [WizardPageInfo; 3] = [
                WizardPageInfo { label: Some("Welcome") },
                WizardPageInfo { label: Some("Configure") },
                WizardPageInfo { label: Some("Review") },
            ];
            let render_kind = match modal_kind {
                ModalKind::PlainDemo      => ModalRenderKind::Plain,
                ModalKind::HeaderDemo     => ModalRenderKind::WithHeader,
                ModalKind::TopTabsDemo    => ModalRenderKind::TopTabs,
                ModalKind::SideTabsDemo   => ModalRenderKind::SideTabs,
                ModalKind::WizardDemo     => ModalRenderKind::Wizard,
                _                         => ModalRenderKind::WithHeaderFooter,
            };
            let tabs: &[&str] = match modal_kind {
                ModalKind::TopTabsDemo  => &toptabs_tabs,
                ModalKind::SideTabsDemo => &sidetabs_tabs,
                _                       => &[],
            };
            let wizard_pages_ref: &[WizardPageInfo] = match modal_kind {
                ModalKind::WizardDemo => &wizard_pages_data,
                _                     => &[],
            };
            // Overflow mode: chevrons make sense only inside the L2 widget
            // catalog (its content can be wider than the body). All other
            // demo modals fit their body — clip is enough.
            // Pick a different overflow strategy per kind so each demo modal
            // exercises one of the three reactions:
            //   • HeaderDemo   — Chevrons (paging arrows)
            //   • TopTabsDemo  — Scrollbar (vertical track)
            //   • SideTabsDemo — Compress (children scaled to fit; fallback
            //                    chevrons kick in if min-factor is reached)
            //   • L2           — Chevrons (legacy behaviour for catalog)
            //   • everything else — Clip (with auto-fallback to chevrons)
            let overflow_mode = match modal_kind {
                ModalKind::L2           => uzor::types::OverflowMode::Chevrons,
                ModalKind::HeaderDemo   => uzor::types::OverflowMode::Chevrons,
                ModalKind::TopTabsDemo  => uzor::types::OverflowMode::Scrollbar,
                ModalKind::SideTabsDemo => uzor::types::OverflowMode::Compress,
                _                       => uzor::types::OverflowMode::Clip,
            };
            let mut modal_view = ModalView {
                title: Some(title),
                tabs,
                footer_buttons: &footer_btns,
                wizard_pages: wizard_pages_ref,
                backdrop: BackdropKind::Dim,
                overflow: overflow_mode,
                resizable: true,
            };
            // Tell the composite the natural body content size BEFORE
            // it registers — `register_body_overflow` reads these to
            // decide whether to register vertical / horizontal chevrons.
            // Natural body content size per kind. These are the dimensions
            // that the body *would* like to occupy. When the modal is shrunk
            // below this, register_body_overflow falls back to chevrons even
            // when overflow is set to Clip.
            let (cw, ch): (f64, f64) = match modal_kind {
                ModalKind::L2           => (l2_demo_blackbox::L2_WIN_W, l2_demo_blackbox::L2_WIN_H),
                ModalKind::L1           => (360.0, 200.0),
                ModalKind::Settings     => (520.0, 360.0),
                ModalKind::Tags         => (520.0, 320.0),
                ModalKind::PlainDemo    => (360.0, 120.0),
                ModalKind::HeaderDemo   => (420.0, 180.0),
                ModalKind::TopTabsDemo  => (420.0, 220.0),
                ModalKind::SideTabsDemo => (480.0, 280.0),
                ModalKind::WizardDemo   => (480.0, 240.0),
            };
            {
                let h = self.modal_h.clone();
                let ms = self.layout.modal_mut(&h);
                ms.body_content_w = cw;
                ms.body_content_h = ch;
            }
            let modal_node = register_layout_manager_modal(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "modal-overlay",
                &self.modal_h.clone(),
                modal_rect,
                None,
                &mut modal_view,
                &ModalSettings::default(),
                &render_kind,
            );
            // Draw modal body content inline.
            // `frame_rect` = full overlay rect (includes chrome).
            // `body_rect` = content area carved out by the composite.
            if let Some(frame_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let body_rect_raw = uzor::ui::widgets::composite::modal::render::body_rect(
                    frame_rect,
                    &modal_view,
                    &ModalSettings::default(),
                    &render_kind,
                );
                // Apply scroll offset so dragging the scrollbar moves the
                // content. Composite paints the scrollbar; body content
                // is shifted by scroll offsets and clipped to body_rect.
                let (scroll_y, scroll_x) = {
                    let ms = self.layout.modal(&self.modal_h);
                    (ms.scroll.offset, ms.body_scroll_x)
                };
                let body_rect = Rect::new(
                    body_rect_raw.x - scroll_x,
                    body_rect_raw.y - scroll_y,
                    body_rect_raw.width  + scroll_x,
                    body_rect_raw.height + scroll_y,
                );
                // Hard-clip everything we draw inside body_rect_raw so
                // the overflow can't bleed past the modal frame.
                render.save();
                render.clip_rect(
                    body_rect_raw.x, body_rect_raw.y,
                    body_rect_raw.width, body_rect_raw.height,
                );
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
                        let btn_id = unsafe_widget_id("l1-mybtn");
                        // Button is atomic — use register_atomic.
                        // Sticky chevrons require a composite parent; to add a
                        // chevron to a button, wrap it in a composite container
                        // (e.g. WidgetKind::Panel) or use a ToolbarItem::SplitIconButton.
                        let btn_composite_id = self.layout.ctx_mut().input.register_atomic(
                            btn_id.clone(),
                            WidgetKind::Button,
                            btn_r,
                            Sense::CLICK | Sense::HOVER,
                            &layer,
                        );
                        // Three-state colour via coordinator widget_state — no manual fields needed.
                        let btn_state = self.layout.ctx_mut().input.widget_state(&btn_id);
                        let btn_color = match btn_state {
                            WidgetState::Pressed  => "#1a40c8",
                            WidgetState::Hovered  => "#4080ff",
                            _                     => "#3769af",
                        };
                        render.set_fill_color(btn_color);
                        render.fill_rounded_rect(btn_r.x, btn_r.y, btn_r.width, btn_r.height, 6.0);
                        label(&mut render, btn_r, "Click me (L1 custom)", TextAlign::Center, "#ffffff");

                        // Sticky chevron requires a composite parent widget.
                        // Button is atomic — chevron attachment removed.
                        // To add a chevron to a button, use ToolbarItem::SplitIconButton
                        // or wrap the button in a composite Panel container.
                        let _ = btn_composite_id;
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
                        let body_inner = Rect::new(
                            body_rect.x + 16.0,
                            body_rect.y + 8.0,
                            body_rect.width - 32.0,
                            body_rect.height - 16.0,
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
                        // Phase D: L2 demo now lives entirely inside L2DemoBlackbox.
                        // body_rect is in screen coords — render directly.
                        use uzor::ui::widgets::composite::blackbox_panel::types::BlackboxHandler;
                        self.l2_demo.set_panel_size((body_rect.width, body_rect.height));
                        self.l2_demo.render(&mut render, body_rect);
                    }
                    ModalKind::PlainDemo => {
                        // No header/footer — caller draws everything inside the body.
                        render.set_fill_color("#1a1a22");
                        render.fill_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height);
                        render.set_fill_color("#d1d4dc");
                        render.set_font("14px sans-serif");
                        render.set_text_align(TextAlign::Center);
                        render.set_text_baseline(TextBaseline::Middle);
                        render.fill_text(
                            "ModalRenderKind::Plain — frame only",
                            body_rect.x + body_rect.width / 2.0,
                            body_rect.y + body_rect.height / 2.0 - 12.0,
                        );
                        render.set_fill_color("#7080a0");
                        render.set_font("12px sans-serif");
                        render.fill_text(
                            "Click outside to dismiss.",
                            body_rect.x + body_rect.width / 2.0,
                            body_rect.y + body_rect.height / 2.0 + 12.0,
                        );
                    }
                    ModalKind::HeaderDemo => {
                        render.set_fill_color("#d1d4dc");
                        render.set_font("13px sans-serif");
                        render.set_text_align(TextAlign::Left);
                        render.set_text_baseline(TextBaseline::Top);
                        render.fill_text(
                            "Header-only modal — no footer buttons.",
                            body_rect.x + 16.0,
                            body_rect.y + 16.0,
                        );
                        render.set_fill_color("#7080a0");
                        render.set_font("12px sans-serif");
                        render.fill_text(
                            "Drag the title bar to move me. Click X or outside to close.",
                            body_rect.x + 16.0,
                            body_rect.y + 40.0,
                        );

                        // ── Live text input ────────────────────────────────
                        let ti_id = unsafe_widget_id("modal:header_demo:text_input");
                        let ti_rect = Rect::new(
                            body_rect.x + 16.0,
                            body_rect.y + 72.0,
                            (body_rect.width - 32.0).max(120.0),
                            32.0,
                        );
                        self.layout.ctx_mut().input.register_text_field(
                            ti_id.clone(),
                            ti_rect,
                            StoreTextFieldConfig::text(),
                        );
                        let text_str = self.layout.ctx().input.text_fields()
                            .text(&ti_id).to_owned();
                        let cursor_pos = self.layout.ctx().input.text_fields().cursor(&ti_id);
                        let selection  = self.layout.ctx().input.text_fields().selection_range(&ti_id);
                        let focused    = self.layout.ctx().input.text_fields().is_focused(&ti_id);
                        let ti_view = InputView {
                            text:        text_str.as_str(),
                            placeholder: "Type here...",
                            cursor:      cursor_pos,
                            selection,
                            focused,
                            disabled:    false,
                            input_type:  InputType::Text,
                        };
                        let ti_state = if focused { WidgetState::Active } else { WidgetState::Normal };
                        let parent_node = modal_node.map(|n| n.0).unwrap_or(LayoutNodeId::ROOT);
                        let _ = register_layout_manager_text_input(
                            &mut self.layout,
                            &mut render,
                            parent_node,
                            ti_id,
                            ti_rect,
                            ti_state,
                            &ti_view,
                            &TextInputSettings::with_config(TiTextFieldConfig::text()),
                        );
                    }
                    ModalKind::TopTabsDemo => {
                        render.set_fill_color("#d1d4dc");
                        render.set_font("13px sans-serif");
                        render.set_text_align(TextAlign::Left);
                        render.set_text_baseline(TextBaseline::Top);
                        render.fill_text(
                            "TopTabs — horizontal tab strip below the header.",
                            body_rect.x + 16.0,
                            body_rect.y + 16.0,
                        );
                        render.set_fill_color("#7080a0");
                        render.set_font("12px sans-serif");
                        render.fill_text(
                            "Tabs (decorative): General / Network / Storage / Advanced.",
                            body_rect.x + 16.0,
                            body_rect.y + 40.0,
                        );
                    }
                    ModalKind::SideTabsDemo => {
                        // Read the composite-computed compress factor and
                        // apply it to font sizes + offsets.  Identity (1.0)
                        // outside Compress mode so this code is safe to keep
                        // unconditionally.
                        let factor = self.layout.modal(&self.modal_h).compress_factor();
                        let scale = factor.sx.min(factor.sy);
                        let pad   = 16.0 * factor.sx;
                        let font_a = (13.0 * scale).max(8.0);
                        let font_b = (12.0 * scale).max(8.0);
                        render.set_fill_color("#d1d4dc");
                        render.set_font(&format!("{}px sans-serif", font_a as i32));
                        render.set_text_align(TextAlign::Left);
                        render.set_text_baseline(TextBaseline::Top);
                        render.fill_text(
                            "SideTabs (Compress) — resize the modal smaller; the body text scales down.",
                            body_rect.x + pad,
                            body_rect.y + pad,
                        );
                        render.set_fill_color("#7080a0");
                        render.set_font(&format!("{}px sans-serif", font_b as i32));
                        render.fill_text(
                            "Sidebar tabs (decorative): Profile / Account / Privacy / Notifications.",
                            body_rect.x + pad,
                            body_rect.y + pad + 24.0 * factor.sy,
                        );
                        render.fill_text(
                            &format!("compress factor: ({:.2}, {:.2})", factor.sx, factor.sy),
                            body_rect.x + pad,
                            body_rect.y + pad + 48.0 * factor.sy,
                        );
                    }
                    ModalKind::WizardDemo => {
                        render.set_fill_color("#d1d4dc");
                        render.set_font("13px sans-serif");
                        render.set_text_align(TextAlign::Left);
                        render.set_text_baseline(TextBaseline::Top);
                        render.fill_text(
                            "Wizard — multi-step page flow with Back/Next nav.",
                            body_rect.x + 16.0,
                            body_rect.y + 16.0,
                        );
                        render.set_fill_color("#7080a0");
                        render.set_font("12px sans-serif");
                        render.fill_text(
                            "Pages: Welcome → Configure → Review.",
                            body_rect.x + 16.0,
                            body_rect.y + 40.0,
                        );
                    }
                }
                // Close the body clip established before the per-kind branch.
                render.restore();

                // Two-pass body finish: paint overflow overlays, then re-register
                // overflow hit-zones after body content so they outrank body widgets.
                {
                    // Take/return state via mem::replace because modal_body_finish
                    // takes &mut LayoutManager + &mut ModalState concurrently.
                    let h = self.modal_h.clone();
                    let mut ms = std::mem::take(self.layout.modal_mut(&h));
                    modal_input::modal_body_finish(
                        &mut self.layout,
                        &mut render,
                        frame_rect,
                        &mut ms,
                        &modal_view,
                        &ModalSettings::default(),
                        &render_kind,
                    );
                    *self.layout.modal_mut(&self.modal_h.clone()) = ms;
                }
            }
        }

        // Demo A (l2-connect popup) and Demo D (l2-4dir popup) removed — simplified in Phase D.

        // ── Context menu ──────────────────────────────────────────────────────
        let ctx_menu_is_open = self.layout.context_menu(&self.ctx_menu_h).is_open;
        if ctx_menu_is_open {
            let (ctx_x, ctx_y) = {
                let s = self.layout.context_menu(&self.ctx_menu_h);
                (s.x, s.y)
            };
            let items = [
                ContextMenuItem { action: "ctx-copy",     label: "Copy",       icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-paste",    label: "Paste",      icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-delete",   label: "Delete",     icon: None, danger: true,  separator_after: true,  enabled: true },
                ContextMenuItem { action: "ctx-props",    label: "Properties", icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-settings", label: "Settings",   icon: None, danger: false, separator_after: false, enabled: true },
            ];
            let menu_h = items.len() as f64 * 28.0 + 16.0;
            let ctx_menu_rect = Rect::new(ctx_x, ctx_y, 170.0, menu_h);
            let mut ctx_menu_view = ContextMenuView { items: &items, target_id: None, title: None };
            register_layout_manager_context_menu(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "ctx-menu-overlay",
                &self.ctx_menu_h.clone(),
                ctx_menu_rect,
                None,
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
        open_dropdown_flat(
            &mut self.layout, &mut render, LayoutNodeId::ROOT,
            "dd-file-overlay", &self.dd_file_h.clone(),
            &file_items, &DropdownSettings::default(),
        );

        let view_items = [
            DropdownItem::Item { id: "view-sidebar", label: "Toggle Sidebar", icon: None, right: DropdownItemRight::Toggle(self.sidebar_open),         disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "view-toolbar", label: "Show Toolbar",   icon: None, right: DropdownItemRight::Toggle(self.left_toolbar_visible), disabled: false, danger: false, accent_color: None },
        ];
        open_dropdown_flat(
            &mut self.layout, &mut render, LayoutNodeId::ROOT,
            "dd-view-overlay", &self.dd_view_h.clone(),
            &view_items, &DropdownSettings::default(),
        );

        let help_items = [
            DropdownItem::Header { label: "Existing demos" },
            DropdownItem::Item { id: "modals-l2",       label: "L2 Widget Set",  icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-l1",       label: "L1 Big Button",  icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-panels",   label: "Dock Panels",    icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-settings", label: "Settings",       icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Separator,
            DropdownItem::Header { label: "ModalRenderKind catalog" },
            DropdownItem::Item { id: "modals-plain",    label: "Plain",          icon: None, right: DropdownItemRight::Shortcut("frame only"),    disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-header",   label: "WithHeader",     icon: None, right: DropdownItemRight::Shortcut("title + drag"),  disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-toptabs",  label: "TopTabs",        icon: None, right: DropdownItemRight::Shortcut("tabs across"),   disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-sidetabs", label: "SideTabs",       icon: None, right: DropdownItemRight::Shortcut("icon sidebar"),  disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "modals-wizard",   label: "Wizard",         icon: None, right: DropdownItemRight::Shortcut("multi-step"),    disabled: false, danger: false, accent_color: None },
        ];
        open_dropdown_flat(
            &mut self.layout, &mut render, LayoutNodeId::ROOT,
            "dd-help-overlay", &self.dd_help_h.clone(),
            &help_items, &DropdownSettings::default(),
        );

        // ── Sidebar dropdown — toggles to spawn / hide demo sidebars ─────────
        let main_open = self.sidebar_open;
        let sidebar_items = [
            DropdownItem::Header { label: "Spawn extra sidebars" },
            DropdownItem::Item { id: "sb-toggle-main",   label: "Main (Left)",    icon: None, right: DropdownItemRight::Toggle(main_open),               disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "sb-spawn-right",   label: "Right sidebar",  icon: None, right: DropdownItemRight::Toggle(self.demo_sidebar_right), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "sb-spawn-top",     label: "Top sidebar",    icon: None, right: DropdownItemRight::Toggle(self.demo_sidebar_top),   disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "sb-spawn-bottom",  label: "Bottom sidebar", icon: None, right: DropdownItemRight::Toggle(self.demo_sidebar_bottom),disabled: false, danger: false, accent_color: None },
            DropdownItem::Separator,
            DropdownItem::Item { id: "sb-overlay-mode",  label: "Overlay mode",   icon: None, right: DropdownItemRight::Toggle(self.demo_overlay_mode),  disabled: false, danger: false, accent_color: None },
        ];
        open_dropdown_flat(
            &mut self.layout, &mut render, LayoutNodeId::ROOT,
            "dd-sidebar-overlay", &self.dd_sidebar_h.clone(),
            &sidebar_items, &DropdownSettings::default(),
        );

        // ── Toolbar dropdown — toggles to spawn / hide demo toolbars ─────────
        let left_main_visible = self.left_toolbar_visible;
        let toolbar_items_dd = [
            DropdownItem::Header { label: "Spawn extra toolbars" },
            DropdownItem::Item { id: "tb-toggle-main",  label: "Main (Top)",     icon: None, right: DropdownItemRight::Toggle(true),                    disabled: true, danger: false, accent_color: None },
            DropdownItem::Item { id: "tb-toggle-left",  label: "Left (Vertical)",icon: None, right: DropdownItemRight::Toggle(left_main_visible),       disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "tb-spawn-left2",  label: "Left2 (extra)",  icon: None, right: DropdownItemRight::Toggle(self.demo_toolbar_left2), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "tb-spawn-right",  label: "Right toolbar",  icon: None, right: DropdownItemRight::Toggle(self.demo_toolbar_right), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "tb-spawn-bottom", label: "Bottom toolbar", icon: None, right: DropdownItemRight::Toggle(self.demo_toolbar_bottom),disabled: false, danger: false, accent_color: None },
        ];
        open_dropdown_flat(
            &mut self.layout, &mut render, LayoutNodeId::ROOT,
            "dd-toolbar-overlay", &self.dd_toolbar_h.clone(),
            &toolbar_items_dd, &DropdownSettings::default(),
        );

        // ── Popup templates dropdown (Plain | Custom) ──────────────────────────
        // Both rows are submenu triggers — Plain opens its L2 on hover,
        // Custom opens its L2 only on chevron click. Demonstrates both
        // SubmenuTrigger variants in one place.
        let popup_items_dd = [
            DropdownItem::Header { label: "Popup kind" },
            DropdownItem::Submenu {
                id: "popup-plain",
                label: "Plain",
                icon: None,
                trigger: uzor::ui::widgets::composite::dropdown::types::SubmenuTrigger::Hover,
                chevron_hover: false,
            },
            DropdownItem::Submenu {
                id: "popup-custom",
                label: "Custom",
                icon: None,
                trigger: uzor::ui::widgets::composite::dropdown::types::SubmenuTrigger::ChevronClick,
                chevron_hover: true,
            },
        ];
        let popup_plain_sub_items = [
            DropdownItem::Item {
                id: "popup-plain", label: "Plain popup",
                icon: None,
                right: DropdownItemRight::Shortcut("text"),
                disabled: false, danger: false, accent_color: None,
            },
        ];
        let popup_custom_sub_items = [
            DropdownItem::Item {
                id: "popup-custom-grid", label: "Color grid 4×4",
                icon: None,
                right: DropdownItemRight::Shortcut("buttons"),
                disabled: false, danger: false, accent_color: None,
            },
            // Debug stubs — exercise multi-row submenu hover.
            DropdownItem::Item {
                id: "popup-stub-a", label: "Stub A",
                icon: None, right: DropdownItemRight::None,
                disabled: false, danger: false, accent_color: None,
            },
            DropdownItem::Item {
                id: "popup-stub-b", label: "Stub B",
                icon: None, right: DropdownItemRight::None,
                disabled: false, danger: false, accent_color: None,
            },
            DropdownItem::Item {
                id: "popup-stub-c", label: "Stub C",
                icon: None, right: DropdownItemRight::None,
                disabled: false, danger: false, accent_color: None,
            },
        ];
        {
            let dd_popup_open = self.layout.dropdown(&self.dd_popup_h).open;
            if dd_popup_open {
                let (hovered_id, open_id, origin, anchor_rect, position_override) = {
                    let s = self.layout.dropdown(&self.dd_popup_h);
                    (s.hovered_id.clone(), s.submenu_open.clone(), s.effective_origin(), s.anchor_rect, s.open_position_override)
                };
                let (pw, ph) = measure_flat(&popup_items_dd, &DropdownSettings::default());
                let submenu_items = match open_id.as_deref() {
                    Some("popup-plain")  => Some(("popup-plain",  &popup_plain_sub_items[..])),
                    Some("popup-custom") => Some(("popup-custom", &popup_custom_sub_items[..])),
                    _                    => None,
                };
                let mut dd_view = DropdownView {
                    anchor: anchor_rect,
                    position_override,
                    open: true,
                    kind: DropdownViewKind::Flat {
                        items: &popup_items_dd,
                        hovered_id: hovered_id.as_deref(),
                        submenu_items,
                        submenu_hovered_id: None,
                    },
                    size_mode: uzor::types::SizeMode::AutoFit,
                    overflow: uzor::types::OverflowMode::Clip,
                    submenu_width: uzor::ui::widgets::composite::dropdown::types::SubmenuWidth::Auto,
                };
                register_layout_manager_dropdown(
                    &mut self.layout, &mut render,
                    LayoutNodeId::ROOT, "dd-popup-overlay", &self.dd_popup_h.clone(),
                    Rect::new(origin.0, origin.1, pw, ph), None,
                    &mut dd_view,
                    &DropdownSettings::default(),
                    DropdownRenderKind::Flat,
                );
            }
        }

        // ── Demo popup ────────────────────────────────────────────────────────
        // Two flavours, both via the slim popup composite:
        //   0 (Plain)  → text body, no inner widgets
        //   1 (Custom) → caller-driven 4×4 color grid where every cell is
        //                registered as a child Button so it hovers/clicks
        //                through the dispatcher
        if let Some(kind_idx) = self.popup_kind {
            use uzor::ui::widgets::composite::popup::render::body_rect;
            let popup_settings = PopupSettings::default();
            let pad = popup_settings.style.padding();

            match kind_idx {
                0 => {
                    // Plain popup — text body sized from font metric.
                    let body_w = 220.0_f64;
                    let body_h = 60.0_f64;
                    let popup_w = body_w + pad * 2.0;
                    let popup_h = body_h + pad * 2.0;
                    let px = (width as f64 - popup_w) / 2.0;
                    let py = (height as f64 - popup_h) / 2.0;
                    let mut v = PopupView {
                        origin: (px, py),
                        anchor: None,
                        backdrop: PopupBackdrop::Dim,
                        kind: PopupViewKind::Plain,
                        size_mode: uzor::types::SizeMode::AutoFit,
                        overflow: uzor::types::OverflowMode::Clip,
                    };
                    let _ = register_layout_manager_popup(
                        &mut self.layout, &mut render,
                        LayoutNodeId::ROOT,
                        "demo-popup-overlay", &self.demo_popup_h.clone(),
                        Rect::new(px, py, popup_w, popup_h), None,
                        &mut v,
                        &popup_settings, PopupRenderKind::Plain,
                    );
                    if let Some(frame) = self.layout.rect_for_overlay("demo-popup-overlay") {
                        let body = body_rect(frame, &popup_settings);
                        label(&mut render, body, "Plain popup body", TextAlign::Center, "#d1d4dc");
                    }
                }
                _ => {
                    // Custom popup — 4×4 grid of color buttons. Every cell
                    // is a real child button so it dispatches through the
                    // coordinator (hover / click / focus).
                    let palette: [&str; 16] = [
                        "#ef5350","#f59e0b","#fbbf24","#10b981","#22d3ee","#2962ff","#7c3aed","#ec4899",
                        "#94a3b8","#fde68a","#86efac","#67e8f9","#93c5fd","#c4b5fd","#fbcfe8","#1f2937",
                    ];
                    let cols = 4_usize;
                    let cell = 28.0_f64;
                    let gap  = 6.0_f64;
                    let rows = (palette.len() + cols - 1) / cols;
                    let body_w = cols as f64 * cell + (cols as f64 - 1.0) * gap;
                    let body_h = rows as f64 * cell + (rows as f64 - 1.0) * gap;
                    let popup_w = body_w + pad * 2.0;
                    let popup_h = body_h + pad * 2.0;
                    let px = (width as f64 - popup_w) / 2.0;
                    let py = (height as f64 - popup_h) / 2.0;
                    // Use Plain so the composite paints the chrome — Custom
                    // would skip the frame draw.
                    let mut v = PopupView {
                        origin: (px, py),
                        anchor: None,
                        backdrop: PopupBackdrop::Dim,
                        kind: PopupViewKind::Plain,
                        size_mode: uzor::types::SizeMode::AutoFit,
                        overflow: uzor::types::OverflowMode::Clip,
                    };
                    let _ = register_layout_manager_popup(
                        &mut self.layout, &mut render,
                        LayoutNodeId::ROOT,
                        "demo-popup-overlay", &self.demo_popup_h.clone(),
                        Rect::new(px, py, popup_w, popup_h), None,
                        &mut v,
                        &popup_settings, PopupRenderKind::Plain,
                    );
                    // Caller body — register every cell + paint its swatch via register_popup_grid.
                    if let Some(frame) = self.layout.rect_for_overlay("demo-popup-overlay") {
                        use uzor::ui::widgets::composite::popup::input::{register_popup_grid, PopupGridCell};
                        let body = body_rect(frame, &popup_settings);
                        let cell_ids: Vec<String> = (0..palette.len()).map(|i| format!("demo-popup-cell-{i}")).collect();
                        let grid_cells: Vec<PopupGridCell<'_>> = palette.iter().zip(cell_ids.iter())
                            .map(|(color, id)| PopupGridCell { id: id.as_str(), color })
                            .collect();
                        register_popup_grid(&mut self.layout, &mut render, "demo-popup-widget", body, &grid_cells, cols, cell, gap);
                    }
                }
            }
        }

        // ── Dock separators — z-aware hit-test registration ───────────────────
        // Owned by LayoutManager. Called after composites so overlays already
        // pushed their (modal=true) layers; separator clicks under an open
        // overlay are blocked by z-ordered hit-test.
        self.layout.register_dock_separators(&LayerId::main());

        // Register prefix patterns for dock-leaf clicks and close buttons so
        // the dispatcher surfaces DockLeafClicked / DockLeafClosedByIndex
        // instead of raw Unhandled ids.
        // Also register Indexed patterns for L2 modal radio/swatch/tab buttons.
        {
            use uzor::layout::EventBuilder;
            self.layout.dispatcher_mut().on_prefix("dock-leaf-close-", EventBuilder::DockLeafCloseFromSuffix);
            self.layout.dispatcher_mut().on_prefix("dock-leaf-",       EventBuilder::DockLeafFromSuffix);
            // L2 modal indexed widgets (radio, swatch, tab — all have numeric suffixes)
            self.layout.dispatcher_mut().on_prefix("l2-radio-",    EventBuilder::IndexedFromSuffix { base: "l2-radio".into()    });
            self.layout.dispatcher_mut().on_prefix("l2-swatch-",   EventBuilder::IndexedFromSuffix { base: "l2-swatch".into()   });
            self.layout.dispatcher_mut().on_prefix("l2-sub-tab-",  EventBuilder::IndexedFromSuffix { base: "l2-sub-tab".into()  });
            self.layout.dispatcher_mut().on_prefix("l2-tab-",      EventBuilder::IndexedFromSuffix { base: "l2-tab".into()      });
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
                        id.as_str(), resp.clicked, resp.hovered, resp.scrolled, resp.dragged);
                }
            }
        }

        // Process coordinator responses (l2 scrollbar is now inside the blackbox — no orphan ids here)

        // Update popup based on hovered widget.
        // Items with popup_on_hover:true open on hover — derive the set from the
        // toolbar item definitions rather than a separate hardcoded allowlist.
        let hovered_id = self.layout.ctx_mut().input.hovered_widget().map(|id| id.as_str().to_owned());
        self.popup_item = hovered_id.as_deref().and_then(|hovered| {
            // Iterate the top toolbar items and check popup_on_hover flag.
            let top_items: &[(&str, bool)] = &[
                ("tb-view", true),
                ("tb-help", true),
            ];
            top_items.iter()
                .find(|(id, on_hover)| *on_hover && *id == hovered)
                .map(|(id, _)| id.to_string())
        });

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
        _clicked_id: Option<WidgetId>,
        event_loop: &ActiveEventLoop,
    ) {
        eprintln!("[LEFT_UP] pos=({:.1},{:.1}) modal_open={} dropdown_open=(file:{} view:{} help:{}) ctx_menu_open={}",
            x, y,
            self.modal_open,
            self.layout.dropdown(&self.dd_file_h).open,
            self.layout.dropdown(&self.dd_view_h).open,
            self.layout.dropdown(&self.dd_help_h).open,
            self.layout.context_menu(&self.ctx_menu_h).is_open,
        );

        use uzor::layout::ClickOutcome;
        match self.layout.handle_click((x, y)) {
            ClickOutcome::DismissOverlay(overlay_handle) => {
                eprintln!("[DISMISS] {:?}", overlay_handle);
                match overlay_handle {
                    OverlayHandle::Modal(_) => { self.modal_open = false; }
                    OverlayHandle::ContextMenu(_) => {
                        self.layout.context_menu_mut(&self.ctx_menu_h.clone()).close();
                    }
                    OverlayHandle::Popup(ref ph) => {
                        // Discriminate by comparing handle identity.
                        if *ph == self.demo_popup_h {
                            self.popup_kind = None;
                        }
                    }
                    OverlayHandle::Dropdown(ref dh) => {
                        // Close the specific dropdown by handle identity.
                        let h = dh.clone();
                        self.layout.dropdown_mut(&h).close();
                    }
                    OverlayHandle::Other { .. } => {}
                }
            }
            ClickOutcome::DispatchEvent(event) => {
                self.dispatch_event(event, x, y, event_loop);
            }
            ClickOutcome::Unhandled { .. } => {
                // No overlay to dismiss, no coord widget hit.
                // Fall through to legacy geometry-based handlers below.
                self.on_left_up_geometry(x, y, event_loop);
            }
        }
    }

    /// Handle a click that resolved to a semantic DispatchEvent from the
    /// LayoutManager dispatch table.  Called from `on_left_up`'s
    /// `ClickOutcome::DispatchEvent` arm.
    fn dispatch_event(
        &mut self,
        event: uzor::layout::DispatchEvent,
        x: f64,
        y: f64,
        event_loop: &ActiveEventLoop,
    ) {
        use uzor::layout::DispatchEvent;

        let viewport = {
            let cfg = &self.surface.config;
            (cfg.width as f64, cfg.height as f64)
        };
        let cursor = (x, y);

        match event {
            // Mouse-up on a separator drag-handle — drag was started on
            // mouse-down and is ended in on_mouse_up's release path.
            DispatchEvent::DockSeparatorDragStarted { .. } => return,
            // Mouse-up on a resize handle — drag started on mouse-down,
            // ended elsewhere; no action on release click.
            DispatchEvent::ResizeHandleDragStarted { .. } => return,
            DispatchEvent::ModalCloseRequested(_) => {
                eprintln!("[DISPATCHER] ModalCloseRequested");
                self.modal_open = false;
            }
            DispatchEvent::ModalTabClicked { index, .. } => {
                eprintln!("[DISPATCHER] ModalTabClicked index={index}");
                self.layout.modal_mut(&self.modal_h.clone()).active_tab = index;
            }
            DispatchEvent::ModalWizardNext(_) => {
                eprintln!("[DISPATCHER] ModalWizardNext");
                let last = 2;
                let h = self.modal_h.clone();
                let ms = self.layout.modal_mut(&h);
                if ms.current_page < last {
                    ms.current_page += 1;
                } else {
                    self.modal_open = false;
                }
            }
            DispatchEvent::ModalWizardBack(_) => {
                eprintln!("[DISPATCHER] ModalWizardBack");
                let h = self.modal_h.clone();
                let ms = self.layout.modal_mut(&h);
                if ms.current_page > 0 {
                    ms.current_page -= 1;
                }
            }
            DispatchEvent::DropdownItemClicked { dropdown, item_id } => {
                eprintln!("[DISPATCHER] DropdownItemClicked item={}", item_id);
                let dd_name = if dropdown == self.dd_file_h    { "dd-file-widget" }
                    else if dropdown == self.dd_view_h         { "dd-view-widget" }
                    else if dropdown == self.dd_help_h         { "dd-help-widget" }
                    else if dropdown == self.dd_sidebar_h      { "dd-sidebar-widget" }
                    else if dropdown == self.dd_toolbar_h      { "dd-toolbar-widget" }
                    else if dropdown == self.dd_popup_h        { "dd-popup-widget" }
                    else                                       { "unknown" };
                self.handle_dropdown_item(dd_name, item_id.as_str(), event_loop);
            }
            DispatchEvent::ToolbarItemClicked { toolbar, item_id } => {
                eprintln!("[DISPATCHER] ToolbarItemClicked item={}", item_id);
                let tb_name = if toolbar == self.top_toolbar_h         { "top-toolbar-widget" }
                    else if toolbar == self.left_vtoolbar_h            { "left-vtoolbar-widget" }
                    else if toolbar == self.demo_toolbar_left2_h       { "demo-toolbar-left2-widget" }
                    else if toolbar == self.demo_toolbar_right_h       { "demo-toolbar-right-widget" }
                    else if toolbar == self.demo_toolbar_bottom_h      { "demo-toolbar-bottom-widget" }
                    else                                               { "unknown" };
                self.handle_toolbar_item(tb_name, item_id.as_str());
            }
            DispatchEvent::ContextMenuItemClicked { item_index, .. } => {
                eprintln!("[DISPATCHER] ContextMenuItemClicked index={item_index}");
                self.handle_ctx_menu_item(item_index);
            }
            DispatchEvent::ChromeTabClicked { tab_index } => {
                eprintln!("[DISPATCHER] ChromeTabClicked index={tab_index}");
                self.switch_tab(tab_index);
            }
            DispatchEvent::ChromeTabClosed { tab_index } => {
                eprintln!("[DISPATCHER] ChromeTabClosed index={tab_index}");
            }
            DispatchEvent::ChromeNewTab => {
                eprintln!("[DISPATCHER] ChromeNewTab");
            }
            DispatchEvent::ChromeWindowControl { control } => {
                eprintln!("[DISPATCHER] ChromeWindowControl {:?}", control);
                use uzor::layout::ChromeWindowControl as CC;
                match control {
                    CC::CloseApp        => { self.window.request_redraw(); std::process::exit(0); }
                    CC::Minimize        => { self.window.set_minimized(true); }
                    CC::MaximizeRestore => { self.window.set_maximized(!self.window.is_maximized()); }
                    _ => {}
                }
            }
            DispatchEvent::StickyChevronClicked { host_id } => {
                eprintln!("[DISPATCHER] StickyChevronClicked host={}", host_id.as_str());
                match host_id.as_str() {
                    _ => {}
                }
            }
            DispatchEvent::StickyChevronAtSlotClicked { host_id, slot } => {
                eprintln!("[DISPATCHER] StickyChevronAtSlotClicked host={} slot={} — no-op (L2 removed)", host_id.as_str(), slot);
            }
            DispatchEvent::Indexed { ref base, n } => {
                eprintln!("[DISPATCHER] Indexed base={base} n={n} — no-op");
            }
            DispatchEvent::DockLeafClicked { leaf_id } => {
                eprintln!("[DISPATCHER] DockLeafClicked leaf_id={}", leaf_id.0);
                let is_watchlist = self.layout.panels().tree().leaf(leaf_id)
                    .and_then(|l| l.panels.first())
                    .map(|p| p.kind == PanelKind::Watchlist)
                    .unwrap_or(false);
                if is_watchlist {
                    if let Some(&rect) = self.layout.panels().panel_rects().get(&leaf_id) {
                        let r = Rect::new(rect.x as f64, rect.y as f64, rect.width as f64, rect.height as f64);
                        let symbols = ["BTC/USDT", "ETH/USDT", "SOL/USDT", "BNB/USDT", "ADA/USDT"];
                        let local = (x - r.x, y - r.y);
                        if let Some(row) = watchlist_blackbox::click_row(&self.watchlist, (r.width, r.height), local.0, local.1) {
                            if let Some(sym) = symbols.get(row) {
                                println!("[L3] watchlist row clicked: {sym}");
                            }
                        }
                    }
                }
                self.layout.panels_mut().set_active_leaf(leaf_id);
            }
            DispatchEvent::DockLeafClosedByIndex { leaf_idx } => {
                eprintln!("[DISPATCHER] DockLeafClosedByIndex idx={leaf_idx}");
                let mut leaves: Vec<uzor::docking::panels::LeafId> = self
                    .layout
                    .panels()
                    .panel_rects()
                    .keys()
                    .copied()
                    .collect();
                leaves.sort_by_key(|l| l.0);
                if leaves.len() > 1 {
                    if let Some(&leaf_id) = leaves.get(leaf_idx) {
                        self.layout.panels_mut().tree_mut().remove_leaf(leaf_id);
                    }
                }
            }
            ev => {
                // Run the event through every composite's consume_event via
                // LayoutManager::consume_event — it iterates the per-frame
                // composite registry using the states stored in layout maps.
                let opt_ev = self.layout.consume_event(ev, cursor, viewport);
                // opt_ev still Some(Unhandled(id)) → app-specific id routing.
                if let Some(DispatchEvent::Unhandled(ref id)) = opt_ev {
                    let id_str = id.as_str();
                    // ── Sidebar: spawn kind radio buttons
                    if let Some(kind_str) = id_str.strip_prefix("spawn-kind-") {
                        eprintln!("[DISPATCH] spawn-kind→{kind_str}");
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
                    // ── Sidebar: spawn split radio buttons
                    match id_str {
                        "spawn-split-horiz" => { self.spawn_split = SpawnSplit::SplitRight;  return; }
                        "spawn-split-vert"  => { self.spawn_split = SpawnSplit::SplitBottom; return; }
                        "spawn-split-grid"  => { self.spawn_split = SpawnSplit::Grid2x2;     return; }
                        _ => {}
                    }
                    // ── Sidebar: Spawn button
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
                    // ── L1-modal custom button
                    if id_str == "l1-mybtn" {
                        eprintln!("[DISPATCH] l1-mybtn");
                        println!("[L3] L1 custom button clicked");
                        return;
                    }
                    // Composite frame clicks (chrome, sidebar, toolbar, modal) are no-ops.
                    // They fall through the match without matching and are silently dropped.
                    eprintln!("[DISPATCH] Unhandled id={id_str} — no-op");
                }
                // Consumed by composite (opt_ev is None) or a semantic event that
                // was handled in an arm above — nothing more to do.
            }
        }
    }

    /// Geometry-based fallback click handler for when `handle_click` returns
    /// `Unhandled` (no overlay to dismiss, no coord widget hit).
    fn on_left_up_geometry(
        &mut self,
        _x: f64,
        _y: f64,
        _event_loop: &ActiveEventLoop,
    ) {
        // Guard: if any overlay is open (modal/popup) and coord didn't hit a
        // widget in it, the click was on the backdrop — but dismiss_topmost_at
        // already handled that in handle_click. If we're here, the overlay is
        // closed or the click missed everything. Proceed with geometry.
        if self.modal_open { return; }
        if self.popup_kind.is_some() { return; }

        // Chrome tab/buttons handled via LayoutManager dispatcher
        // (DispatchEvent::ChromeTabClicked / ChromeWindowControl) — no
        // geometry hit-test needed here.
        // L2 modal widget clicks are now handled by the coordinator dispatch
        // chain (all L2 widgets are registered via register_context_manager_*).
    }

    fn on_right_up(&mut self, x: f64, y: f64) {
        eprintln!("[RIGHT_UP] pos=({:.1},{:.1})", x, y);
        let (w, h) = { let s = &self.surface; (s.config.width as f64, s.config.height as f64) };
        self.layout.context_menu_mut(&self.ctx_menu_h.clone()).open_smart(x, y, w, h, 170.0, 156.0, None);
        for hh in &[self.dd_file_h.clone(), self.dd_view_h.clone(), self.dd_help_h.clone()] {
            self.layout.dropdown_mut(hh).close();
        }
    }

    fn on_mouse_down(&mut self, x: f64, y: f64) {
        self.last_mouse = (x, y);
        self.mouse_down = true;
        self.drag_origin = Some((x, y));

        // Fix 3: modal header drag — start drag when clicking in modal header zone.
        if self.modal_open {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                if modal_input::modal_header_hit(modal_rect, x, y, 44.0, 34.0) {
                    let modal_pos = self.layout.modal(&self.modal_h).position;
                    let origin = if modal_pos != (0.0, 0.0) {
                        modal_pos
                    } else {
                        (modal_rect.x, modal_rect.y)
                    };
                    self.layout.modal_mut(&self.modal_h.clone()).start_drag((x, y), origin);
                    self.drag_target = Some(DragTarget::ModalDrag);
                    return;
                }
            }
        }

        // Sidebar resize handle is now resolved via the LayoutManager
        // dispatcher (z-aware). See bridge mouse-down: it routes drag_id
        // through dispatch_widget → ResizeHandleDragStarted.

        // L1 button pressed state is read via coordinator widget_state() on each render frame.

        // Watchlist mouse-down is now routed by the BlackboxPanel composite
        // through view.handle_event(PointerDown). No manual forwarding here.

        // L2 demo: forward PointerDown to blackbox handler.
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            use uzor::ui::widgets::composite::blackbox_panel::types::{BlackboxEvent, BlackboxHandler};
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let modal_pos = self.layout.modal(&self.modal_h).position;
                let frame_x = if modal_pos != (0.0, 0.0) { modal_pos.0 } else { modal_rect.x };
                let frame_y = if modal_pos != (0.0, 0.0) { modal_pos.1 } else { modal_rect.y };
                // body_rect starts 44px from top (header), 52px from bottom (footer)
                let body_y = frame_y + 44.0;
                let body_h = modal_rect.height - 44.0 - 52.0;
                let body_rect = uzor::types::Rect::new(frame_x, body_y, modal_rect.width, body_h);
                if body_rect.contains(x, y) {
                    let lx = x - body_rect.x;
                    let ly = y - body_rect.y;
                    self.l2_demo.handle_event(BlackboxEvent::PointerDown {
                        local_x: lx, local_y: ly,
                        button: uzor::input::MouseButton::Left,
                    });
                }
            }
        }
    }

    fn on_mouse_move(&mut self, x: f64, y: f64) {
        self.last_mouse = (x, y);

        // Fix #2/#13: update chrome hover state so buttons visually highlight.
        // Tab-slot hover is resolved via the coordinator (sync_hover_from_coordinator);
        // the geometry-based chrome_hit_test is still needed for the overall
        // ChromeHit (min/max/close button zones, drag zone, resize edges).
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
                    &self.layout.chrome_state(),
                    &chrome_view_mv,
                    &ChromeSettings::default(),
                    &ChromeRenderKind::Default,
                    chrome_rect,
                    (x, y),
                );
                self.layout.chrome_state_mut().hovered = hit;
            }
            // Sync per-tab hover flags from coordinator (replaces manual Tab(j) loop).
            // Use take/put-back to avoid simultaneous mutable borrows of `self.layout`.
            {
                let mut cs = std::mem::take(self.layout.chrome_state_mut());
                cs.sync_hover_from_coordinator(&self.layout.ctx().input, "chrome-widget");
                *self.layout.chrome_state_mut() = cs;
            }
        }


        // Watchlist mouse-move is now routed by the BlackboxPanel composite
        // through view.handle_event(PointerMove). The composite suppresses
        // forwarding when an overlay is on top.

        // Hover-state forwarding is now done inside each composite's
        // register_layout_manager_* helper via *State::sync_hover_from().
        // No app-side bookkeeping needed.

        // Fix 3: modal drag — update position while dragging modal header
        if matches!(self.drag_target, Some(DragTarget::ModalDrag)) {
            let (sw, sh) = { let s = &self.surface; (s.config.width as f64, s.config.height as f64) };
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let h = self.modal_h.clone();
                let ms = self.layout.modal_mut(&h);
                handle_modal_drag(ms, (x, y), (sw, sh), (modal_rect.width, modal_rect.height));
            }
            return;
        }

        // L2 demo: forward PointerMove to blackbox handler (drag tracking inside).
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            use uzor::ui::widgets::composite::blackbox_panel::types::{BlackboxEvent, BlackboxHandler};
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let modal_pos = self.layout.modal(&self.modal_h).position;
                let frame_x = if modal_pos != (0.0, 0.0) { modal_pos.0 } else { modal_rect.x };
                let frame_y = if modal_pos != (0.0, 0.0) { modal_pos.1 } else { modal_rect.y };
                let body_y = frame_y + 44.0;
                let body_h = modal_rect.height - 44.0 - 52.0;
                let lx = x - frame_x;
                let ly = y - body_y;
                if body_h > 0.0 {
                    self.l2_demo.handle_event(BlackboxEvent::PointerMove { local_x: lx, local_y: ly });
                }
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
            let _dx = x - ox;
            let _dy = y - oy;
            match target {
                DragTarget::ModalDrag => {} // handled above
                DragTarget::SeparatorDrag { .. } => {} // handled above
                // L2 drag math lives inside L2DemoBlackbox::handle_event(PointerMove).
                DragTarget::SidebarScrollbar { track_rect, content_h, viewport_h } => {
                    // Atomic scrollbar API converts current cursor Y → scroll offset
                    // using the track height + content/viewport ratios.
                    let h = self.sidebar_h.clone();
                    let scroll = self.layout.sidebar_mut(&h).get_or_insert_scroll("default");
                    scroll.handle_drag(y, track_rect.height, *content_h, *viewport_h);
                }
                DragTarget::SidebarResize { which } => {
                    // Resize math lives on SidebarState — just forward the cursor.
                    let (handle, is_h) = match *which {
                        "main"   => (self.sidebar_h.clone(),             true),
                        "right"  => (self.demo_sidebar_right_h.clone(),  true),
                        "top"    => (self.demo_sidebar_top_h.clone(),    false),
                        "bottom" => (self.demo_sidebar_bottom_h.clone(), false),
                        _        => return,
                    };
                    self.layout.sidebar_mut(&handle).update_resize((x, y), is_h);
                }
                DragTarget::ToolbarResize { which } => {
                    // Resize math lives on ToolbarState — just forward the cursor.
                    match *which {
                        "top" => {
                            let h = self.top_toolbar_h.clone();
                            let st = self.layout.toolbar_mut(&h);
                            st.update_resize((x, y), false);
                            self.top_toolbar_height_override = st.resized_thickness;
                        }
                        "demo-left2" => {
                            let h = self.demo_toolbar_left2_h.clone();
                            let st = self.layout.toolbar_mut(&h);
                            st.update_resize((x, y), true);
                            self.demo_toolbar_left2_w_override = st.resized_thickness;
                        }
                        "demo-right" => {
                            let h = self.demo_toolbar_right_h.clone();
                            let st = self.layout.toolbar_mut(&h);
                            st.update_resize((x, y), true);
                            self.demo_toolbar_right_w_override = st.resized_thickness;
                        }
                        "demo-bottom" => {
                            let h = self.demo_toolbar_bottom_h.clone();
                            let st = self.layout.toolbar_mut(&h);
                            st.update_resize((x, y), false);
                            self.demo_toolbar_bottom_h_override = st.resized_thickness;
                        }
                        _ => {}
                    }
                }
                DragTarget::ModalBodyScroll => {
                    let h = self.modal_h.clone();
                    self.layout.modal_mut(&h).update_body_scroll_drag(y);
                }
                DragTarget::PopupBodyScroll => {
                    let h = self.demo_popup_h.clone();
                    self.layout.popup_mut(&h).update_body_scroll_drag(y);
                }
                DragTarget::OverlayResize { which } => {
                    match *which {
                        "modal" => {
                            let h = self.modal_h.clone();
                            let ms = self.layout.modal_mut(&h);
                            ms.update_resize((x, y));
                            if let Some(r) = ms.resized_rect {
                                self.modal_size_override = (r.width, r.height);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn on_mouse_up(&mut self) {
        self.mouse_down = false;
        self.drag_origin = None;
        // End any composite resize drags.
        { let h = self.top_toolbar_h.clone();  let st = self.layout.toolbar_mut(&h); st.end_resize(); }
        { let h = self.modal_h.clone();        let st = self.layout.modal_mut(&h);   st.end_resize(); }
        { let h = self.demo_popup_h.clone();   let st = self.layout.popup_mut(&h);   st.end_resize(); }
        // End body scroll drags.
        { let h = self.modal_h.clone();      let st = self.layout.modal_mut(&h); st.end_body_scroll_drag(); }
        { let h = self.demo_popup_h.clone(); let st = self.layout.popup_mut(&h); st.end_body_scroll_drag(); }
        // Fix 3: end modal drag
        { let h = self.modal_h.clone(); let st = self.layout.modal_mut(&h); st.end_drag(); }
        // End any active sidebar resize drags.
        for h in &[
            self.sidebar_h.clone(), self.demo_sidebar_right_h.clone(),
            self.demo_sidebar_top_h.clone(), self.demo_sidebar_bottom_h.clone(),
        ] {
            self.layout.sidebar_mut(h).end_resize();
        }
        // End any active scrollbar thumb drag.
        {
            let h = self.sidebar_h.clone();
            let st = self.layout.sidebar_mut(&h);
            if let Some(scroll) = st.scroll_per_panel.get_mut("default") {
                scroll.end_drag();
            }
        }
        self.drag_target = None;
        // L2 demo drag end.
        {
            use uzor::ui::widgets::composite::blackbox_panel::types::{BlackboxEvent, BlackboxHandler};
            self.l2_demo.handle_event(BlackboxEvent::PointerUp {
                local_x: 0.0, local_y: 0.0, button: uzor::input::MouseButton::Left,
            });
        }
        // Watchlist drag end is routed by the composite via PointerUp.
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

        // Seed chrome state into the layout.
        {
            let cs = layout.chrome_state_mut();
            cs.sync_tabs(&["tab-0", "tab-1", "tab-2"]);
            cs.active_tab_id = Some("tab-0".into());
        }
        // ── Phase A+C: register all composite handles ────────────────────────
        let modal_h               = layout.add_modal("modal-widget");
        let dd_file_h             = layout.add_dropdown("dd-file-widget");
        let dd_view_h             = layout.add_dropdown("dd-view-widget");
        let dd_help_h             = layout.add_dropdown("dd-help-widget");
        let dd_sidebar_h          = layout.add_dropdown("dd-sidebar-widget");
        let dd_toolbar_h          = layout.add_dropdown("dd-toolbar-widget");
        let dd_popup_h            = layout.add_dropdown("dd-popup-widget");
        let ctx_menu_h            = layout.add_context_menu("ctx-menu-widget");
        let top_toolbar_h         = layout.add_toolbar("top-toolbar-widget");
        let left_vtoolbar_h       = layout.add_toolbar("left-vtoolbar-widget");
        let demo_toolbar_left2_h  = layout.add_toolbar("demo-toolbar-left2-widget");
        let demo_toolbar_right_h  = layout.add_toolbar("demo-toolbar-right-widget");
        let demo_toolbar_bottom_h = layout.add_toolbar("demo-toolbar-bottom-widget");
        let sidebar_h             = layout.add_sidebar("sidebar-widget");
        let demo_sidebar_right_h  = layout.add_sidebar("demo-sidebar-right-widget");
        let demo_sidebar_top_h    = layout.add_sidebar("demo-sidebar-top-widget");
        let demo_sidebar_bottom_h = layout.add_sidebar("demo-sidebar-bottom-widget");
        let demo_popup_h          = layout.add_popup("demo-popup-widget");

        // Seed initial sidebar width.
        {
            let (sidebar_w_init, _) = measure_sidebar(&SidebarSettings::default(), &SidebarRenderKind::Left);
            layout.sidebar_mut(&sidebar_h).width = sidebar_w_init;
        }

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
            top_toolbar_height_override: 0.0,
            demo_toolbar_left2_w_override:  0.0,
            demo_toolbar_right_w_override:  0.0,
            demo_toolbar_bottom_h_override: 0.0,
            modal_size_override: (0.0, 0.0),
            // demo sidebar states are now in layout.sidebars
            active_view: 0,
            sidebar_open: true,
            sidebar_kind: 0, // Left
            popup_kind: None,
            _toolbar_kind: 0, // Horizontal
            demo_toolbar_left2: false,
            demo_toolbar_right: false,
            demo_toolbar_bottom: false,
            demo_sidebar_right: false,
            demo_sidebar_top: false,
            demo_sidebar_bottom: false,
            demo_overlay_mode: false,
            left_toolbar_visible: true,
            modal_open: false,
            modal_kind: ModalKind::L2,
            popup_item: None,
            l2_demo: l2_demo_blackbox::L2DemoBlackbox::default(),
            last_mouse: (0.0, 0.0),
            mouse_down: false,
            drag_origin: None,
            drag_target: None,
            tab_trees,
            spawn_kind: PanelKind::Notes,
            spawn_split: SpawnSplit::SplitRight,
            exit_requested: false,
            watchlist: watchlist_blackbox::WatchlistState::default(),
            modal_h,
            dd_file_h,
            dd_view_h,
            dd_help_h,
            dd_sidebar_h,
            dd_toolbar_h,
            dd_popup_h,
            ctx_menu_h,
            top_toolbar_h,
            left_vtoolbar_h,
            demo_toolbar_left2_h,
            demo_toolbar_right_h,
            demo_toolbar_bottom_h,
            sidebar_h,
            demo_sidebar_right_h,
            demo_sidebar_top_h,
            demo_sidebar_bottom_h,
            demo_popup_h,
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
            // Close-app is app-specific (sets exit_requested); others delegated to helper.
            if let Some(chrome_rect) = app.layout.rect_for_chrome() {
                let hit = chrome_hit_test(&app.layout.chrome_state(), &chrome_view_tmp, &ChromeSettings::default(), &ChromeRenderKind::Default, chrome_rect, (mx, my));
                if matches!(handle_chrome_action(hit), ChromeAction::CloseApp) {
                    app.exit_requested = true;
                    app.window.request_redraw();
                    return;
                }
            }
            use uzor_desktop::chrome::handle_chrome_window_event;
            if handle_chrome_window_event(&app.layout, &app.layout.chrome_state(), &chrome_view_tmp, &ChromeSettings::default(), &ChromeRenderKind::Default, &app.window, mx, my) {
                return;
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
                out.left_up.as_ref().map(|((x, y), id)| (x, y, id.as_ref().map(|i| i.as_str().to_owned()))),
                out.right_up,
                out.wheel,
            );
        }

        if out.cursor_moved.is_some() || out.text_changed || out.focus_cleared {
            app.window.request_redraw();
        }

        // Left mouse down
        if let Some(((x, y), drag_id)) = out.left_down {
            // Blackbox sync dispatch — if the hovered widget is a BlackboxPanel,
            // route PointerDown directly to the handler and skip the rest.
            use uzor::ui::widgets::composite::blackbox_panel::types::BlackboxEvent;
            use uzor::ui::widgets::composite::blackbox_panel::input::route_blackbox_pointer_down;
            // Resolve the watchlist panel rect before borrowing layout mutably.
            let watchlist_info: Option<(uzor::docking::panels::LeafId, Rect)> =
                app.layout.panels().panel_rects().iter()
                    .find(|(&id, _)| {
                        app.layout.panels().tree().leaf(id)
                            .and_then(|l| l.panels.first())
                            .map(|p| p.kind == PanelKind::Watchlist)
                            .unwrap_or(false)
                    })
                    .map(|(&id, &r)| (id, Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64)));
            let blackbox_consumed = route_blackbox_pointer_down(
                &mut app.layout, x, y,
                BlackboxEvent::PointerDown { local_x: 0.0, local_y: 0.0, button: uzor::input::MouseButton::Left },
                |widget_id, sx, sy, ev| {
                    if let Some((leaf_id, rect)) = watchlist_info {
                        if format!("dock-leaf-{}", leaf_id.0) == widget_id.as_str() {
                            use uzor::ui::widgets::composite::blackbox_panel::input::dispatch_to_handler;
                            dispatch_to_handler(&mut app.watchlist, rect, sx, sy, ev);
                            return true;
                        }
                    }
                    false
                },
            );
            if blackbox_consumed { app.window.request_redraw(); }
            if blackbox_consumed {
                // skip the rest of the down pipeline
            } else {
            // If the bridge resolved a DRAG-sense widget under the cursor,
            // dispatch through LayoutManager so registered drag patterns
            // (scrollbar thumb, resize handle, etc.) fire semantic events.
            // Use layout.consume_event + drag_outcome_* helpers to collapse
            // the per-composite post-consume drag detection into one place.
            let mut handled = false;
            if let Some(ref id) = drag_id {
                use uzor::layout::DragOutcome;

                let viewport = { let cfg = &app.surface.config; (cfg.width as f64, cfg.height as f64) };
                let cursor   = (x, y);
                let ev       = app.layout.dispatch_widget(id);

                let est_panels = app.layout.panels().tree().leaves().len() as f64;
                let sidebar_content_h = 480.0 + est_panels * 30.0;

                // Pre-resolve sidebar rects for drag_outcome_sidebar calls below.
                let r_sb_main    = app.layout.rect_for_edge_slot("sidebar");
                let r_sb_right   = app.layout.rect_for_edge_slot("demo-sidebar-right");
                let r_sb_top     = app.layout.rect_for_edge_slot("demo-sidebar-top");
                let r_sb_bottom  = app.layout.rect_for_edge_slot("demo-sidebar-bottom");

                let remaining = app.layout.consume_event(ev, cursor, viewport);

                if remaining.is_none() {
                    handled = true;
                    // Determine what drag was started using drag_outcome_* helpers.
                    let drag_outcome = {
                        let ms  = app.layout.modal(&app.modal_h);
                        let ps  = app.layout.popup(&app.demo_popup_h);
                        let tts = app.layout.toolbar(&app.top_toolbar_h);
                        let tl2 = app.layout.toolbar(&app.demo_toolbar_left2_h);
                        let tr  = app.layout.toolbar(&app.demo_toolbar_right_h);
                        let tb  = app.layout.toolbar(&app.demo_toolbar_bottom_h);
                        let sb  = app.layout.sidebar(&app.sidebar_h);
                        let sbr = app.layout.sidebar(&app.demo_sidebar_right_h);
                        let sbt = app.layout.sidebar(&app.demo_sidebar_top_h);
                        let sbb = app.layout.sidebar(&app.demo_sidebar_bottom_h);
                        modal_input::drag_outcome_modal(ms)
                        .or_else(|| popup_input::drag_outcome_popup(ps))
                        .or_else(|| toolbar_input::drag_outcome_toolbar(tts, "top"))
                        .or_else(|| toolbar_input::drag_outcome_toolbar(tl2, "demo-left2"))
                        .or_else(|| toolbar_input::drag_outcome_toolbar(tr, "demo-right"))
                        .or_else(|| toolbar_input::drag_outcome_toolbar(tb, "demo-bottom"))
                        .or_else(|| sidebar_input::drag_outcome_sidebar(sb,  "main",   r_sb_main.unwrap_or_default(),   sidebar_content_h))
                        .or_else(|| sidebar_input::drag_outcome_sidebar(sbr, "right",  r_sb_right.unwrap_or_default(),  sidebar_content_h))
                        .or_else(|| sidebar_input::drag_outcome_sidebar(sbt, "top",    r_sb_top.unwrap_or_default(),    sidebar_content_h))
                        .or_else(|| sidebar_input::drag_outcome_sidebar(sbb, "bottom", r_sb_bottom.unwrap_or_default(), sidebar_content_h))
                    };

                    app.drag_target = drag_outcome.map(|o| match o {
                        DragOutcome::ModalBodyScroll      => DragTarget::ModalBodyScroll,
                        DragOutcome::ModalResize          => DragTarget::OverlayResize { which: "modal" },
                        DragOutcome::PopupBodyScroll      => DragTarget::PopupBodyScroll,
                        DragOutcome::PopupResize          => DragTarget::OverlayResize { which: "popup" },
                        DragOutcome::ToolbarResize{which} => DragTarget::ToolbarResize { which },
                        DragOutcome::SidebarResize{which} => DragTarget::SidebarResize { which },
                        DragOutcome::SidebarScrollbar { track_rect, content_h, viewport_h }
                            => DragTarget::SidebarScrollbar { track_rect, content_h, viewport_h },
                    });
                    app.drag_origin = Some((x, y));
                    app.mouse_down = true;
                }
            }
            // ── Dock separator drag — fully via LayoutManager dispatcher ──────
            if !handled {
                if let Some(ref id) = drag_id {
                    if let uzor::layout::DispatchEvent::DockSeparatorDragStarted { sep_idx } =
                        app.layout.dispatch_widget(id)
                    {
                        app.drag_target = Some(DragTarget::SeparatorDrag {
                            sep_idx, start_x: x, start_y: y,
                        });
                        app.drag_origin = Some((x, y));
                        app.mouse_down = true;
                        handled = true;
                    }
                }
            }
            if !handled {
                app.on_mouse_down(x, y);
            }
            app.window.request_redraw();
            } // close blackbox_consumed-else
        }

        // Left mouse up
        if let Some(((x, y), clicked_id)) = out.left_up {
            // Blackbox PointerUp sync dispatch (mirror of left_down).
            let blackbox_consumed = {
                use uzor::ui::widgets::composite::blackbox_panel::input::dispatch_to_handler;
                use uzor::ui::widgets::composite::blackbox_panel::types::BlackboxEvent;
                use uzor::input::WidgetKind as WK;
                let coord = &app.layout.ctx_mut().input;
                let top = coord.hovered_widget().cloned();
                let mut consumed = false;
                if let Some(top_id) = top {
                    if coord.widget_kind(&top_id) == Some(WK::BlackboxPanel) {
                        let watchlist_rect = app.layout.panels().panel_rects().iter()
                            .find(|(&id, _)| {
                                app.layout.panels().tree().leaf(id)
                                    .and_then(|l| l.panels.first())
                                    .map(|p| p.kind == PanelKind::Watchlist)
                                    .unwrap_or(false)
                                    && format!("dock-leaf-{}", id.0) == top_id.as_str()
                            })
                            .map(|(_, &r)| Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64));
                        if let Some(rect) = watchlist_rect {
                            let _ = dispatch_to_handler(
                                &mut app.watchlist, rect, x, y,
                                BlackboxEvent::PointerUp {
                                    local_x: 0.0, local_y: 0.0,
                                    button: uzor::input::MouseButton::Left,
                                },
                            );
                            app.window.request_redraw();
                            consumed = true;
                        }
                    }
                }
                // Always end any drag separator on mouse-up regardless of cursor pos
                // (user may release outside panel after a drag started inside).
                {
                    use uzor::ui::widgets::composite::blackbox_panel::types::BlackboxHandler;
                    if app.watchlist.needs_pointer_up() {
                        let _ = app.watchlist.handle_event(BlackboxEvent::PointerUp {
                            local_x: 0.0, local_y: 0.0, button: uzor::input::MouseButton::Left,
                        });
                        app.window.request_redraw();
                        consumed = true;
                    }
                }
                consumed
            };
            let _ = blackbox_consumed;
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
            app.layout.on_pointer_move(x, y);
            app.on_mouse_move(x, y);
            app.window.request_redraw();
        }

        // Main sidebar mousewheel scroll — forward via SidebarState::handle_wheel.
        if let Some(((cx, cy), (_, dy))) = out.wheel {
            if !app.modal_open && app.sidebar_open {
                if let Some(sidebar_rect) = app.layout.rect_for_edge_slot("sidebar") {
                    if sidebar_rect.contains(cx, cy) {
                        let est_panels = app.layout.panels().tree().leaves().len() as f64;
                        let content_h = 480.0 + est_panels * 30.0;
                        app.layout.sidebar_mut(&app.sidebar_h.clone()).handle_wheel(sidebar_rect, dy, content_h);
                        app.window.request_redraw();
                    }
                }
            }
        }

        // Watchlist blackbox scroll — routed via route_blackbox_wheel.
        // Overlay-aware: hovered_widget won't be a BlackboxPanel when a modal/dropdown
        // is on top, so the dispatch closure is never called in those cases.
        if let Some(((_, _), (_, dy))) = out.wheel {
            use uzor::ui::widgets::composite::blackbox_panel::input::{dispatch_to_handler, route_blackbox_wheel};
            use uzor::ui::widgets::composite::blackbox_panel::types::BlackboxEvent;
            // Snapshot watchlist rect before borrowing layout for routing.
            let watchlist_info: Option<(uzor::docking::panels::LeafId, Rect)> =
                app.layout.panels().panel_rects().iter()
                    .find(|(&id, _)| {
                        app.layout.panels().tree().leaf(id)
                            .and_then(|l| l.panels.first())
                            .map(|p| p.kind == PanelKind::Watchlist)
                            .unwrap_or(false)
                    })
                    .map(|(&id, &r)| (id, Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64)));
            let consumed = route_blackbox_wheel(&mut app.layout, 0.0, dy, |widget_id, _dx, _dy| {
                if let Some((leaf_id, rect)) = watchlist_info {
                    if widget_id.as_str() == format!("dock-leaf-{}", leaf_id.0) {
                        let _ = dispatch_to_handler(
                            &mut app.watchlist, rect, 0.0, 0.0,
                            BlackboxEvent::Wheel { delta_x: 0.0, delta_y: _dy },
                        );
                        return true;
                    }
                }
                false
            });
            if consumed { app.window.request_redraw(); }
        }

        // L2 demo wheel — forwarded to blackbox handler.
        if let Some(((cx, cy), (_, dy))) = out.wheel {
            if app.modal_open && app.modal_kind == ModalKind::L2 {
                use uzor::ui::widgets::composite::blackbox_panel::types::{BlackboxEvent, BlackboxHandler};
                if let Some(modal_rect) = app.layout.rect_for_overlay("modal-overlay") {
                    let modal_pos = app.layout.modal(&app.modal_h).position;
                    let frame_x = if modal_pos != (0.0, 0.0) { modal_pos.0 } else { modal_rect.x };
                    let frame_y = if modal_pos != (0.0, 0.0) { modal_pos.1 } else { modal_rect.y };
                    let body_y = frame_y + 44.0;
                    let body_h = modal_rect.height - 44.0 - 52.0;
                    let body_rect = uzor::types::Rect::new(frame_x, body_y, modal_rect.width, body_h);
                    if body_rect.contains(cx, cy) {
                        app.l2_demo.handle_event(BlackboxEvent::Wheel { delta_x: 0.0, delta_y: dy });
                        app.window.request_redraw();
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
                    let ctx_open = app.layout.context_menu(&app.ctx_menu_h).is_open;
                    if app.modal_open {
                        app.modal_open = false;
                    } else if ctx_open {
                        app.layout.context_menu_mut(&app.ctx_menu_h.clone()).close();
                    } else {
                        for hh in &[app.dd_file_h.clone(), app.dd_view_h.clone(), app.dd_help_h.clone()] {
                            app.layout.dropdown_mut(hh).close();
                        }
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
    use uzor::ui::widgets::composite::blackbox_panel::types::{
        BlackboxEvent, BlackboxEventResult, BlackboxHandler,
    };

    pub struct WatchlistState {
        pub col_widths:  [f64; 4],
        pub scroll_off:  f64,
        pub hovered_row: Option<usize>,
        pub hovered_sep: Option<usize>,
        /// (sep_idx, start_frac, widths_at_drag_start) — start_frac is cursor x as fraction of rect.width
        pub drag_sep: Option<(usize, f64, [f64; 4])>,
        /// Panel body size (width, height) — set by host each frame before events.
        pub panel_size: (f64, f64),
    }

    impl Default for WatchlistState {
        fn default() -> Self {
            Self {
                col_widths:  [0.25, 0.30, 0.20, 0.25],
                scroll_off:  0.0,
                hovered_row: None,
                hovered_sep: None,
                drag_sep:    None,
                panel_size:  (0.0, 0.0),
            }
        }
    }

    impl WatchlistState {
        pub fn set_panel_size(&mut self, size: (f64, f64)) {
            self.panel_size = size;
        }
    }

    impl BlackboxHandler for WatchlistState {
        fn render(&self, ctx: &mut dyn RenderContext, body_rect: Rect) {
            render(self, ctx, body_rect);
        }

        fn handle_event(&mut self, event: BlackboxEvent) -> BlackboxEventResult {
            let ps = self.panel_size;
            match event {
                BlackboxEvent::PointerMove { local_x, local_y } => {
                    on_pointer_move(self, ps, local_x, local_y);
                    BlackboxEventResult::Redraw
                }
                BlackboxEvent::PointerDown { local_x, local_y, .. } => {
                    on_pointer_down(self, ps, local_x, local_y);
                    BlackboxEventResult::Consumed
                }
                BlackboxEvent::PointerUp { .. } => {
                    on_pointer_up(self);
                    BlackboxEventResult::Consumed
                }
                BlackboxEvent::Wheel { delta_y, .. } => {
                    on_wheel(self, delta_y);
                    BlackboxEventResult::Consumed
                }
                _ => BlackboxEventResult::NotConsumed,
            }
        }

        fn needs_pointer_up(&self) -> bool {
            self.drag_sep.is_some()
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

    pub fn hit_test_local(state: &WatchlistState, panel_size: (f64, f64), lx: f64, ly: f64) -> Hit {
        if lx < 0.0 || ly < 0.0 || lx > panel_size.0 || ly > panel_size.1 {
            return Hit::None;
        }
        const HEADER_H: f64 = 28.0;
        const ROW_H:    f64 = 24.0;
        const SEP_HALF: f64 = 4.0;

        let mut cx = 0.0_f64;
        let mut boundaries = [0.0_f64; 3];
        for i in 0..3 {
            cx += panel_size.0 * state.col_widths[i];
            boundaries[i] = cx;
        }
        for (i, &bx) in boundaries.iter().enumerate() {
            if (lx - bx).abs() <= SEP_HALF {
                return Hit::Separator(i);
            }
        }
        if ly >= HEADER_H {
            let row_f = (ly - HEADER_H + state.scroll_off) / ROW_H;
            if row_f >= 0.0 {
                return Hit::Row(row_f as usize);
            }
        }
        Hit::None
    }

    // ── Input handlers (panel-local coords) ───────────────────────────────────

    pub fn on_pointer_move(state: &mut WatchlistState, panel_size: (f64, f64), lx: f64, ly: f64) {
        // Normalize col_widths so they always sum to 1.0 (guards against drift).
        {
            let sum: f64 = state.col_widths.iter().sum();
            if (sum - 1.0).abs() > 1e-6 && sum > 0.0 {
                for w in &mut state.col_widths {
                    *w /= sum;
                }
            }
        }
        if let Some((idx, start_frac, start_widths)) = state.drag_sep {
            if panel_size.0 > 0.0 {
                let cur_frac = lx / panel_size.0;
                let delta_frac = cur_frac - start_frac;
                let new_left  = (start_widths[idx]     + delta_frac).clamp(0.05, 0.90);
                let new_right = (start_widths[idx + 1] - delta_frac).clamp(0.05, 0.90);
                let total_before = start_widths[idx] + start_widths[idx + 1];
                let total_after  = new_left + new_right;
                if (total_after - total_before).abs() < 1e-9 {
                    state.col_widths[idx]     = new_left;
                    state.col_widths[idx + 1] = new_right;
                }
            }
            return;
        }
        match hit_test_local(state, panel_size, lx, ly) {
            Hit::Separator(i) => { state.hovered_sep = Some(i); state.hovered_row = Option::None; }
            Hit::Row(r)       => { state.hovered_row = Some(r); state.hovered_sep = Option::None; }
            Hit::None         => { state.hovered_row = Option::None; state.hovered_sep = Option::None; }
        }
    }

    pub fn on_pointer_down(state: &mut WatchlistState, panel_size: (f64, f64), lx: f64, ly: f64) {
        if let Hit::Separator(i) = hit_test_local(state, panel_size, lx, ly) {
            let start_frac = if panel_size.0 > 0.0 { lx / panel_size.0 } else { 0.0 };
            state.drag_sep = Some((i, start_frac, state.col_widths));
        }
    }

    pub fn on_pointer_up(state: &mut WatchlistState) {
        state.drag_sep = Option::None;
    }

    pub fn on_wheel(state: &mut WatchlistState, dy: f64) {
        state.scroll_off = (state.scroll_off - dy * 20.0).max(0.0);
    }

    /// Returns the row index that was clicked (for app-level row routing).
    /// `None` if the click was not on a row (separator / header / outside).
    pub fn click_row(state: &WatchlistState, panel_size: (f64, f64), lx: f64, ly: f64) -> Option<usize> {
        match hit_test_local(state, panel_size, lx, ly) {
            Hit::Row(r) => Some(r),
            _           => None,
        }
    }
}

// =============================================================================
// === BLACKBOX PANEL DEMO === L2 Widget Set
// === Phase D: all L2-inside-L3 state, render, drag, and input moves here.
// === The modal body calls L2DemoBlackbox::render() + routes events to
// === L2DemoBlackbox::handle_event().  AppState no longer sees l2_* fields.
// =============================================================================

mod l2_demo_blackbox {
    use uzor::render::{RenderContext, TextAlign, TextBaseline};
    use uzor::types::{Rect, WidgetState};
    use uzor::ui::widgets::composite::blackbox_panel::types::{
        BlackboxEvent, BlackboxEventResult, BlackboxHandler,
    };

    // ── Layout constants (local to this demo) ─────────────────────────────────
    pub const L2_WIN_W: f64 = 560.0;
    pub const L2_WIN_H: f64 = 440.0;

    const BTN_RECT:   Rect = Rect { x: 28.0, y: 28.0,  width: 130.0, height: 36.0 };
    const CB_RECT:    Rect = Rect { x: 28.0, y: 88.0,  width: 160.0, height: 22.0 };
    const TOG_RECT:   Rect = Rect { x: 28.0, y: 130.0, width: 80.0,  height: 24.0 };
    const SLID_RECT:  Rect = Rect { x: 28.0, y: 200.0, width: 260.0, height: 24.0 };
    const RANGE_RECT: Rect = Rect { x: 28.0, y: 228.0, width: 260.0, height: 24.0 };

    const TAB_STRIP_Y:    f64 = 12.0;
    const TAB_STRIP_H:    f64 = 28.0;
    const CONTENT_START_Y: f64 = 52.0;
    const SB_W:           f64 = 10.0;
    const SB_H:           f64 = 376.0;
    const CONTENT_ROWS:   usize = 20;
    const ROW_H:          f64 = 28.0;
    const CONTENT_H:      f64 = CONTENT_ROWS as f64 * ROW_H;
    const SPLITTER_W:     f64 = 6.0;
    const LEFT_PANEL_X:   f64 = 12.0;

    // ── Drag state ────────────────────────────────────────────────────────────

    #[derive(Clone, Copy)]
    enum L2Drag {
        Slider  { start_x: f64, v0: f64 },
        RangeMin { start_x: f64, v0: f64 },
        RangeMax { start_x: f64, v0: f64 },
        Scroll  { start_y: f64, v0: f64 },
        Splitter { start_x: f64, w0: f64 },
    }

    // ── Hovered widget id (for visual feedback) ───────────────────────────────
    #[derive(Clone, Copy, PartialEq, Eq, Default)]
    enum Hovered {
        #[default]
        None,
        Button,
        Checkbox,
        Toggle,
        Radio(usize),
        Slider,
        Range,
        Swatch(usize),
        Tab(usize),
        SubTab(usize),
        Scrollbar,
        Splitter,
    }

    // ── Main struct ───────────────────────────────────────────────────────────

    pub struct L2DemoBlackbox {
        // Widget state
        connected:      bool,
        checked:        bool,
        toggled:        bool,
        radio_sel:      usize,
        slider_val:     f64,
        range_min:      f64,
        range_max:      f64,
        scroll_off:     f64,
        swatch_sel:     usize,
        active_tab:     usize,
        active_sub_tab: usize,
        right_panel_w:  f64,

        // Input tracking
        hovered:        Hovered,
        drag:           Option<L2Drag>,

        // Panel geometry (set each frame by host before render)
        panel_size:     (f64, f64),
    }

    impl Default for L2DemoBlackbox {
        fn default() -> Self {
            Self {
                connected:      false,
                checked:        true,
                toggled:        true,
                radio_sel:      1,
                slider_val:     40.0,
                range_min:      20.0,
                range_max:      80.0,
                scroll_off:     0.0,
                swatch_sel:     0,
                active_tab:     0,
                active_sub_tab: 0,
                right_panel_w:  330.0,
                hovered:        Hovered::None,
                drag:           None,
                panel_size:     (0.0, 0.0),
            }
        }
    }

    impl L2DemoBlackbox {
        pub fn set_panel_size(&mut self, size: (f64, f64)) {
            self.panel_size = size;
        }
    }

    // ── BlackboxHandler impl ──────────────────────────────────────────────────

    impl BlackboxHandler for L2DemoBlackbox {
        fn render(&self, ctx: &mut dyn RenderContext, body_rect: Rect) {
            render_l2(self, ctx, body_rect);
        }

        fn handle_event(&mut self, event: BlackboxEvent) -> BlackboxEventResult {
            match event {
                BlackboxEvent::PointerMove { local_x, local_y } => {
                    on_move(self, local_x, local_y);
                    BlackboxEventResult::Redraw
                }
                BlackboxEvent::PointerDown { local_x, local_y, .. } => {
                    on_down(self, local_x, local_y);
                    BlackboxEventResult::Consumed
                }
                BlackboxEvent::PointerUp { .. } => {
                    self.drag = None;
                    BlackboxEventResult::Consumed
                }
                BlackboxEvent::Wheel { delta_y, .. } => {
                    if self.active_tab == 0 {
                        self.scroll_off = (self.scroll_off - delta_y * 20.0)
                            .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                    }
                    BlackboxEventResult::Consumed
                }
                _ => BlackboxEventResult::NotConsumed,
            }
        }

        fn needs_pointer_up(&self) -> bool {
            self.drag.is_some()
        }
    }

    // ── Hit-test (local coords: 0,0 = body top-left) ─────────────────────────

    fn right_panel_x(right_panel_w: f64) -> f64 {
        L2_WIN_W - right_panel_w
    }

    fn hit_test(state: &L2DemoBlackbox, lx: f64, ly: f64) -> Hovered {
        let rpx = right_panel_x(state.right_panel_w);
        // Splitter zone
        let splitter_cx = rpx - SPLITTER_W / 2.0;
        if (lx - splitter_cx).abs() <= SPLITTER_W / 2.0 + 2.0 {
            return Hovered::Splitter;
        }
        // Left panel widgets
        if lx < rpx - SPLITTER_W / 2.0 {
            if BTN_RECT.contains(lx, ly)  { return Hovered::Button; }
            if CB_RECT.contains(lx, ly)   { return Hovered::Checkbox; }
            if TOG_RECT.contains(lx, ly)  { return Hovered::Toggle; }
            for (i, cx_off) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                let r = Rect::new(*cx_off, 175.0, 28.0, 28.0);
                if r.contains(lx, ly) { return Hovered::Radio(i); }
            }
            if SLID_RECT.contains(lx, ly)  { return Hovered::Slider; }
            if RANGE_RECT.contains(lx, ly) { return Hovered::Range; }
            let swatch_colors: [usize; 4] = [0, 1, 2, 3];
            for i in swatch_colors {
                let r = Rect::new(28.0 + i as f64 * 34.0, 344.0, 26.0, 26.0);
                if r.contains(lx, ly) { return Hovered::Swatch(i); }
            }
        }
        // Right panel
        if lx >= rpx {
            let tab_w = ((state.right_panel_w - 16.0) / 3.0).floor();
            for i in 0..3 {
                let tab_x = rpx + 8.0 + i as f64 * (tab_w + 4.0);
                let tab_rect = Rect::new(tab_x, TAB_STRIP_Y, tab_w, TAB_STRIP_H);
                if tab_rect.contains(lx, ly) { return Hovered::Tab(i); }
            }
            // Scrollbar
            let sb_x = L2_WIN_W - SB_W - 8.0;
            if lx >= sb_x && ly >= 52.0 && ly <= 52.0 + SB_H {
                return Hovered::Scrollbar;
            }
            // Sub-tabs
            if state.active_tab == 2 {
                for i in 0..3 {
                    let r = Rect::new(rpx + 8.0, CONTENT_START_Y + 8.0 + i as f64 * 36.0, 90.0, 30.0);
                    if r.contains(lx, ly) { return Hovered::SubTab(i); }
                }
            }
        }
        Hovered::None
    }

    // ── Input handlers ────────────────────────────────────────────────────────

    fn on_move(state: &mut L2DemoBlackbox, lx: f64, ly: f64) {
        // If dragging, update drag math
        if let Some(drag) = state.drag {
            match drag {
                L2Drag::Slider { start_x, v0 } => {
                    let frac = (lx - start_x) / SLID_RECT.width;
                    state.slider_val = (v0 + frac * 100.0).clamp(0.0, 100.0);
                }
                L2Drag::RangeMin { start_x, v0 } => {
                    let frac = (lx - start_x) / RANGE_RECT.width;
                    state.range_min = (v0 + frac * 100.0).clamp(0.0, state.range_max);
                }
                L2Drag::RangeMax { start_x, v0 } => {
                    let frac = (lx - start_x) / RANGE_RECT.width;
                    state.range_max = (v0 + frac * 100.0).clamp(state.range_min, 100.0);
                }
                L2Drag::Scroll { start_y, v0 } => {
                    use uzor::ui::widgets::atomic::scrollbar::input::thumb_height;
                    let sr = SB_H - thumb_height(CONTENT_H, SB_H, SB_H, 30.0);
                    if sr > 0.0 {
                        let dy = lx - start_y; // using x axis for vertical scroll? No: scroll uses y.
                        let _ = dy;
                        let dy_real = lx - start_y; // see below: we stored start_y as start_y
                        let _ = dy_real;
                        // Correction: scroll drag stores start_y from ly
                        let dy2 = ly - start_y;
                        state.scroll_off = (v0 + dy2 / sr * (CONTENT_H - SB_H))
                            .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                    }
                }
                L2Drag::Splitter { start_x, w0 } => {
                    let dx = lx - start_x;
                    state.right_panel_w = (w0 - dx).clamp(200.0, L2_WIN_W - 100.0);
                }
            }
        } else {
            state.hovered = hit_test(state, lx, ly);
        }
    }

    fn on_down(state: &mut L2DemoBlackbox, lx: f64, ly: f64) {
        let hit = hit_test(state, lx, ly);
        match hit {
            Hovered::Button   => { state.connected = !state.connected; }
            Hovered::Checkbox => { state.checked = !state.checked; }
            Hovered::Toggle   => { state.toggled = !state.toggled; }
            Hovered::Radio(i) => { state.radio_sel = i; }
            Hovered::Swatch(i) => { state.swatch_sel = i; }
            Hovered::Tab(i)   => { state.active_tab = i; state.scroll_off = 0.0; }
            Hovered::SubTab(i) => { state.active_sub_tab = i; }
            Hovered::Slider   => {
                state.drag = Some(L2Drag::Slider { start_x: lx, v0: state.slider_val });
            }
            Hovered::Range => {
                let x_min = RANGE_RECT.x + (state.range_min / 100.0) * RANGE_RECT.width;
                let x_max = RANGE_RECT.x + (state.range_max / 100.0) * RANGE_RECT.width;
                if (lx - x_min).abs() <= (lx - x_max).abs() {
                    state.drag = Some(L2Drag::RangeMin { start_x: lx, v0: state.range_min });
                } else {
                    state.drag = Some(L2Drag::RangeMax { start_x: lx, v0: state.range_max });
                }
            }
            Hovered::Scrollbar => {
                state.drag = Some(L2Drag::Scroll { start_y: ly, v0: state.scroll_off });
            }
            Hovered::Splitter => {
                state.drag = Some(L2Drag::Splitter { start_x: lx, w0: state.right_panel_w });
            }
            Hovered::None => {}
        }
    }

    // ── Render (body_rect is in SCREEN coordinates) ───────────────────────────

    fn draw_label(
        render: &mut dyn RenderContext,
        rect:   Rect,
        text:   &str,
        align:  TextAlign,
        color:  &str,
    ) {
        use uzor::ui::widgets::atomic::text::{draw_text, TextSettings};
        use uzor::ui::widgets::atomic::text::types::{TextOverflow, TextView};
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

    fn render_l2(state: &L2DemoBlackbox, render: &mut dyn RenderContext, body_rect: Rect) {
        use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
        use uzor::ui::widgets::atomic::button::render::draw_button;
        use uzor::ui::widgets::atomic::checkbox::render::draw_checkbox;
        use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
        use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};
        use uzor::ui::widgets::atomic::toggle::render::draw_toggle;
        use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
        use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};
        use uzor::ui::widgets::atomic::radio::render::draw_radio;
        use uzor::ui::widgets::atomic::radio::settings::RadioSettings;
        use uzor::ui::widgets::atomic::radio::types::{DotShape, RadioDotView, RadioRenderKind};
        use uzor::ui::widgets::atomic::slider::render::draw_slider;
        use uzor::ui::widgets::atomic::slider::settings::SliderSettings;
        use uzor::ui::widgets::atomic::slider::types::SliderType;
        use uzor::ui::widgets::atomic::slider::render::SliderView;
        use uzor::ui::widgets::atomic::color_swatch::render::draw_color_swatch;
        use uzor::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
        use uzor::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};
        use uzor::ui::widgets::atomic::drag_handle::render::draw_drag_handle;
        use uzor::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
        use uzor::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};
        use uzor::ui::widgets::atomic::tab::render::{draw_tab, TabView};
        use uzor::ui::widgets::atomic::tab::settings::TabSettings;
        use uzor::ui::widgets::atomic::tab::types::TabConfig;
        use uzor::ui::widgets::atomic::scrollbar::render::{draw_scrollbar, ScrollbarView, ScrollbarVisualState};
        use uzor::ui::widgets::atomic::scrollbar::style::StandardScrollbarStyle;
        use uzor::ui::widgets::atomic::scrollbar::theme::DefaultScrollbarTheme;
        use uzor::ui::widgets::atomic::separator::render::draw_separator;
        use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
        use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};
        use uzor::ui::widgets::atomic::separator::render::SeparatorView;
        use uzor::render::draw_svg_icon;

        let rpx = right_panel_x(state.right_panel_w);
        let ox = body_rect.x;
        let oy = body_rect.y;
        let body_h = body_rect.height;
        let left_panel_w = rpx - LEFT_PANEL_X - SPLITTER_W / 2.0;
        let panel_inner_h = body_h - 24.0;

        // Panel backgrounds
        render.set_fill_color("#1e222d");
        render.fill_rounded_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, panel_inner_h, 8.0);
        render.fill_rounded_rect(ox + rpx, oy + 12.0, L2_WIN_W - rpx - 12.0, panel_inner_h, 8.0);

        // ── Left panel ────────────────────────────────────────────────────────
        render.save();
        render.clip_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, panel_inner_h);

        // 1. Button
        let btn_rect = Rect::new(BTN_RECT.x + ox, BTN_RECT.y + oy, BTN_RECT.width, BTN_RECT.height);
        let btn_state = if state.hovered == Hovered::Button { WidgetState::Hovered }
                        else if state.connected { WidgetState::Active }
                        else { WidgetState::Normal };
        draw_button(&mut *render, btn_rect, btn_state, &ButtonView {
            text: Some(if state.connected { "Disconnect" } else { "Connect" }),
            icon: None,
            active: state.connected,
            disabled: false,
            active_border: None,
            hover_chevron: None,
        }, &ButtonSettings::default().with_theme(Box::new(super::VisibleButtonTheme)), |_, _, _, _| {});

        // 2. Checkbox
        let cb_state = if state.hovered == Hovered::Checkbox { WidgetState::Hovered } else { WidgetState::Normal };
        draw_checkbox(&mut *render,
            Rect::new(CB_RECT.x + ox, CB_RECT.y + oy, CB_RECT.width, CB_RECT.height),
            cb_state,
            &CheckboxView { checked: state.checked, label: Some("Setting A") },
            &CheckboxSettings::default().with_theme(Box::new(super::VisibleCheckboxTheme)),
            &CheckboxRenderKind::Standard, "13px sans-serif",
        );

        // 3. Toggle
        let tog_state = if state.hovered == Hovered::Toggle { WidgetState::Hovered } else { WidgetState::Normal };
        draw_toggle(&mut *render,
            Rect::new(TOG_RECT.x + ox, TOG_RECT.y + oy, TOG_RECT.width, TOG_RECT.height),
            tog_state,
            &ToggleView { toggled: state.toggled, label: Some("ON"), disabled: false },
            &ToggleSettings::default(),
            &ToggleRenderKind::Switch,
            |_, _, _, _| {},
        );

        // 4. Radio ×3
        for (i, cx_off) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
            let r_state = if state.hovered == Hovered::Radio(i) { WidgetState::Hovered } else { WidgetState::Normal };
            draw_radio(&mut *render,
                Rect::new(cx_off + ox, 175.0 + oy, 28.0, 28.0),
                r_state,
                &RadioSettings::default(),
                &RadioRenderKind::Dot {
                    shape: DotShape::Circle,
                    cx:   cx_off + 14.0 + ox,
                    cy:   175.0 + 14.0 + oy,
                    view: RadioDotView { selected: state.radio_sel == i },
                },
            );
        }

        // 5. Slider
        let sl_state = if state.hovered == Hovered::Slider { WidgetState::Hovered } else { WidgetState::Normal };
        draw_slider(&mut *render,
            Rect::new(SLID_RECT.x + ox, SLID_RECT.y + oy, SLID_RECT.width, SLID_RECT.height),
            sl_state,
            &SliderView { kind: SliderType::Single { value: state.slider_val, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: None },
            &SliderSettings::default(),
        );

        // 6. Range slider
        let rng_state = if state.hovered == Hovered::Range { WidgetState::Hovered } else { WidgetState::Normal };
        draw_slider(&mut *render,
            Rect::new(RANGE_RECT.x + ox, RANGE_RECT.y + oy, RANGE_RECT.width, RANGE_RECT.height),
            rng_state,
            &SliderView { kind: SliderType::Dual { min_value: state.range_min, max_value: state.range_max, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: None },
            &SliderSettings::default(),
        );

        // 7. Separator line
        draw_separator(&mut *render,
            Rect::new(28.0 + ox, 260.0 + oy, 260.0, 2.0),
            &SeparatorView { kind: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal }, hovered: false, dragging: false },
            &SeparatorSettings::default(),
        );

        // 8. Color swatches ×4
        let swatch_colors: [[u8; 4]; 4] = [
            [41, 98, 255, 255], [16, 185, 129, 255], [245, 158, 11, 255], [239, 83, 80, 255],
        ];
        for (i, color) in swatch_colors.iter().enumerate() {
            let sw_state = if state.hovered == Hovered::Swatch(i) { WidgetState::Hovered } else { WidgetState::Normal };
            draw_color_swatch(&mut *render,
                Rect::new(28.0 + i as f64 * 34.0 + ox, 344.0 + oy, 26.0, 26.0),
                sw_state,
                &ColorSwatchView { color: *color, hovered: false, selected: state.swatch_sel == i, show_transparency: false, border_color_override: None },
                &ColorSwatchSettings::default(),
                &ColorSwatchRenderKind::Simple,
            );
        }

        render.restore();

        // ── Right panel ───────────────────────────────────────────────────────
        render.save();
        render.clip_rect(ox + rpx, oy + 12.0, L2_WIN_W - rpx - 12.0, panel_inner_h);

        // Tab strip
        let tab_labels = ["List", "Empty", "Sub-tabs"];
        let tab_w = ((state.right_panel_w - 16.0) / 3.0).floor();
        for (i, lbl) in tab_labels.iter().enumerate() {
            let tab_x = rpx + 8.0 + i as f64 * (tab_w + 4.0);
            let tab_rect = Rect::new(tab_x + ox, TAB_STRIP_Y + oy, tab_w, TAB_STRIP_H);
            let tab_hovered = state.hovered == Hovered::Tab(i);
            let tab_cfg = TabConfig::new(
                if i == 0 { "l2-tab-0" } else if i == 1 { "l2-tab-1" } else { "l2-tab-2" },
                *lbl,
            );
            let tab_cfg = {
                let mut tc = tab_cfg;
                tc.active = state.active_tab == i;
                tc
            };
            draw_tab(&mut *render, tab_rect, &TabView { tab: &tab_cfg, hovered: tab_hovered, pressed: false, close_btn_hovered: false }, &TabSettings::default());
        }

        // Tab content
        if state.active_tab == 0 {
            // Scrollable list + scrollbar
            let sb_x = L2_WIN_W - SB_W - 8.0;
            let sb_track = Rect::new(sb_x + ox, 52.0 + oy, SB_W, SB_H);
            let sb_vis = if state.hovered == Hovered::Scrollbar {
                ScrollbarVisualState::HandleHovered
            } else if state.drag.is_some() {
                ScrollbarVisualState::Dragging
            } else {
                ScrollbarVisualState::Active
            };
            draw_scrollbar(&mut *render, sb_track, &ScrollbarView {
                content_height:  CONTENT_H,
                viewport_height: SB_H,
                scroll_offset:   state.scroll_off,
                state:           sb_vis,
                drag_pos_y:      None,
                style:           &StandardScrollbarStyle,
                theme:           &DefaultScrollbarTheme,
            });

            let row_labels = [
                "★ Roboto regular", "Sans-serif clean", "→ arrow + ✓ check",
                "Quick brown fox", "✨ ★ ☀ ☂ ❤", "fn main() { ... }",
                "let x: u32 = 42;", "if let Some(v) = opt", "// monospace code",
                "0xCAFE_BABE", "PT Root UI light", "вариативный шрифт",
                "12345 67890", "Кириллица OK", "ƒ unicode glyphs",
                "Bold Roboto bold", "❗ Heads up ❗", "✓ Done · 14 items",
                "🌍 globe · 🌟 star", "═══ end of list ═══",
            ];
            let content_x = rpx + 8.0 + ox;
            let content_w = state.right_panel_w - SB_W - 20.0;
            for row in 0..CONTENT_ROWS {
                let row_y = 52.0 + oy + row as f64 * ROW_H - state.scroll_off;
                if row_y + ROW_H < 52.0 + oy || row_y > 52.0 + oy + SB_H { continue; }
                let color = if row % 2 == 0 { "rgba(255,255,255,0.08)" } else { "rgba(0,0,0,0)" };
                render.set_fill_color(color);
                render.fill_rect(content_x, row_y, content_w, ROW_H - 2.0);
                draw_label(render, Rect::new(content_x + 4.0, row_y, content_w - 8.0, ROW_H - 2.0),
                    row_labels[row], TextAlign::Left, "#c8c8dc");
            }
        }

        if state.active_tab == 2 {
            for (i, lbl) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
                let sub_rect = Rect::new(rpx + 8.0 + ox, CONTENT_START_Y + 8.0 + i as f64 * 36.0 + oy, 90.0, 30.0);
                let sub_hov = state.hovered == Hovered::SubTab(i);
                let sub_id = if i == 0 { "l2-sub-0" } else if i == 1 { "l2-sub-1" } else { "l2-sub-2" };
                let sub_cfg = { let mut tc = TabConfig::new(sub_id, *lbl); tc.active = state.active_sub_tab == i; tc };
                draw_tab(&mut *render, sub_rect, &TabView { tab: &sub_cfg, hovered: sub_hov, pressed: false, close_btn_hovered: false }, &TabSettings::default());
            }
            let sub_content_y = CONTENT_START_Y + 8.0 + 3.0 * 36.0 + 8.0;
            let sub_cx = rpx + state.right_panel_w / 2.0 + ox;
            let icon_size = 64.0_f64;
            let ix = sub_cx - icon_size / 2.0;
            let iy = sub_content_y + oy + 8.0;
            let (sub_svg, sub_color) = match state.active_sub_tab {
                0 => (super::SVG_CIRCLE,   "#2962ff"),
                1 => (super::SVG_TRIANGLE, "#f59e0b"),
                _ => (super::SVG_DIAMOND,  "#ef5350"),
            };
            draw_svg_icon(&mut *render, sub_svg, ix, iy, icon_size, icon_size, sub_color);
        }

        if state.active_tab == 1 {
            let icon_size = 64.0_f64;
            let gap = 16.0_f64;
            let content_cx = rpx + state.right_panel_w / 2.0 + ox;
            let content_cy = CONTENT_START_Y + (420.0 - CONTENT_START_Y - 12.0) / 2.0 + oy;
            let grid_x0 = content_cx - (icon_size * 2.0 + gap) / 2.0;
            let grid_y0 = content_cy - (icon_size * 2.0 + gap) / 2.0;
            for (idx, (svg, color)) in [
                (super::SVG_CIRCLE, "#2962ff"), (super::SVG_SQUARE, "#10b981"),
                (super::SVG_TRIANGLE, "#f59e0b"), (super::SVG_DIAMOND, "#ef5350"),
            ].iter().enumerate() {
                let col = idx % 2;
                let row = idx / 2;
                draw_svg_icon(&mut *render, svg, grid_x0 + col as f64 * (icon_size + gap), grid_y0 + row as f64 * (icon_size + gap), icon_size, icon_size, color);
            }
        }

        render.restore();

        // ── Splitter handle ───────────────────────────────────────────────────
        let dh_rect = Rect::new(rpx - SPLITTER_W / 2.0 + ox, 12.0 + oy, SPLITTER_W, panel_inner_h);
        let dh_state = if state.hovered == Hovered::Splitter { WidgetState::Hovered } else { WidgetState::Normal };
        let _ = dh_state;
        draw_drag_handle(&mut *render, dh_rect, &DragHandleView { rect: dh_rect }, &DragHandleSettings::default(), &DragHandleRenderKind::GripDots);
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
