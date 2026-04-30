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
use uzor::app_context::{layout::types::LayoutNode, ContextManager};
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
use uzor::ui::widgets::composite::chrome::types::{
    ChromeAction, ChromeRenderKind, ChromeTabConfig, ChromeView,
};

use uzor::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu;
use uzor::ui::widgets::composite::context_menu::settings::ContextMenuSettings;
use uzor::ui::widgets::composite::context_menu::state::ContextMenuState;
use uzor::ui::widgets::composite::context_menu::types::{
    ContextMenuItem, ContextMenuRenderKind, ContextMenuView,
};

use uzor::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown;
use uzor::ui::widgets::composite::dropdown::settings::DropdownSettings;
use uzor::ui::widgets::composite::dropdown::state::DropdownState;
use uzor::ui::widgets::composite::dropdown::types::{
    DropdownItem, DropdownItemRight, DropdownRenderKind, DropdownView, DropdownViewKind,
};

use uzor::ui::widgets::composite::modal::input::register_layout_manager_modal;
use uzor::ui::widgets::composite::modal::settings::ModalSettings;
use uzor::ui::widgets::composite::modal::state::ModalState;
use uzor::ui::widgets::composite::modal::types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView,
};

use uzor::ui::widgets::composite::panel::input::register_layout_manager_panel;
use uzor::ui::widgets::composite::panel::settings::PanelSettings;
use uzor::ui::widgets::composite::panel::state::PanelState;
use uzor::ui::widgets::composite::panel::types::{PanelHeader, PanelRenderKind, PanelView};

use uzor::ui::widgets::composite::popup::input::register_layout_manager_popup;
use uzor::ui::widgets::composite::popup::settings::PopupSettings;
use uzor::ui::widgets::composite::popup::state::PopupState;
use uzor::ui::widgets::composite::popup::types::{
    BackdropKind as PopupBackdrop, PopupRenderKind, PopupView, PopupViewKind,
};

use uzor::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use uzor::ui::widgets::composite::sidebar::settings::SidebarSettings;
use uzor::ui::widgets::composite::sidebar::state::SidebarState;
use uzor::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarRenderKind, SidebarView,
};

use uzor::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar;
use uzor::ui::widgets::composite::toolbar::settings::ToolbarSettings;
use uzor::ui::widgets::composite::toolbar::state::ToolbarState;
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
// DockPanel impl
// =============================================================================

#[derive(Clone, Debug)]
struct DemoPanel {
    title: String,
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
    panel_a_state: PanelState,
    panel_b_state: PanelState,
    modal_state: ModalState,
    popup_state: PopupState,
    ctx_menu_state: ContextMenuState,
    dropdown_file_state: DropdownState,
    dropdown_view_state: DropdownState,
    dropdown_help_state: DropdownState,

    // App interaction state
    active_view: usize, // 0=Dashboard,1=Charts,2=Settings
    sidebar_open: bool,
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

    // Dock leaf ids
    leaf_a_id: Option<String>,
    leaf_b_id: Option<String>,

    // Need-exit flag set by close button
    exit_requested: bool,
}

impl AppState {
    fn time_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn time_ms(&self) -> f64 {
        self.start.elapsed().as_millis() as f64
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

        // Left vertical toolbar (always visible)
        self.layout.edges_mut().add(EdgeSlot {
            id: "left-vtoolbar".to_string(),
            side: EdgeSide::Left,
            thickness: LEFT_VTOOLBAR_W as f32,
            visible: true,
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

        // Register L2 text field
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            self.layout.ctx_mut().input.register_text_field(
                "l2-text",
                TI_RECT,
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
            ChromeTabConfig { id: "tab-0", label: "Dashboard", icon: None, color_tag: None, closable: false, active: self.active_view == 0 },
            ChromeTabConfig { id: "tab-1", label: "Charts",    icon: None, color_tag: None, closable: false, active: self.active_view == 1 },
            ChromeTabConfig { id: "tab-2", label: "Settings",  icon: None, color_tag: None, closable: false, active: self.active_view == 2 },
        ];
        let chrome_view = ChromeView {
            tabs: &chrome_tabs,
            active_tab_id: Some(tab_ids[self.active_view]),
            show_new_tab_btn: false,
            show_menu_btn: false,
            is_maximized: false,
            cursor_x: mx,
            cursor_y: my,
            time_ms,
        };
        let chrome_settings = ChromeSettings::default();
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
        let help_btn_active = self.dropdown_help_state.open;
        let top_toolbar_items = [
            ToolbarItem::TextButton { id: "tb-file", text: "File", active: file_btn_active, tooltip: Some("File menu") },
            ToolbarItem::TextButton { id: "tb-view", text: "View", active: view_btn_active, tooltip: Some("View menu") },
            ToolbarItem::TextButton { id: "tb-help", text: "Help", active: help_btn_active, tooltip: Some("Help menu") },
            ToolbarItem::Separator,
            ToolbarItem::TextButton { id: "tb-new", text: "New", active: false, tooltip: Some("New chart") },
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
            &ToolbarSettings::default(),
            &ToolbarRenderKind::Horizontal,
        );

        // ── Left vertical toolbar ─────────────────────────────────────────────
        let sidebar_open = self.sidebar_open;
        let left_items = [
            ToolbarItem::IconButton { id: "lt-toggle-sidebar", icon: &uzor::types::IconId::new("sidebar"), active: sidebar_open, tooltip: Some("Toggle sidebar") },
            ToolbarItem::Separator,
            ToolbarItem::IconButton { id: "lt-zoom-in",  icon: &uzor::types::IconId::new("zoom-in"),  active: false, tooltip: Some("Zoom in") },
            ToolbarItem::IconButton { id: "lt-zoom-out", icon: &uzor::types::IconId::new("zoom-out"), active: false, tooltip: Some("Zoom out") },
            ToolbarItem::Separator,
            ToolbarItem::IconButton { id: "lt-draw",     icon: &uzor::types::IconId::new("pencil"),   active: false, tooltip: Some("Draw mode") },
        ];
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
            &ToolbarSettings::default(),
            &ToolbarRenderKind::Vertical,
        );

        // ── Sidebar ───────────────────────────────────────────────────────────
        if self.sidebar_open {
            let modal_open = self.modal_open;
            let modal_kind = self.modal_kind;
            let sidebar_actions: &[HeaderAction<'_>] = &[];
            let sidebar_header = SidebarHeader { icon: None, title: "Navigation", actions: sidebar_actions };
            let mut sidebar_view = SidebarView {
                header: sidebar_header,
                tabs: &[],
                active_tab: None,
                show_scrollbar: false,
                content_height: 400.0,
            };
            let _sidebar_node = register_layout_manager_sidebar(
                &mut self.layout,
                &mut render,
                LayoutNodeId::ROOT,
                "sidebar",
                "sidebar-widget",
                &mut self.sidebar_state,
                &mut sidebar_view,
                &SidebarSettings::default(),
                &SidebarRenderKind::Left,
            );
            // Draw sidebar body content inline
            if let Some(body_rect) = self.layout.rect_for_edge_slot("sidebar") {
                let mut y = body_rect.y + 8.0;
                render.set_fill_color("rgba(255,255,255,0.25)");
                render.fill_text("MODALS", body_rect.x + 12.0, y + 11.0);
                y += 28.0;
                let btns: &[(&str, ModalKind, &str)] = &[
                    ("Open L2 Modal", ModalKind::L2, "#2962ff"),
                    ("Open L1 Modal", ModalKind::L1, "#10b981"),
                    ("Settings Modal", ModalKind::Settings, "#7c3aed"),
                ];
                for (label, kind, color) in btns {
                    let is_open = modal_open && modal_kind == *kind;
                    let btn_color = if is_open { "#ef5350" } else { *color };
                    render.set_fill_color(btn_color);
                    let bx = body_rect.x + 8.0;
                    let bw = body_rect.width - 16.0;
                    render.fill_rounded_rect(bx, y, bw, 30.0, 4.0);
                    render.set_fill_color("#ffffff");
                    render.fill_text(if is_open { "Close Modal" } else { *label }, bx + bw / 2.0, y + 15.0);
                    y += 38.0;
                }
                y += 12.0;
                render.set_fill_color("rgba(255,255,255,0.25)");
                render.fill_text("PANELS", body_rect.x + 12.0, y + 11.0);
                y += 28.0;
                let panel_labels = ["Dashboard", "Charts", "Settings"];
                for lbl in &panel_labels {
                    render.set_fill_color("rgba(255,255,255,0.07)");
                    render.fill_rounded_rect(body_rect.x + 8.0, y, body_rect.width - 16.0, 26.0, 3.0);
                    render.set_fill_color("#d1d4dc");
                    render.fill_text(*lbl, body_rect.x + 16.0, y + 13.0);
                    y += 32.0;
                }
                let _ = y;
            }
        }

        // ── Main content (dock panels) ────────────────────────────────────────
        {
            let view = self.active_view;
            let view_labels = ["Dashboard — market overview", "Charts — price data", "Settings — preferences"];
            let view_label = view_labels[view];
            let mut panel_a_view = PanelView {
                header: Some(PanelHeader { title: view_label, actions: &[] }),
                columns: &[],
                show_scrollbar: false,
                content_height: 600.0,
            };
            if let Some(ref id) = self.leaf_a_id.clone() {
                let _panel_a_node = register_layout_manager_panel(
                    &mut self.layout, &mut render,
                    LayoutNodeId::ROOT, id, "panel-a-widget",
                    &mut self.panel_a_state,
                    &mut panel_a_view,
                    &PanelSettings::default(),
                    &PanelRenderKind::WithHeader,
                );
                // Draw panel A body inline
                if let Some(body_rect) = self.layout.rect_for(id.as_str()) {
                    render.set_fill_color("rgba(255,255,255,0.04)");
                    render.fill_rounded_rect(body_rect.x + 12.0, body_rect.y + 12.0, body_rect.width - 24.0, 60.0, 4.0);
                    render.set_fill_color("rgba(255,255,255,0.55)");
                    render.fill_text("Right-click for context menu", body_rect.x + body_rect.width / 2.0, body_rect.y + 44.0);
                    render.set_fill_color("rgba(100,180,255,0.5)");
                    render.fill_text("Use toolbar buttons to open dropdowns", body_rect.x + body_rect.width / 2.0, body_rect.y + 80.0);
                }
            }
        }
        {
            let mut panel_b_view = PanelView {
                header: Some(PanelHeader { title: "Trade History", actions: &[] }),
                columns: &[],
                show_scrollbar: false,
                content_height: 400.0,
            };
            if let Some(ref id) = self.leaf_b_id.clone() {
                let _panel_b_node = register_layout_manager_panel(
                    &mut self.layout, &mut render,
                    LayoutNodeId::ROOT, id, "panel-b-widget",
                    &mut self.panel_b_state,
                    &mut panel_b_view,
                    &PanelSettings::default(),
                    &PanelRenderKind::WithHeader,
                );
                // Draw panel B body inline
                if let Some(body_rect) = self.layout.rect_for(id.as_str()) {
                    render.set_fill_color("rgba(255,255,255,0.04)");
                    render.fill_rounded_rect(body_rect.x + 12.0, body_rect.y + 12.0, body_rect.width - 24.0, 40.0, 4.0);
                    render.set_fill_color("rgba(255,255,255,0.5)");
                    render.fill_text("Panel B — Trade History", body_rect.x + body_rect.width / 2.0, body_rect.y + 32.0);
                }
            }
        }

        // ── Modal ─────────────────────────────────────────────────────────────
        if self.modal_open {
            let modal_w = match self.modal_kind {
                ModalKind::L2 => L2_WIN_W + 24.0,
                ModalKind::L1 => 320.0,
                ModalKind::Settings => 400.0,
            };
            let modal_h = match self.modal_kind {
                ModalKind::L2 => L2_WIN_H + 80.0,
                ModalKind::L1 => 200.0,
                ModalKind::Settings => 300.0,
            };
            let modal_rect = Rect::new(
                (width as f64 / 2.0 - modal_w / 2.0).max(0.0),
                (height as f64 / 2.0 - modal_h / 2.0).max(0.0),
                modal_w,
                modal_h,
            );
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
                ModalKind::L2 => "L2 Widget Set",
                ModalKind::L1 => "L1 Custom Button",
                ModalKind::Settings => "Settings",
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
                        render.set_fill_color("rgba(255,255,255,0.55)");
                        render.fill_text("Hand-rolled MyButton (no uzor button widget)", body_rect.x + body_rect.width / 2.0, body_rect.y + 20.0);
                        let btn_r = Rect::new(body_rect.x + 60.0, body_rect.y + 40.0, 200.0, 60.0);
                        use uzor::input::core::sense::Sense;
                        use uzor::input::core::widget_kind::WidgetKind;
                        self.layout.ctx_mut().input.register_atomic(
                            WidgetId::new("l1-mybtn"),
                            WidgetKind::Custom,
                            btn_r,
                            Sense::CLICK | Sense::HOVER,
                            &layer,
                        );
                        render.set_fill_color("#3366bb");
                        render.fill_rounded_rect(btn_r.x - 1.0, btn_r.y - 1.0, btn_r.width + 2.0, btn_r.height + 2.0, 7.0);
                        render.set_fill_color("#3769af");
                        render.fill_rounded_rect(btn_r.x, btn_r.y, btn_r.width, btn_r.height, 6.0);
                        render.set_fill_color("#c8e6ff");
                        render.fill_rounded_rect(btn_r.x + btn_r.width / 2.0 - 6.0, btn_r.y + btn_r.height / 2.0 - 6.0, 12.0, 12.0, 6.0);
                        render.set_fill_color("#ffffff");
                        render.fill_text("Click me (L1 custom)", btn_r.x + btn_r.width / 2.0, btn_r.y + btn_r.height / 2.0);
                    }
                    ModalKind::Settings => {
                        render.set_fill_color("rgba(255,255,255,0.55)");
                        render.fill_text("Settings content", body_rect.x + body_rect.width / 2.0, body_rect.y + 20.0);
                        let items = [
                            ("Enable dark mode", true),
                            ("Show tooltips",    true),
                            ("Auto-save",        false),
                        ];
                        let mut ctx_local = ContextManager::new(LayoutNode::new("settings-modal"));
                        for (i, (label, checked)) in items.iter().enumerate() {
                            let r = Rect::new(body_rect.x + 16.0, body_rect.y + 48.0 + i as f64 * 36.0, body_rect.width - 32.0, 28.0);
                            let cb_id = format!("settings-cb-{i}");
                            register_context_manager_checkbox(
                                &mut ctx_local, &mut render,
                                cb_id.as_str(), r, &layer,
                                WidgetState::Normal,
                                &CheckboxView { checked: *checked, label: Some(label) },
                                &CheckboxSettings::default().with_theme(Box::new(VisibleCheckboxTheme)),
                                &CheckboxRenderKind::Standard,
                                "13px sans-serif",
                            );
                        }
                    }
                    ModalKind::L2 => {
                        // Full L2 widget set rendered inside modal body
                        let left_panel_w = l2_right_panel_x - LEFT_PANEL_X - SPLITTER_W / 2.0;
                        let ox = body_rect.x;
                        let oy = body_rect.y;

                        let text_id = WidgetId::new("l2-text");
                        let text_str = self.layout.ctx_mut().input.text_fields().text(&text_id).to_owned();
                        let text_cursor = self.layout.ctx_mut().input.text_fields().cursor(&text_id);
                        let text_sel = self.layout.ctx_mut().input.text_fields().selection_range(&text_id);
                        let text_focused = self.layout.ctx_mut().input.text_fields().is_focused(&text_id);
                        let now_ms = start_time.elapsed().as_millis() as u64;
                        let cursor_vis = text_focused && self.layout.ctx_mut().input.text_fields().cursor_visible(now_ms);

                        // Draw panel BGs via RenderContext
                        render.set_fill_color("#1e222d");
                        render.fill_rounded_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, L2_WIN_H - 24.0, 8.0);
                        render.fill_rounded_rect(ox + l2_right_panel_x, oy + 12.0, L2_WIN_W - l2_right_panel_x - 12.0, L2_WIN_H - 24.0, 8.0);

                        let mut ctx_l2 = ContextManager::new(LayoutNode::new("l2-modal"));

                        // ── Left panel (clipped) ──────────────────────────
                        render.save();
                        render.clip_rect(ox + LEFT_PANEL_X, oy + 12.0, left_panel_w, L2_WIN_H - 24.0);

                        // 1. Button
                        let btn_state = if l2_hovered.as_deref() == Some("l2-btn-connect") { WidgetState::Hovered } else if l2_connected { WidgetState::Active } else { WidgetState::Normal };
                        register_context_manager_button(
                            &mut ctx_l2, &mut render,
                            "l2-btn-connect", Rect::new(BTN_RECT.x + ox, BTN_RECT.y + oy, BTN_RECT.width, BTN_RECT.height), &layer,
                            btn_state,
                            &ButtonView { text: Some(if l2_connected { "Disconnect" } else { "Connect" }), icon: None, active: l2_connected, disabled: false, active_border: None },
                            &ButtonSettings::default().with_theme(Box::new(VisibleButtonTheme)),
                        );
                        // 2. Checkbox
                        register_context_manager_checkbox(
                            &mut ctx_l2, &mut render,
                            "l2-cb", Rect::new(CB_RECT.x + ox, CB_RECT.y + oy, CB_RECT.width, CB_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-cb") { WidgetState::Hovered } else { WidgetState::Normal },
                            &CheckboxView { checked: l2_checked, label: Some("Setting A") },
                            &CheckboxSettings::default().with_theme(Box::new(VisibleCheckboxTheme)),
                            &CheckboxRenderKind::Standard, "13px sans-serif",
                        );
                        // 3. Toggle
                        register_context_manager_toggle(
                            &mut ctx_l2, &mut render,
                            "l2-tog", Rect::new(TOG_RECT.x + ox, TOG_RECT.y + oy, TOG_RECT.width, TOG_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-tog") { WidgetState::Hovered } else { WidgetState::Normal },
                            &ToggleView { toggled: l2_toggled, label: Some("ON"), disabled: false },
                            &ToggleSettings::default(), &ToggleRenderKind::Switch,
                        );
                        // 4. Radio
                        for (i, cx_off) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                            let rid = format!("l2-radio-{i}");
                            register_context_manager_radio(
                                &mut ctx_l2, &mut render,
                                rid.as_str(), Rect::new(cx_off + ox, 175.0 + oy, 28.0, 28.0), &layer,
                                if l2_hovered.as_deref() == Some(rid.as_str()) { WidgetState::Hovered } else { WidgetState::Normal },
                                &RadioSettings::default(),
                                &RadioRenderKind::Dot { shape: DotShape::Circle, cx: cx_off + 14.0 + ox, cy: 175.0 + 14.0 + oy, view: RadioDotView { selected: l2_radio_sel == i } },
                            );
                        }
                        // 5. Slider
                        register_context_manager_slider(
                            &mut ctx_l2, &mut render,
                            "l2-slider", Rect::new(SLID_RECT.x + ox, SLID_RECT.y + oy, SLID_RECT.width, SLID_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-slider") { WidgetState::Hovered } else { WidgetState::Normal },
                            &SliderView { kind: SliderType::Single { value: l2_slider_val, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: None },
                            &SliderSettings::default(),
                        );
                        // 6. Range slider
                        register_context_manager_slider(
                            &mut ctx_l2, &mut render,
                            "l2-range", Rect::new(RANGE_RECT.x + ox, RANGE_RECT.y + oy, RANGE_RECT.width, RANGE_RECT.height), &layer,
                            if l2_hovered.as_deref() == Some("l2-range") { WidgetState::Hovered } else { WidgetState::Normal },
                            &SliderView { kind: SliderType::Dual { min_value: l2_range_min, max_value: l2_range_max, min: 0.0, max: 100.0, step: 1.0 }, hovered: false, disabled: false, dragging_handle: l2_range_drag_handle },
                            &SliderSettings::default(),
                        );
                        // 7. Separator
                        register_context_manager_separator(
                            &mut ctx_l2, &mut render,
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
                                &mut ctx_l2, &mut render,
                                sid.as_str(), Rect::new(28.0 + i as f64 * 34.0 + ox, 344.0 + oy, 26.0, 26.0), &layer,
                                if l2_hovered.as_deref() == Some(sid.as_str()) { WidgetState::Hovered } else { WidgetState::Normal },
                                &ColorSwatchView { color: *color, hovered: false, selected: l2_swatch_sel == i, show_transparency: false, border_color_override: None },
                                &ColorSwatchSettings::default(), &ColorSwatchRenderKind::Simple,
                            );
                        }

                        render.restore();

                        // ── Right panel (clipped) ─────────────────────────
                        render.save();
                        render.clip_rect(ox + l2_right_panel_x, oy + 12.0, L2_WIN_W - l2_right_panel_x - 12.0, L2_WIN_H - 24.0);

                        let tab_labels = ["List", "Empty", "Sub-tabs"];
                        for (i, lbl) in tab_labels.iter().enumerate() {
                            let tab_w = ((l2_right_panel_w - 16.0) / 3.0).floor();
                            let tab_x = l2_right_panel_x + 8.0 + i as f64 * (tab_w + 4.0);
                            let tab_rect = Rect::new(tab_x + ox, TAB_STRIP_Y + oy, tab_w, TAB_STRIP_H);
                            let tab_id = format!("l2-tab-{i}");
                            let tab_cfg = TabConfig::new(tab_id.as_str(), *lbl).active_if(l2_active_tab == i);
                            register_context_manager_tab(
                                &mut ctx_l2, &mut render,
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
                            register_context_manager_scrollbar(&mut ctx_l2, &mut render, "l2-sb-track", "l2-sb-thumb", sb_track, sb_thumb, 5.0, &layer, CONTENT_H, viewport_h, l2_scroll_off, &ScrollbarSettings::default());

                            let row_labels = ["★ Roboto regular","Sans-serif clean","→ arrow + ✓ check","Quick brown fox","✨ ★ ☀ ☂ ❤","fn main() { ... }","let x: u32 = 42;","if let Some(v) = opt","// monospace code","0xCAFE_BABE","PT Root UI light","вариативный шрифт","12345 67890","Кириллица OK","ƒ unicode glyphs","Bold Roboto bold","❗ Heads up ❗","✓ Done · 14 items","🌍 globe · 🌟 star","═══ end of list ═══"];
                            let content_x = l2_right_panel_x + 8.0 + ox;
                            let content_w = l2_right_panel_w - SB_W - 20.0;
                            for row in 0..CONTENT_ROWS {
                                let row_y = 52.0 + oy + row as f64 * ROW_H - l2_scroll_off;
                                if row_y + ROW_H < 52.0 + oy || row_y > 52.0 + oy + SB_H { continue; }
                                let row_rect = Rect::new(content_x, row_y, content_w, ROW_H - 2.0);
                                let row_id = format!("l2-row-{row}");
                                let row_settings = match row { 0..=4 => ItemSettings::default().with_style(Box::new(RowStyleRoboto)), 5..=9 => ItemSettings::default().with_style(Box::new(RowStyleJetBrains)), 10..=14 => ItemSettings::default().with_style(Box::new(RowStylePtRoot)), _ => ItemSettings::default().with_style(Box::new(RowStyleRobotoBold)) };
                                register_context_manager_item(&mut ctx_l2, &mut render, row_id.as_str(), row_rect, &layer, WidgetState::Normal, &ItemView { label: Some(row_labels[row]), icon: None, svg: None }, &row_settings, &ItemRenderKind::Label);
                            }
                        }
                        if l2_active_tab == 2 {
                            for (i, lbl) in ["Alpha","Beta","Gamma"].iter().enumerate() {
                                let sub_rect = Rect::new(l2_right_panel_x + 8.0 + ox, CONTENT_START_Y + 8.0 + i as f64 * 36.0 + oy, 90.0, 30.0);
                                let sub_id = format!("l2-sub-tab-{i}");
                                let sub_cfg = TabConfig::new(sub_id.as_str(), *lbl).active_if(l2_active_sub_tab == i);
                                register_context_manager_tab(&mut ctx_l2, &mut render, sub_id.as_str(), sub_rect, None, &layer, &TabView { tab: &sub_cfg, hovered: l2_hovered.as_deref() == Some(sub_id.as_str()), pressed: false, close_btn_hovered: false }, &TabSettings::default());
                            }
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

                        // Splitter drag handle (no clip needed)
                        let dh_rect = Rect::new(l2_right_panel_x - SPLITTER_W / 2.0 + ox, 12.0 + oy, SPLITTER_W, L2_WIN_H - 24.0);
                        register_context_manager_drag_handle(
                            &mut ctx_l2, &mut render,
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
                ContextMenuItem { action: "ctx-copy",   label: "Copy",       icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-paste",  label: "Paste",      icon: None, danger: false, separator_after: false, enabled: true },
                ContextMenuItem { action: "ctx-delete", label: "Delete",     icon: None, danger: true,  separator_after: true,  enabled: true },
                ContextMenuItem { action: "ctx-props",  label: "Properties", icon: None, danger: false, separator_after: false, enabled: true },
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
            DropdownItem::Item { id: "view-sidebar", label: "Toggle Sidebar", icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "view-toolbar", label: "Show Toolbar",   icon: None, right: DropdownItemRight::Toggle(true), disabled: false, danger: false, accent_color: None },
            DropdownItem::Separator,
            DropdownItem::Item { id: "view-zoom-in",  label: "Zoom In",  icon: None, right: DropdownItemRight::Shortcut("+"), disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "view-zoom-out", label: "Zoom Out", icon: None, right: DropdownItemRight::Shortcut("-"), disabled: false, danger: false, accent_color: None },
        ];
        if self.dropdown_view_state.open {
            let hovered_id = self.dropdown_view_state.hovered_id.clone();
            let origin = self.dropdown_view_state.effective_origin();
            self.layout.push_overlay(OverlayEntry {
                id: "dd-view-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, 200.0, 180.0),
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
            DropdownItem::Item { id: "help-about", label: "About",    icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "help-docs",  label: "Docs",     icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
            DropdownItem::Item { id: "help-keys",  label: "Shortcuts",icon: None, right: DropdownItemRight::None, disabled: false, danger: false, accent_color: None },
        ];
        if self.dropdown_help_state.open {
            let hovered_id = self.dropdown_help_state.hovered_id.clone();
            let origin = self.dropdown_help_state.effective_origin();
            self.layout.push_overlay(OverlayEntry {
                id: "dd-help-overlay".to_string(),
                kind: OverlayKind::Dropdown,
                rect: Rect::new(origin.0, origin.1, 180.0, 110.0),
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
                render.set_fill_color("#d1d4dc");
                render.fill_text(text_for_popup, body_rect.x + body_rect.width / 2.0, body_rect.y + 16.0);
            }
        }

        // ── end_frame ─────────────────────────────────────────────────────────
        let responses = self.layout.ctx_mut().input.end_frame();

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

        // Clear overlays — pushed fresh each frame
        self.layout.clear_overlays();

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
        // ── Priority 1: coord-resolved widget id ──────────────────────────────
        // If InputCoordinator identified the click as landing on a registered
        // widget, dispatch by id BEFORE the manual geometry fallback.  This
        // makes every register_*-ed widget click-able without per-button hit
        // testing in the example.
        if let Some(id) = clicked_id.as_ref() {
            let id_str = id.0.as_str();
            // Modal close affordances (X / Apply / Cancel inside any modal)
            if id_str.starts_with("modal-") && (
                id_str.ends_with("-close") ||
                id_str.ends_with("-apply") ||
                id_str.ends_with("-cancel")
            ) {
                self.modal_open = false;
                println!("[L3] modal closed via {id_str}");
                return;
            }
            // Dropdown items
            if let Some(rest) = id_str.strip_prefix("dropdown-") {
                println!("[L3] dropdown item → {rest}");
                self.dropdown_file_state.close();
                self.dropdown_view_state.close();
                self.dropdown_help_state.close();
                return;
            }
            // Context menu items
            if let Some(rest) = id_str.strip_prefix("ctxmenu-") {
                println!("[L3] context menu → {rest}");
                self.ctx_menu_state.close();
                return;
            }
            // Else fall through to manual dispatch
        }
        let _ = clicked_id; // silence unused warning if no fall-through uses it

        // ── Chrome hit ────────────────────────────────────────────────────────
        let tab_ids = ["tab-0", "tab-1", "tab-2"];
        let chrome_tabs = [
            ChromeTabConfig { id: "tab-0", label: "Dashboard", icon: None, color_tag: None, closable: false, active: self.active_view == 0 },
            ChromeTabConfig { id: "tab-1", label: "Charts",    icon: None, color_tag: None, closable: false, active: self.active_view == 1 },
            ChromeTabConfig { id: "tab-2", label: "Settings",  icon: None, color_tag: None, closable: false, active: self.active_view == 2 },
        ];
        let chrome_view = ChromeView { tabs: &chrome_tabs, active_tab_id: Some(tab_ids[self.active_view]), show_new_tab_btn: false, show_menu_btn: false, is_maximized: false, cursor_x: x, cursor_y: y, time_ms: self.time_ms() };
        if let Some(chrome_rect) = self.layout.rect_for_chrome() {
            let hit = chrome_hit_test(&self.chrome_state, &chrome_view, &ChromeSettings::default(), &ChromeRenderKind::Default, chrome_rect, (x, y));
            match handle_chrome_action(hit) {
                ChromeAction::SelectTab(i) => {
                    self.active_view = i;
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

        // ── Sidebar modal buttons ─────────────────────────────────────────────
        if self.sidebar_open {
            if let Some(sb_rect) = self.layout.rect_for_edge_slot("sidebar") {
                let body_y = sb_rect.y + 40.0; // approx header
                let mut by = body_y + 8.0 + 28.0; // skip section header
                let modal_kinds = [ModalKind::L2, ModalKind::L1, ModalKind::Settings];
                for kind in &modal_kinds {
                    let bx = sb_rect.x + 8.0;
                    let bw = sb_rect.width - 16.0;
                    if x >= bx && x <= bx + bw && y >= by && y <= by + 30.0 {
                        if self.modal_open && self.modal_kind == *kind {
                            self.modal_open = false;
                        } else {
                            self.modal_open = true;
                            self.modal_kind = *kind;
                        }
                        println!("[L3] modal → {:?}", self.modal_kind);
                        return;
                    }
                    by += 38.0;
                }
            }
        }

        // ── Dropdown item clicks ──────────────────────────────────────────────
        let any_dd_open = self.dropdown_file_state.open || self.dropdown_view_state.open || self.dropdown_help_state.open;
        if any_dd_open {
            let clicked_dd =
                (self.dropdown_file_state.open && self.layout.rect_for_overlay("dd-file-overlay").map(|r| r.contains(x, y)).unwrap_or(false)) ||
                (self.dropdown_view_state.open && self.layout.rect_for_overlay("dd-view-overlay").map(|r| r.contains(x, y)).unwrap_or(false)) ||
                (self.dropdown_help_state.open && self.layout.rect_for_overlay("dd-help-overlay").map(|r| r.contains(x, y)).unwrap_or(false));
            if !clicked_dd {
                self.dropdown_file_state.close();
                self.dropdown_view_state.close();
                self.dropdown_help_state.close();
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
                    match hid.as_str() {
                        "view-sidebar" => { self.sidebar_open = !self.sidebar_open; }
                        _ => println!("[L3] view item: {hid}"),
                    }
                    self.dropdown_view_state.close();
                }
                if let Some(ref hid) = self.dropdown_help_state.hovered_id.clone() {
                    println!("[L3] help: {hid}");
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
                    println!("[L3] ctx menu click inside");
                    self.ctx_menu_state.close();
                }
            }
        }

        // ── L2 modal widget clicks ────────────────────────────────────────────
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                // body starts after header (~40px) + inside padding
                let body_y = modal_rect.y + 40.0;
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
        let (w, h) = { let s = &self.surface; (s.config.width as f64, s.config.height as f64) };
        self.ctx_menu_state.open_smart(x, y, w, h, 170.0, 130.0, None);
        self.dropdown_file_state.close();
        self.dropdown_view_state.close();
        self.dropdown_help_state.close();
    }

    fn on_mouse_down(&mut self, x: f64, y: f64) {
        self.last_mouse = (x, y);
        self.mouse_down = true;
        self.drag_origin = Some((x, y));

        // L2 drag targets
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let body_y = modal_rect.y + 40.0;
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

        // L2 hover tracking
        if self.modal_open && self.modal_kind == ModalKind::L2 {
            if let Some(modal_rect) = self.layout.rect_for_overlay("modal-overlay") {
                let body_y = modal_rect.y + 40.0;
                let rel_x = x - modal_rect.x;
                let rel_y = y - body_y;
                self.l2_hovered = self.l2_hovered_at(rel_x, rel_y);
            }
        }

        // Drag
        if let (Some((ox, oy)), Some(ref target)) = (self.drag_origin, self.drag_target.as_ref()) {
            let dx = x - ox;
            let dy = y - oy;
            match target {
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
        self.drag_target = None;
        self.l2_range_drag_handle = None;
        self.l2_pressed = None;
    }
}

// =============================================================================
// Layout setup
// =============================================================================

fn setup_dock(layout: &mut LayoutManager<DemoPanel>) -> (String, String) {
    let leaf_a = layout.panels_mut().tree_mut().add_leaf(DemoPanel { title: "Content".into() });
    let ids = layout.panels_mut().tree_mut().split_leaf(leaf_a, SplitKind::Horizontal, 0.0, 0.0);
    let leaf_a2 = ids[0];
    let leaf_b = ids[1];
    if let Some(leaf) = layout.panels_mut().tree_mut().leaf_mut(leaf_b) {
        if let Some(panel) = leaf.panels.first_mut() {
            panel.title = "Trade History".into();
        }
    }
    (leaf_a2.to_string(), leaf_b.to_string())
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
            .with_resizable(true);
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
        let (leaf_a_id, leaf_b_id) = setup_dock(&mut layout);

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
            panel_a_state: PanelState::default(),
            panel_b_state: PanelState::default(),
            modal_state: ModalState::default(),
            popup_state: PopupState::default(),
            ctx_menu_state: ContextMenuState::default(),
            dropdown_file_state: DropdownState::default(),
            dropdown_view_state: DropdownState::default(),
            dropdown_help_state: DropdownState::default(),
            active_view: 0,
            sidebar_open: true,
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
            leaf_a_id: Some(leaf_a_id),
            leaf_b_id: Some(leaf_b_id),
            exit_requested: false,
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

        // Bridge handles text-field key routing + clipboard
        let focused = app.layout.ctx_mut().input.focused_widget().cloned();
        let out = app.bridge.handle_event(&mut app.layout.ctx_mut().input, focused.as_ref(), &event);

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

        // Scroll wheel
        if let Some(((_cx, _cy), (_, dy))) = out.wheel {
            if app.modal_open && app.modal_kind == ModalKind::L2 {
                app.l2_scroll_off = (app.l2_scroll_off - dy * 20.0)
                    .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut handler = Handler { state: None };
    event_loop.run_app(&mut handler)?;
    Ok(())
}
