//! # Level 2 — uzor multi-widget demo
//!
//! Demonstrates 13 different atomic widgets rendered through the L2 API
//! (`register_context_manager_<X>`).  All 13 widgets are fully interactive:
//! clicks, hovers, drag (slider + scrollbar + drag-handle), scroll (scrollbar),
//! and keyboard (text input via InputCoordinator text field store).
//!
//! Event pipeline (mlc direct-coord pattern — NO EventProcessor / PlatformEvent / InputState):
//!   winit WindowEvent
//!     → app parses x/y/button/key directly
//!     → calls coord methods: process_click, text_fields_mut().on_char, etc.
//!     → render: ctx.begin_frame(input, viewport)  (minimal InputState built inline)
//!     → register widgets
//!     → ctx.end_frame() → Vec<(WidgetId, WidgetResponse)>
//!     → apply responses to app state
//!
//! Layout is hardcoded (no flex/yoga).  Window is 680 × 440.
//!
//! # Run
//!
//! ```sh
//! cargo run -p uzor-examples --bin l2-launcher
//! ```

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

// ── vello ──────────────────────────────────────────────────────────────────────
use vello::util::{RenderContext as VelloRenderCx, RenderSurface};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use vello::peniko::{Color, Fill};
use vello::kurbo::{Affine, RoundedRect};
use vello::wgpu;

// ── uzor core ─────────────────────────────────────────────────────────────────
use uzor::app_context::{ContextManager, layout::types::LayoutNode};
use uzor::input::core::coordinator::LayerId;
use uzor::input::keyboard::keyboard::KeyPress;
use uzor::input::pointer::state::{InputState, PointerState};
use uzor::input::text::store::TextFieldConfig;
use uzor::types::{Rect, WidgetState, unsafe_widget_id};

// ── widgets ──────────────────────────────────────────────────────────────────
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonTheme, ButtonView};

use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::theme::CheckboxTheme;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

use uzor::ui::widgets::atomic::radio::settings::RadioSettings;
use uzor::ui::widgets::atomic::radio::types::{
    DotShape, RadioDotView, RadioRenderKind,
};

use uzor::ui::widgets::atomic::slider::render::SliderView;
use uzor::ui::widgets::atomic::slider::settings::SliderSettings;
use uzor::ui::widgets::atomic::slider::types::{DualSliderHandle, SliderType};

use uzor::ui::widgets::atomic::scrollbar::settings::ScrollbarSettings;

use uzor::ui::widgets::atomic::separator::input::SeparatorKind;
use uzor::ui::widgets::atomic::separator::render::SeparatorView;
use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};

use uzor::ui::widgets::atomic::text_input::render::{
    draw_input, draw_input_cursor, InputView,
};
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::types::InputType;

use uzor::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
use uzor::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};

use uzor::ui::widgets::atomic::close_button::render::CloseButtonView;
use uzor::ui::widgets::atomic::close_button::settings::CloseButtonSettings;
use uzor::ui::widgets::atomic::close_button::theme::CloseButtonTheme;
use uzor::ui::widgets::atomic::close_button::types::CloseButtonRenderKind;

use uzor::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
use uzor::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};

use uzor::ui::widgets::atomic::tab::render::TabView;
use uzor::ui::widgets::atomic::tab::settings::TabSettings;
use uzor::ui::widgets::atomic::tab::types::TabConfig;

use uzor::ui::widgets::atomic::item::render::ItemView;
use uzor::ui::widgets::atomic::item::settings::ItemSettings;
use uzor::ui::widgets::atomic::item::style::ItemStyle;
use uzor::ui::widgets::atomic::item::types::ItemRenderKind;

use uzor::ui::widgets::atomic::text_input::render::InputResult;

// ── SVG icon drawing ──────────────────────────────────────────────────────────
use uzor::render::draw_svg_icon;

// ── GPU render context ────────────────────────────────────────────────────────
use uzor_render_vello_gpu::VelloGpuRenderContext;

// ─────────────────────────────────────────────────────────────────────────────
// Fix 6: Inline SVG strings for icon grid (Tab 1) and sub-tab icons (Tab 2)
// ─────────────────────────────────────────────────────────────────────────────

const SVG_CIRCLE: &str = r#"<svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="9" stroke-width="2"/></svg>"#;
const SVG_SQUARE: &str = r#"<svg viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="18" height="18" rx="2" stroke-width="2"/></svg>"#;
const SVG_TRIANGLE: &str = r#"<svg viewBox="0 0 24 24" fill="none"><polyline points="12,3 22,21 2,21 12,3" stroke-width="2"/></svg>"#;
const SVG_DIAMOND: &str = r#"<svg viewBox="0 0 24 24" fill="none"><polyline points="12,2 22,12 12,22 2,12 12,2" stroke-width="2"/></svg>"#;

// ─────────────────────────────────────────────────────────────────────────────
// Fix 5: Per-group row font styles
// ─────────────────────────────────────────────────────────────────────────────

// Different uzor-bundled font families per group of rows.
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

// ─────────────────────────────────────────────────────────────────────────────
// Geometry + colours
// ─────────────────────────────────────────────────────────────────────────────

const WIN_W: u32 = 680;
const WIN_H: u32 = 440;

const BG: Color = Color::from_rgb8(0x16, 0x16, 0x1e);
const PANEL_BG: Color = Color::from_rgb8(0x1e, 0x22, 0x2d);

// Text field widget ID — used to register with coordinator and query state.
const TEXT_FIELD_ID: &str = "text-search";

// Left panel width (fixed). Right panel = WIN_W - left panel width - margins.
// The resize handle sits at x = LEFT_PANEL_X + left_panel_w.
const LEFT_PANEL_X: f64 = 12.0;
const SPLITTER_W: f64 = 6.0; // visual width of the drag handle strip

// Widget rects in the left panel (constants — do NOT depend on right panel width).
const BTN_RECT:   Rect = Rect { x: 28.0,  y: 28.0,  width: 130.0, height: 36.0 };
const CLOSE_RECT: Rect = Rect { x: 278.0, y: 28.0,  width: 24.0,  height: 24.0 };
const CB_RECT:    Rect = Rect { x: 28.0,  y: 88.0,  width: 160.0, height: 22.0 };
const TOG_RECT:   Rect = Rect { x: 28.0,  y: 130.0, width: 80.0,  height: 24.0 };
const SLID_RECT:  Rect = Rect { x: 28.0,  y: 200.0, width: 260.0, height: 24.0 };
const RANGE_RECT: Rect = Rect { x: 28.0,  y: 228.0, width: 260.0, height: 24.0 };
const TI_RECT:    Rect = Rect { x: 28.0,  y: 278.0, width: 200.0, height: 28.0 };

// ── Scrollbar geometry (right panel) ──────────────────────────────────────────
// X position depends on right_panel_width; computed per-frame.
// Tabs are rendered at top of right panel (y=12..40); content starts at y=52.
const TAB_STRIP_Y: f64 = 12.0;
const TAB_STRIP_H: f64 = 28.0;
const CONTENT_START_Y: f64 = 52.0;
const SB_Y: f64   =  CONTENT_START_Y;
const SB_W: f64   =  10.0;
// WIN_H=440: 440 - 52 - 12 = 376
const SB_H: f64   = 376.0;
// Content scroll extent
const CONTENT_ROWS: usize = 20;
const ROW_H: f64 = 28.0;
const CONTENT_H: f64 = CONTENT_ROWS as f64 * ROW_H; // 560 px

// ─────────────────────────────────────────────────────────────────────────────
// Custom themes
// ─────────────────────────────────────────────────────────────────────────────

// ── Button ────────────────────────────────────────────────────────────────────
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

// ── Fix 1: Checkbox — visible box with border ─────────────────────────────────
struct VisibleCheckboxTheme;

impl CheckboxTheme for VisibleCheckboxTheme {
    fn checkbox_bg_checked(&self) -> &str         { "#2962ff" }
    fn checkbox_bg_unchecked(&self) -> &str       { "#1a1a1f" }
    fn checkbox_border(&self) -> &str             { "#3a3a45" }
    fn checkbox_checkmark(&self) -> &str          { "#ffffff" }
    fn checkbox_notification_inner(&self) -> &str { "#ffffff" }
    fn checkbox_label_text(&self) -> &str         { "#d1d4dc" }
}

// ── Fix 2: Close button — red-ish hover (Win11 style) ─────────────────────────
struct VisibleCloseButtonTheme;

impl CloseButtonTheme for VisibleCloseButtonTheme {
    fn close_button_x_color(&self) -> &str       { "#a0a0a8" }
    fn close_button_x_color_hover(&self) -> &str { "#ffffff" }
    fn close_button_bg_hover(&self) -> &str       { "#c42b1c" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hit-test helper
// ─────────────────────────────────────────────────────────────────────────────

fn rect_contains(r: Rect, x: f64, y: f64) -> bool {
    x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height
}

// ─────────────────────────────────────────────────────────────────────────────
// App state
// ─────────────────────────────────────────────────────────────────────────────

/// Drag origin + which widget is being dragged and its starting value.
enum DragTarget {
    Slider(f64),        // start value (0-100)
    RangeMin(f64),      // start range_min (0-100)
    RangeMax(f64),      // start range_max (0-100)
    Scroll(f64),        // start scroll_off
    Splitter(f64),      // start right_panel_width
}

struct AppState {
    window:    Arc<Window>,
    render_cx: VelloRenderCx,
    surface:   RenderSurface<'static>,
    renderer:  Renderer,
    scene:     Scene,

    ctx:        ContextManager,
    start_time: Instant,

    // mlc-style: app owns raw pointer position
    last_mouse_pos: (f64, f64),
    // drag state: origin + target widget + start value
    drag_origin:    Option<(f64, f64)>,
    drag_target:    Option<DragTarget>,

    // per-widget state
    connected:           bool,
    checked:             bool,
    toggled:             bool,
    radio_sel:           usize,
    slider_val:          f64,
    range_min:           f64,
    range_max:           f64,
    range_drag_handle:   Option<DualSliderHandle>,
    scroll_off:          f64,
    swatch_sel:          usize,

    // Fix 3: active tab index (0=List, 1=Empty, 2=Sub-tabs)
    active_tab:          usize,
    // Fix 3b: active sub-tab index (within Tab 2 sub-tabs)
    active_sub_tab:      usize,

    // Fix 4: right panel width (resizable by splitter drag)
    right_panel_width:   f64,
    // Fix 4: splitter drag direction — positive dx = dragging right (left panel grows)
    splitter_drag_dx:    f64,

    // mlc-style hover/press tracking — app owns the state machine
    hovered_widget_id: Option<String>,
    pressed_widget_id: Option<String>,

    // keyboard modifier state — updated on ModifiersChanged
    modifiers_shift: bool,
    modifiers_ctrl:  bool,

    // text drag selection — true while LMB is held inside the text field
    text_dragging: bool,

    // last rendered text-input geometry; used for click→cursor and blink
    last_input_result: InputResult,
}

impl AppState {
    fn current_time(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    fn text_focused(&self) -> bool {
        self.ctx.input.text_fields().is_focused(&unsafe_widget_id(TEXT_FIELD_ID))
    }

    /// Scrollbar X (right edge of right panel minus scrollbar width and margin).
    fn sb_x(&self) -> f64 {
        WIN_W as f64 - SB_W - 8.0
    }

    /// Right panel content area X (left edge).
    fn right_panel_x(&self) -> f64 {
        WIN_W as f64 - self.right_panel_width
    }

    /// Splitter rect — vertical strip on the boundary.
    fn splitter_rect(&self) -> Rect {
        let rx = self.right_panel_x();
        Rect::new(rx - SPLITTER_W / 2.0, 12.0, SPLITTER_W, WIN_H as f64 - 24.0)
    }

    /// Compute which widget is under the pointer, given current mouse position.
    fn compute_hovered(&self, mouse: (f64, f64)) -> Option<String> {
        let (mx, my) = mouse;
        if rect_contains(BTN_RECT,   mx, my) { return Some("btn-connect".into()); }
        if rect_contains(CLOSE_RECT, mx, my) { return Some("btn-close".into()); }
        if rect_contains(CB_RECT,    mx, my) { return Some("cb-setting-a".into()); }
        if rect_contains(TOG_RECT,   mx, my) { return Some("tog-enable".into()); }
        if rect_contains(SLID_RECT,  mx, my) { return Some("slider-main".into()); }
        if rect_contains(RANGE_RECT, mx, my) { return Some("range-slider".into()); }
        if rect_contains(TI_RECT,    mx, my) { return Some("text-search".into()); }

        // Fix 4: splitter hit-test (slightly wider than visual strip for ergonomics)
        let splitter = self.splitter_rect();
        let splitter_hit = Rect::new(splitter.x - 4.0, splitter.y, splitter.width + 8.0, splitter.height);
        if rect_contains(splitter_hit, mx, my) { return Some("splitter".into()); }

        // Radio buttons
        for i in 0..3_usize {
            let cx = 28.0 + i as f64 * 40.0;
            if rect_contains(Rect::new(cx, 175.0, 28.0, 28.0), mx, my) {
                return Some(format!("radio-opt-{i}"));
            }
        }
        // Color swatches
        for i in 0..4_usize {
            let sx = 28.0 + i as f64 * 34.0;
            if rect_contains(Rect::new(sx, 344.0, 26.0, 26.0), mx, my) {
                return Some(format!("swatch-{i}"));
            }
        }
        // Fix 3: Tab strip (3 tabs at top of right panel)
        for i in 0..3_usize {
            let tab_rect = self.tab_rect(i);
            if rect_contains(tab_rect, mx, my) {
                return Some(format!("tab-{i}"));
            }
        }
        // Fix 3b: Sub-tabs (only visible when active_tab == 2)
        if self.active_tab == 2 {
            for i in 0..3_usize {
                let sub_rect = self.sub_tab_rect(i);
                if rect_contains(sub_rect, mx, my) {
                    return Some(format!("sub-tab-{i}"));
                }
            }
        }
        // Scrollbar (only for tab 0)
        if self.active_tab == 0 {
            let sb_x = self.sb_x();
            let sb_track = Rect::new(sb_x, SB_Y, SB_W, SB_H);
            if rect_contains(sb_track, mx, my) { return Some("sb-track".into()); }
        }
        None
    }

    /// Tab strip rect — 3 tabs distributed evenly across the top of the right panel.
    fn tab_rect(&self, i: usize) -> Rect {
        let rx = self.right_panel_x();
        let rw = self.right_panel_width;
        let tab_w = ((rw - 16.0) / 3.0).floor();
        let tab_h = TAB_STRIP_H;
        let tab_x = rx + 8.0 + i as f64 * (tab_w + 4.0);
        Rect::new(tab_x, TAB_STRIP_Y, tab_w, tab_h)
    }

    /// Sub-tab rects (3 stacked in right panel when active_tab == 2).
    fn sub_tab_rect(&self, i: usize) -> Rect {
        let rx = self.right_panel_x() + 8.0;
        // content area starts at CONTENT_START_Y; micro-panel is inset by 8px
        Rect::new(rx, CONTENT_START_Y + 8.0 + i as f64 * 36.0, 90.0, 30.0)
    }

    /// Thumb height calculation (same formula used in render).
    fn thumb_h() -> f64 {
        let thumb_ratio = (SB_H / CONTENT_H).clamp(0.0, 1.0);
        (thumb_ratio * SB_H).max(30.0)
    }

    fn render(&mut self) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };
        let viewport = Rect::new(0.0, 0.0, width as f64, height as f64);

        let input = {
            let (mx, my) = self.last_mouse_pos;
            let mut s = InputState::default();
            s.pointer = PointerState {
                pos: Some((mx, my)),
                ..PointerState::default()
            };
            s.time = self.current_time();
            s
        };

        self.ctx.begin_frame(input, viewport);

        // ── Register text field with coordinator ─────────────────────────────
        self.ctx.input.register_text_field(
            TEXT_FIELD_ID,
            TI_RECT,
            TextFieldConfig::text(),
        );

        // ── scene ─────────────────────────────────────────────────────────────
        self.scene.reset();

        // background
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, BG, None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Snapshot splitter drag direction for z-order decisions (needed before cache section)
        let splitter_drag_dx = self.splitter_drag_dx;

        // Left panel rect
        let left_panel_w = self.right_panel_x() - LEFT_PANEL_X - SPLITTER_W / 2.0;
        let left_panel_rrect = RoundedRect::new(
            LEFT_PANEL_X, 12.0,
            LEFT_PANEL_X + left_panel_w, height as f64 - 12.0,
            8.0,
        );

        // Right panel rect
        let rx = self.right_panel_x();
        let right_panel_rrect = RoundedRect::new(
            rx, 12.0,
            width as f64 - 12.0, height as f64 - 12.0,
            8.0,
        );

        // Fix 4: z-order — during splitter drag, push the growing panel on top.
        // splitter_drag_dx > 0 means dragging right (left panel grows → left on top).
        // splitter_drag_dx < 0 means dragging left  (right panel grows → right on top).
        if splitter_drag_dx > 0.0 {
            // Left panel on top
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &right_panel_rrect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &left_panel_rrect);
        } else {
            // Right panel on top (default and when dragging left)
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &left_panel_rrect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &right_panel_rrect);
        }

        // Splitter visual strip
        {
            let sp = self.splitter_rect();
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY,
                Color::from_rgba8(255, 255, 255, 20), None,
                &vello::kurbo::Rect::new(sp.x, sp.y, sp.x + sp.width, sp.y + sp.height),
            );
        }

        // ── Fix 6b: Tab 2 micro-panel border (drawn before widgets = underneath) ──
        if self.active_tab == 2 {
            let rx2 = self.right_panel_x();
            let rw2 = self.right_panel_width;
            let mp_x = rx2 + 8.0;
            let mp_y = CONTENT_START_Y + 4.0;
            let mp_w = rw2 - 16.0;
            let mp_h = 3.0 * 36.0 + 8.0 + 64.0 + 24.0;
            let border_color = Color::from_rgba8(80, 90, 110, 200);
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, border_color, None,
                &RoundedRect::new(mp_x - 1.0, mp_y - 1.0, mp_x + mp_w + 1.0, mp_y + mp_h + 1.0, 6.0),
            );
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, PANEL_BG, None,
                &RoundedRect::new(mp_x, mp_y, mp_x + mp_w, mp_y + mp_h, 5.0),
            );
        }

        // ── Snapshot text field state ────────────────────────────────────────
        let text_id = unsafe_widget_id(TEXT_FIELD_ID);
        let text_str = self.ctx.input.text_fields()
            .text(&text_id)
            .to_owned();
        let text_cursor_pos = self.ctx.input.text_fields()
            .cursor(&text_id);
        let text_selection = self.ctx.input.text_fields()
            .selection_range(&text_id);
        let text_is_focused = self.ctx.input.text_fields()
            .is_focused(&text_id);
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        let cursor_visible = text_is_focused
            && self.ctx.input.text_fields().cursor_visible(now_ms);

        // ── Snapshot hover/press ──────────────────────────────────────────────
        let hovered = self.hovered_widget_id.clone();
        let pressed = self.pressed_widget_id.clone();

        // ── Cache per-frame computed values ──────────────────────────────────
        let active_tab = self.active_tab;
        let active_sub_tab = self.active_sub_tab;
        let right_panel_x = self.right_panel_x();
        let right_panel_width = self.right_panel_width;
        let sb_x = self.sb_x();
        let scroll_off = self.scroll_off;
        let dh_rect = self.splitter_rect();
        let connected = self.connected;
        let checked = self.checked;
        let toggled = self.toggled;
        let radio_sel = self.radio_sel;
        let slider_val = self.slider_val;
        let range_min = self.range_min;
        let range_max = self.range_max;
        let range_drag_handle = self.range_drag_handle;
        let swatch_sel = self.swatch_sel;

        // Snapshot pressed_id for use in closures
        let pressed_id_ref = pressed.clone();

        // ── Compute panel clip rects ──────────────────────────────────────────
        let left_clip = vello::kurbo::Rect::new(
            LEFT_PANEL_X, 12.0,
            LEFT_PANEL_X + left_panel_w, height as f64 - 12.0,
        );
        let right_clip = vello::kurbo::Rect::new(
            rx, 12.0,
            width as f64 - 12.0, height as f64 - 12.0,
        );

        // ── Closure: draw left-panel widgets (clipped) ───────────────────────
        // Returns (char_positions, input_result) from the text field.
        let draw_left = |scene: &mut Scene,
                         ctx: &mut ContextManager,
                         hovered: &Option<String>,
                         _pressed: &Option<String>|
         -> (Vec<f64>, InputResult) {
            scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &left_clip);
            let mut render = VelloGpuRenderContext::new(scene, 0.0, 0.0);
            let layer = LayerId::main();

            // ── 1. Button ─────────────────────────────────────────────────────
            let btn_state = {
                let base = match (hovered.as_deref() == Some("btn-connect"), false) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                };
                if base == WidgetState::Normal && connected {
                    WidgetState::Active
                } else {
                    base
                }
            };
            let btn_view = ButtonView {
                text:          Some(if connected { "Disconnect" } else { "Connect" }),
                icon:          None,
                active:        connected,
                disabled:      false,
                active_border: None,
                hover_chevron: None,
            };
            uzor::app_context::builders::draw_button(
                ctx, &mut render,
                "btn-connect", BTN_RECT, &layer,
                btn_state,
                &btn_view,
                &ButtonSettings::default().with_theme(Box::new(VisibleButtonTheme)),
            );

            // ── 2. Close button ───────────────────────────────────────────────
            uzor::app_context::builders::draw_close_button(
                ctx, &mut render,
                "btn-close", CLOSE_RECT, &layer,
                match (hovered.as_deref() == Some("btn-close"), pressed_id_ref.as_deref() == Some("btn-close")) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                },
                &CloseButtonView { hovered: hovered.as_deref() == Some("btn-close") },
                &CloseButtonSettings::default()
                    .with_theme(Box::new(VisibleCloseButtonTheme)),
                &CloseButtonRenderKind::Default,
            );

            // ── 3. Checkbox ───────────────────────────────────────────────────
            uzor::app_context::builders::draw_checkbox(
                ctx, &mut render,
                "cb-setting-a", CB_RECT, &layer,
                match (hovered.as_deref() == Some("cb-setting-a"), pressed_id_ref.as_deref() == Some("cb-setting-a")) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                },
                &CheckboxView { checked, label: Some("Setting A") },
                &CheckboxSettings::default()
                    .with_theme(Box::new(VisibleCheckboxTheme)),
                &CheckboxRenderKind::Standard,
                "13px sans-serif",
            );

            // ── 4. Toggle ─────────────────────────────────────────────────────
            uzor::app_context::builders::draw_toggle(
                ctx, &mut render,
                "tog-enable", TOG_RECT, &layer,
                match (hovered.as_deref() == Some("tog-enable"), pressed_id_ref.as_deref() == Some("tog-enable")) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                },
                &ToggleView { toggled, label: Some("ON"), disabled: false },
                &ToggleSettings::default(),
                &ToggleRenderKind::Switch,
            );

            // ── 5. Radio group ────────────────────────────────────────────────
            for (i, cx) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                let dot_rect = Rect::new(*cx, 175.0, 28.0, 28.0);
                let radio_id = format!("radio-opt-{i}");
                uzor::app_context::builders::draw_radio(
                    ctx, &mut render,
                    radio_id.as_str(), dot_rect, &layer,
                    match (hovered.as_deref() == Some(radio_id.as_str()), pressed_id_ref.as_deref() == Some(radio_id.as_str())) {
                        (_, true) => WidgetState::Pressed,
                        (true, _) => WidgetState::Hovered,
                        _ => WidgetState::Normal,
                    },
                    &RadioSettings::default(),
                    &RadioRenderKind::Dot {
                        shape: DotShape::Circle,
                        cx: cx + 14.0,
                        cy: 175.0 + 14.0,
                        view: RadioDotView { selected: radio_sel == i },
                    },
                );
            }

            // ── 6. Slider ─────────────────────────────────────────────────────
            uzor::app_context::builders::draw_slider(
                ctx, &mut render,
                "slider-main", SLID_RECT, &layer,
                match (hovered.as_deref() == Some("slider-main"), pressed_id_ref.as_deref() == Some("slider-main")) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                },
                &SliderView {
                    kind: SliderType::Single {
                        value: slider_val,
                        min: 0.0, max: 100.0, step: 1.0,
                    },
                    hovered: false,
                    disabled: false,
                    dragging_handle: None,
                },
                &SliderSettings::default(),
            );

            // ── 6b. Range slider ──────────────────────────────────────────────
            uzor::app_context::builders::draw_slider(
                ctx, &mut render,
                "range-slider", RANGE_RECT, &layer,
                match (hovered.as_deref() == Some("range-slider"), pressed_id_ref.as_deref() == Some("range-slider")) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                },
                &SliderView {
                    kind: SliderType::Dual {
                        min_value: range_min,
                        max_value: range_max,
                        min: 0.0, max: 100.0, step: 1.0,
                    },
                    hovered: hovered.as_deref() == Some("range-slider"),
                    disabled: false,
                    dragging_handle: range_drag_handle,
                },
                &SliderSettings::default(),
            );

            // ── 7. Separator ──────────────────────────────────────────────────
            let sep_rect = Rect::new(28.0, 260.0, 260.0, 2.0);
            uzor::app_context::builders::draw_separator(
                ctx, &mut render,
                "sep-h", sep_rect, SeparatorKind::Divider, &layer,
                &SeparatorView {
                    kind: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal },
                    hovered: false,
                    dragging: false,
                },
                &SeparatorSettings::default(),
            );

            // ── 8. Text input ─────────────────────────────────────────────────
            let ti_state = if text_is_focused {
                WidgetState::Active
            } else {
                match (hovered.as_deref() == Some(TEXT_FIELD_ID), pressed_id_ref.as_deref() == Some(TEXT_FIELD_ID)) {
                    (_, true) => WidgetState::Pressed,
                    (true, _) => WidgetState::Hovered,
                    _ => WidgetState::Normal,
                }
            };
            let ti_settings = TextInputSettings::with_config(
                uzor::ui::widgets::atomic::text_input::state::TextFieldConfig::text(),
            );
            let ti_view = InputView {
                text: text_str.as_str(),
                placeholder: "Search...",
                cursor: text_cursor_pos,
                selection: text_selection,
                focused: text_is_focused,
                disabled: false,
                input_type: InputType::Search,
            };
            let input_result = draw_input(&mut render, TI_RECT, ti_state, &ti_view, &ti_settings);
            if cursor_visible {
                draw_input_cursor(
                    &mut render,
                    input_result.cursor_x,
                    input_result.cursor_y,
                    input_result.cursor_height,
                    1.5,
                    [220, 220, 220, 255],
                );
            }
            let char_positions = input_result.char_x_positions.clone();
            let ir = input_result;

            // ── 9. Color swatches ─────────────────────────────────────────────
            let swatch_colors: [[u8; 4]; 4] = [
                [41, 98, 255, 255],
                [16, 185, 129, 255],
                [245, 158, 11, 255],
                [239, 83, 80, 255],
            ];
            for (i, color) in swatch_colors.iter().enumerate() {
                let sx = 28.0 + i as f64 * 34.0;
                let sw_rect = Rect::new(sx, 344.0, 26.0, 26.0);
                let sw_id = format!("swatch-{i}");
                uzor::app_context::builders::draw_color_swatch(
                    ctx, &mut render,
                    sw_id.as_str(), sw_rect, &layer,
                    match (hovered.as_deref() == Some(sw_id.as_str()), pressed_id_ref.as_deref() == Some(sw_id.as_str())) {
                        (_, true) => WidgetState::Pressed,
                        (true, _) => WidgetState::Hovered,
                        _ => WidgetState::Normal,
                    },
                    &ColorSwatchView {
                        color: *color,
                        hovered: false,
                        selected: swatch_sel == i,
                        show_transparency: false,
                        border_color_override: None,
                    },
                    &ColorSwatchSettings::default(),
                    &ColorSwatchRenderKind::Simple,
                );
            }

            drop(render);
            scene.pop_layer();
            (char_positions, ir)
        };

        // ── Closure: draw right-panel widgets (clipped) ──────────────────────
        let draw_right = |scene: &mut Scene, ctx: &mut ContextManager, hovered: &Option<String>| {
            scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &right_clip);
            let mut render = VelloGpuRenderContext::new(scene, 0.0, 0.0);
            let layer = LayerId::main();

            // ── 10. Tab strip ─────────────────────────────────────────────────
            let tab_labels = ["List", "Empty", "Sub-tabs"];
            for (i, lbl) in tab_labels.iter().enumerate() {
                let tab_rect = {
                    let tab_w = ((right_panel_width - 16.0) / 3.0).floor();
                    let tab_x = right_panel_x + 8.0 + i as f64 * (tab_w + 4.0);
                    Rect::new(tab_x, TAB_STRIP_Y, tab_w, TAB_STRIP_H)
                };
                let tab_id   = format!("tab-{i}");
                let tab_cfg  = TabConfig::new(tab_id.as_str(), *lbl)
                    .active_if(active_tab == i);
                let tab_view = TabView {
                    tab: &tab_cfg,
                    hovered: hovered.as_deref() == Some(tab_id.as_str()),
                    pressed: pressed_id_ref.as_deref() == Some(tab_id.as_str()),
                    close_btn_hovered: false,
                };
                uzor::app_context::builders::draw_tab(
                    ctx, &mut render,
                    tab_id.as_str(), tab_rect, None,
                    &layer,
                    &tab_view,
                    &TabSettings::default(),
                );
            }

            // ── 11. Scrollbar + rows (Tab 0 only) ─────────────────────────────
            if active_tab == 0 {
                let sb_track  = Rect::new(sb_x, SB_Y, SB_W, SB_H);
                let viewport_h = SB_H;
                let thumb_ratio = (viewport_h / CONTENT_H).clamp(0.0, 1.0);
                let thumb_h = (thumb_ratio * sb_track.height).max(30.0);
                let scroll_range = sb_track.height - thumb_h;
                let thumb_y = sb_track.y
                    + (scroll_off / (CONTENT_H - viewport_h).max(1.0)) * scroll_range;
                let sb_thumb = Rect::new(sb_x, thumb_y, SB_W, thumb_h);

                uzor::app_context::builders::draw_scrollbar(
                    ctx, &mut render,
                    "sb-track", "sb-thumb",
                    sb_track, sb_thumb,
                    5.0, &layer,
                    CONTENT_H, viewport_h, scroll_off,
                    &ScrollbarSettings::default(),
                );

                let content_x = right_panel_x + 8.0;
                let content_w = right_panel_width - SB_W - 20.0;
                let clip_top    = SB_Y;
                let clip_bottom = SB_Y + SB_H;

                // 20 rows, mixing uzor's bundled fonts + emoji + symbols.
                // Group by font family; emoji come from Noto Color Emoji,
                // box-drawing/symbols from Symbols Nerd Font / Noto Symbols 2.
                let row_labels = [
                    "★ Roboto regular",
                    "Sans-serif clean",
                    "→ arrow + ✓ check",
                    "Quick brown fox",
                    "✨ ★ ☀ ☂ ❤",
                    "fn main() { ... }",
                    "let x: u32 = 42;",
                    "if let Some(v) = opt",
                    "// monospace code",
                    "0xCAFE_BABE",
                    "PT Root UI light",
                    "вариативный шрифт",
                    "12345 67890",
                    "Кириллица OK",
                    "ƒ unicode glyphs",
                    "Bold Roboto bold",
                    "❗ Heads up ❗",
                    "✓ Done · 14 items",
                    "🌍 globe · 🌟 star",
                    "═══ end of list ═══",
                ];
                for row in 0..CONTENT_ROWS {
                    let row_y = SB_Y + row as f64 * ROW_H - scroll_off;
                    if row_y + ROW_H < clip_top || row_y > clip_bottom {
                        continue;
                    }
                    let row_rect = Rect::new(content_x, row_y, content_w, ROW_H - 2.0);
                    let row_id   = format!("row-{row}");
                    let row_label = row_labels[row].to_string();
                    let row_settings = match row {
                        0..=4   => ItemSettings::default().with_style(Box::new(RowStyleRoboto)),
                        5..=9   => ItemSettings::default().with_style(Box::new(RowStyleJetBrains)),
                        10..=14 => ItemSettings::default().with_style(Box::new(RowStylePtRoot)),
                        _       => ItemSettings::default().with_style(Box::new(RowStyleRobotoBold)),
                    };
                    uzor::app_context::builders::draw_item(
                        ctx, &mut render,
                        row_id.as_str(), row_rect, &layer,
                        WidgetState::Normal,
                        &ItemView {
                            label: Some(row_label.as_str()),
                            icon: None,
                            svg: None,
                        },
                        &row_settings,
                        &ItemRenderKind::Label,
                    );
                }
            }

            // ── Sub-tabs (Tab 2 only) ─────────────────────────────────────────
            if active_tab == 2 {
                let sub_labels = ["Alpha", "Beta", "Gamma"];
                for (i, lbl) in sub_labels.iter().enumerate() {
                    let sub_rect = Rect::new(
                        right_panel_x + 8.0,
                        CONTENT_START_Y + 8.0 + i as f64 * 36.0,
                        90.0, 30.0,
                    );
                    let sub_id  = format!("sub-tab-{i}");
                    let sub_cfg = TabConfig::new(sub_id.as_str(), *lbl)
                        .active_if(active_sub_tab == i);
                    let sub_view = TabView {
                        tab: &sub_cfg,
                        hovered: hovered.as_deref() == Some(sub_id.as_str()),
                        pressed: pressed_id_ref.as_deref() == Some(sub_id.as_str()),
                        close_btn_hovered: false,
                    };
                    uzor::app_context::builders::draw_tab(
                        ctx, &mut render,
                        sub_id.as_str(), sub_rect, None,
                        &layer,
                        &sub_view,
                        &TabSettings::default(),
                    );
                }
            }

            // ── Tab 1: SVG icon grid ──────────────────────────────────────────
            if active_tab == 1 {
                let icon_size = 64.0_f64;
                let gap = 16.0_f64;
                let total_w = icon_size * 2.0 + gap;
                let total_h = icon_size * 2.0 + gap;
                let content_cx = right_panel_x + right_panel_width / 2.0;
                let content_cy = CONTENT_START_Y + (WIN_H as f64 - CONTENT_START_Y - 12.0) / 2.0;
                let grid_x0 = content_cx - total_w / 2.0;
                let grid_y0 = content_cy - total_h / 2.0;

                let icons_and_colors: [(&str, &str); 4] = [
                    (SVG_CIRCLE,   "#2962ff"),
                    (SVG_SQUARE,   "#10b981"),
                    (SVG_TRIANGLE, "#f59e0b"),
                    (SVG_DIAMOND,  "#ef5350"),
                ];
                for (idx, (svg, color)) in icons_and_colors.iter().enumerate() {
                    let col = idx % 2;
                    let row = idx / 2;
                    let ix = grid_x0 + col as f64 * (icon_size + gap);
                    let iy = grid_y0 + row as f64 * (icon_size + gap);
                    draw_svg_icon(&mut render, svg, ix, iy, icon_size, icon_size, color);
                }
            }

            // ── Tab 2: sub-content icon ───────────────────────────────────────
            if active_tab == 2 {
                let sub_content_y = CONTENT_START_Y + 8.0 + 3.0 * 36.0 + 8.0;
                let sub_cx = right_panel_x + right_panel_width / 2.0;
                let icon_size = 64.0_f64;
                let ix = sub_cx - icon_size / 2.0;
                let iy = sub_content_y + 8.0;

                let sub_icon = match active_sub_tab {
                    0 => SVG_CIRCLE,
                    1 => SVG_TRIANGLE,
                    _ => SVG_DIAMOND,
                };
                let sub_color = match active_sub_tab {
                    0 => "#2962ff",
                    1 => "#f59e0b",
                    _ => "#ef5350",
                };
                draw_svg_icon(&mut render, sub_icon, ix, iy, icon_size, icon_size, sub_color);
            }

            drop(render);
            scene.pop_layer();
        };

        // ── widgets — z-order: growing panel drawn last (on top) ─────────────
        let (captured_char_positions, captured_input_result): (Vec<f64>, InputResult) =
            if splitter_drag_dx > 0.0 {
                // Left panel growing → draw right first, left on top
                draw_right(&mut self.scene, &mut self.ctx, &hovered);
                draw_left(&mut self.scene, &mut self.ctx, &hovered, &pressed)
            } else {
                // Right panel growing (or default) → draw left first, right on top
                let result = draw_left(&mut self.scene, &mut self.ctx, &hovered, &pressed);
                draw_right(&mut self.scene, &mut self.ctx, &hovered);
                result
            };

        // ── Drag handle (splitter) — on top of both panels, no clip ──────────
        {
            let mut render = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);
            let layer = LayerId::main();
            uzor::app_context::builders::draw_drag_handle(
                &mut self.ctx, &mut render,
                "drag-handle", dh_rect, &layer,
                &DragHandleView { rect: dh_rect },
                &DragHandleSettings::default(),
                &DragHandleRenderKind::GripDots,
            );
        }

        // ── Update text field geometry ────────────────────────────────────────
        self.ctx.input.text_fields_mut().update_field(
            &unsafe_widget_id(TEXT_FIELD_ID),
            (TI_RECT.x, TI_RECT.y, TI_RECT.width, TI_RECT.height),
            captured_char_positions,
        );
        self.last_input_result = captured_input_result;

        // ── status strip ──────────────────────────────────────────────────────
        let strip_color = if self.connected {
            Color::from_rgb8(0x18, 0x50, 0x28)
        } else {
            Color::from_rgb8(0x60, 0x20, 0x20)
        };
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, strip_color, None,
            &vello::kurbo::Rect::new(0.0, height as f64 - 0.0, width as f64, height as f64),
        );

        // ── end_frame / responses ─────────────────────────────────────────────
        let responses = self.ctx.end_frame();

        let viewport_h = SB_H;
        let thumb_h = Self::thumb_h();
        let scroll_range = SB_H - thumb_h;

        for (id, resp) in &responses {
            let id_str = id.as_str();

            if resp.scrolled && (id_str == "sb-track" || id_str == "sb-thumb") {
                let dy = resp.scroll_delta.1;
                self.scroll_off = (self.scroll_off + dy * 20.0)
                    .clamp(0.0, (CONTENT_H - viewport_h).max(0.0));
            }

            if resp.dragged {
                match id_str {
                    "slider-main" => {
                        let frac_delta = resp.drag_delta.0 / SLID_RECT.width;
                        self.slider_val = (self.slider_val + frac_delta * 100.0)
                            .clamp(0.0, 100.0);
                    }
                    "sb-thumb" | "sb-track" => {
                        if scroll_range > 0.0 {
                            let off_delta =
                                resp.drag_delta.1 / scroll_range * (CONTENT_H - viewport_h);
                            self.scroll_off = (self.scroll_off + off_delta)
                                .clamp(0.0, (CONTENT_H - viewport_h).max(0.0));
                        }
                    }
                    _ => {}
                }
            }
        }

        // ── GPU submit ─────────────────────────────────────────────────────────
        let dev = &self.render_cx.devices[self.surface.dev_id];
        let render_params = RenderParams {
            base_color: BG,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer
            .render_to_texture(
                &dev.device, &dev.queue,
                &self.scene, &self.surface.target_view,
                &render_params,
            )
            .unwrap_or_default();

        let surface_texture = match self.surface.surface.get_current_texture() {
            // wgpu 29: returns CurrentSurfaceTexture enum.
            vello::wgpu::CurrentSurfaceTexture::Success(t)
            | vello::wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return,
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = dev.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("level2-blit"),
        });
        self.surface.blitter.copy(
            &dev.device, &mut encoder,
            &self.surface.target_view, &surface_view,
        );
        dev.queue.submit([encoder.finish()]);
        surface_texture.present();
        self.window.request_redraw();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TabConfig helper — active_if builder method not in upstream; use a local fn
// ─────────────────────────────────────────────────────────────────────────────

trait TabConfigExt {
    fn active_if(self, cond: bool) -> Self;
}

impl TabConfigExt for TabConfig {
    fn active_if(mut self, cond: bool) -> Self {
        self.active = cond;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// winit handler
// ─────────────────────────────────────────────────────────────────────────────

struct Handler {
    state: Option<AppState>,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("uzor L2 — multi-widget demo")
            .with_inner_size(winit::dpi::LogicalSize::new(WIN_W, WIN_H))
            .with_resizable(false);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("OS should be able to create a desktop window"),
        );

        let mut render_cx = VelloRenderCx::new();
        let size = window.inner_size();

        let surface: RenderSurface<'static> = pollster::block_on(async {
            render_cx
                .create_surface(
                    Arc::clone(&window),
                    size.width.max(1),
                    size.height.max(1),
                    wgpu::PresentMode::AutoVsync,
                )
                .await
                .expect("vello should be able to create a GPU surface on desktop")
        });

        let renderer = Renderer::new(
            &render_cx.devices[surface.dev_id].device,
            RendererOptions {
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
                ..RendererOptions::default()
            },
        )
        .expect("vello Renderer creation should succeed");

        window.request_redraw();

        let ctx = ContextManager::new(LayoutNode::new("l2-root"));

        // Default right panel width: everything from x=336 to right edge
        let right_panel_width = WIN_W as f64 - 336.0 - 12.0;

        self.state = Some(AppState {
            window,
            render_cx,
            surface,
            renderer,
            scene: Scene::new(),
            ctx,
            start_time: Instant::now(),
            last_mouse_pos:      (0.0, 0.0),
            drag_origin:         None,
            drag_target:         None,
            connected:           false,
            checked:             true,
            toggled:             true,
            radio_sel:           1,
            slider_val:          40.0,
            range_min:           20.0,
            range_max:           80.0,
            range_drag_handle:   None,
            scroll_off:          0.0,
            swatch_sel:          0,
            active_tab:          0,
            active_sub_tab:      0,
            right_panel_width,
            splitter_drag_dx:    0.0,
            hovered_widget_id:   None,
            pressed_widget_id:   None,
            modifiers_shift:     false,
            modifiers_ctrl:      false,
            text_dragging:       false,
            last_input_result:   InputResult::default(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut app) = self.state else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => app.render(),

            // ── Mouse move ────────────────────────────────────────────────────
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x;
                let y = position.y;
                app.last_mouse_pos = (x, y);
                app.hovered_widget_id = app.compute_hovered((x, y));

                // Text drag — extend selection as mouse moves.
                if app.text_dragging {
                    app.ctx.input.text_fields_mut().on_drag_move(x);
                }

                // Drag in progress
                if let (Some((ox, oy)), Some(target)) =
                    (app.drag_origin, app.drag_target.as_ref())
                {
                    let dx = x - ox;
                    let dy = y - oy;
                    match target {
                        DragTarget::Slider(v0) => {
                            let frac_delta = dx / SLID_RECT.width;
                            app.slider_val = (v0 + frac_delta * 100.0).clamp(0.0, 100.0);
                        }
                        DragTarget::RangeMin(v0) => {
                            let frac_delta = dx / RANGE_RECT.width;
                            app.range_min = (v0 + frac_delta * 100.0)
                                .clamp(0.0, app.range_max);
                        }
                        DragTarget::RangeMax(v0) => {
                            let frac_delta = dx / RANGE_RECT.width;
                            app.range_max = (v0 + frac_delta * 100.0)
                                .clamp(app.range_min, 100.0);
                        }
                        DragTarget::Scroll(v0) => {
                            let scroll_range = SB_H - AppState::thumb_h();
                            if scroll_range > 0.0 {
                                let off_delta = dy / scroll_range * (CONTENT_H - SB_H);
                                app.scroll_off = (v0 + off_delta)
                                    .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                            }
                        }
                        // Fix 4: splitter drag resizes right panel
                        DragTarget::Splitter(w0) => {
                            // Dragging right = splitter moves right = left panel grows
                            app.right_panel_width = (w0 - dx).clamp(200.0, 600.0);
                            app.splitter_drag_dx = dx;
                        }
                    }
                }

                app.window.request_redraw();
            }

            // ── Left mouse down ───────────────────────────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let (x, y) = app.last_mouse_pos;
                app.pressed_widget_id = app.hovered_widget_id.clone();

                if app.hovered_widget_id.as_deref() == Some(TEXT_FIELD_ID) {
                    app.ctx.input.text_fields_mut().on_drag_start(x, y);
                    app.text_dragging = true;
                } else {
                    if app.text_focused() {
                        app.ctx.input.clear_focus();
                    }
                    app.text_dragging = false;
                }

                let target = match app.hovered_widget_id.as_deref() {
                    Some("slider-main") => Some(DragTarget::Slider(app.slider_val)),
                    Some("range-slider") => {
                        let x_min = RANGE_RECT.x + (app.range_min / 100.0) * RANGE_RECT.width;
                        let x_max = RANGE_RECT.x + (app.range_max / 100.0) * RANGE_RECT.width;
                        if (x - x_min).abs() <= (x - x_max).abs() {
                            app.range_drag_handle = Some(DualSliderHandle::Min);
                            Some(DragTarget::RangeMin(app.range_min))
                        } else {
                            app.range_drag_handle = Some(DualSliderHandle::Max);
                            Some(DragTarget::RangeMax(app.range_max))
                        }
                    }
                    Some("sb-thumb") | Some("sb-track") => Some(DragTarget::Scroll(app.scroll_off)),
                    // Fix 4: start splitter drag
                    Some("splitter") | Some("drag-handle") => {
                        Some(DragTarget::Splitter(app.right_panel_width))
                    }
                    _ => None,
                };
                app.drag_origin = Some((x, y));
                app.drag_target = target;

                app.window.request_redraw();
            }

            // ── Left mouse up ─────────────────────────────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let (x, y) = app.last_mouse_pos;

                if let Some(id) = app.ctx.input.process_click(x, y) {
                    match id.as_str() {
                        "btn-connect" => {
                            app.connected = !app.connected;
                            println!("[L2] connected → {}", app.connected);
                        }
                        "btn-close" => {
                            println!("[L2] close clicked");
                        }
                        "cb-setting-a" => {
                            app.checked = !app.checked;
                            println!("[L2] checked → {}", app.checked);
                        }
                        "tog-enable" => {
                            app.toggled = !app.toggled;
                            println!("[L2] toggled → {}", app.toggled);
                        }
                        TEXT_FIELD_ID => {
                            println!("[L2] text input focused");
                        }
                        s if s.starts_with("radio-opt-") => {
                            if let Ok(n) = s["radio-opt-".len()..].parse::<usize>() {
                                app.radio_sel = n;
                                println!("[L2] radio → {n}");
                            }
                        }
                        s if s.starts_with("swatch-") => {
                            if let Ok(n) = s["swatch-".len()..].parse::<usize>() {
                                app.swatch_sel = n;
                                println!("[L2] swatch → {n}");
                            }
                        }
                        // Fix 3: tab click switches active_tab
                        s if s.starts_with("tab-") => {
                            if let Ok(n) = s["tab-".len()..].parse::<usize>() {
                                app.active_tab = n;
                                app.scroll_off = 0.0;
                                println!("[L2] tab → {n}");
                            }
                        }
                        // Fix 3b: sub-tab click
                        s if s.starts_with("sub-tab-") => {
                            if let Ok(n) = s["sub-tab-".len()..].parse::<usize>() {
                                app.active_sub_tab = n;
                                println!("[L2] sub-tab → {n}");
                            }
                        }
                        _ => {}
                    }
                }

                if app.text_dragging {
                    app.ctx.input.text_fields_mut().on_drag_end();
                    app.text_dragging = false;
                }

                app.pressed_widget_id = None;
                app.drag_origin = None;
                app.drag_target = None;
                app.range_drag_handle = None;
                app.splitter_drag_dx = 0.0;
                app.window.request_redraw();
            }

            // ── Modifier keys ─────────────────────────────────────────────────
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                app.modifiers_shift = state.shift_key();
                app.modifiers_ctrl  = state.control_key();
            }

            // ── Scroll wheel ──────────────────────────────────────────────────
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx_lines, dy_lines) = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => (dx as f64, dy as f64),
                    MouseScrollDelta::PixelDelta(p) => (p.x / 20.0, p.y / 20.0),
                };
                let (mx, _my) = app.last_mouse_pos;

                if app.hovered_widget_id.as_deref() == Some("slider-main") {
                    let advance = if dx_lines.abs() > dy_lines.abs() { dx_lines } else { dy_lines };
                    app.slider_val = (app.slider_val + advance * 2.0).clamp(0.0, 100.0);
                    println!("[L2] slider_val → {:.1}", app.slider_val);
                } else if app.hovered_widget_id.as_deref() == Some("range-slider") {
                    let advance = if dx_lines.abs() > dy_lines.abs() { dx_lines } else { dy_lines };
                    let x_min = RANGE_RECT.x + (app.range_min / 100.0) * RANGE_RECT.width;
                    let x_max = RANGE_RECT.x + (app.range_max / 100.0) * RANGE_RECT.width;
                    if (mx - x_min).abs() <= (mx - x_max).abs() {
                        app.range_min = (app.range_min + advance * 2.0)
                            .clamp(0.0, app.range_max);
                    } else {
                        app.range_max = (app.range_max + advance * 2.0)
                            .clamp(app.range_min, 100.0);
                    }
                    println!("[L2] range → [{:.1}, {:.1}]", app.range_min, app.range_max);
                } else if mx >= app.right_panel_x() && app.active_tab == 0 {
                    // Scroll right panel (tab 0)
                    app.scroll_off = (app.scroll_off - dy_lines * 20.0)
                        .clamp(0.0, (CONTENT_H - SB_H).max(0.0));
                    println!("[L2] scroll_off → {:.1}", app.scroll_off);
                }
                app.window.request_redraw();
            }

            // ── Keyboard ──────────────────────────────────────────────────────
            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed =>
            {
                let text_id = unsafe_widget_id(TEXT_FIELD_ID);
                if app.ctx.input.text_fields().is_focused(&text_id) {
                    let shift = app.modifiers_shift;
                    let ctrl  = app.modifiers_ctrl;

                    if ctrl {
                        match ke.physical_key {
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyA) => {
                                app.ctx.input.on_key(KeyPress::SelectAll);
                                app.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyC) => {
                                if let Some(selected) = app.ctx.input.text_fields().copy_selection() {
                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                        let _ = cb.set_text(&selected);
                                    }
                                    println!("[L2] copy: {:?}", selected);
                                }
                                app.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyX) => {
                                if let Some(selected) = app.ctx.input.text_fields().copy_selection() {
                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                        let _ = cb.set_text(&selected);
                                    }
                                    println!("[L2] cut: {:?}", selected);
                                    app.ctx.input.on_key(KeyPress::Delete);
                                }
                                app.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(winit::keyboard::KeyCode::KeyV) => {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    if let Ok(text) = cb.get_text() {
                                        app.ctx.input.on_key(KeyPress::Paste(text.clone()));
                                        println!("[L2] paste: {:?}", text);
                                    }
                                }
                                app.window.request_redraw();
                                return;
                            }
                            _ => {}
                        }
                    }

                    match &ke.logical_key {
                        Key::Named(NamedKey::Backspace) => {
                            app.ctx.input.on_char('\x08');
                        }
                        Key::Named(NamedKey::Enter) => {
                            let committed = app.ctx.input.text_fields()
                                .text(&text_id)
                                .to_owned();
                            app.ctx.input.clear_focus();
                            println!("[L2] text committed: {:?}", committed);
                        }
                        Key::Named(NamedKey::Escape) => {
                            app.ctx.input.clear_focus();
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            if shift {
                                app.ctx.input.on_key(KeyPress::ShiftLeft);
                            } else {
                                app.ctx.input.on_key(KeyPress::ArrowLeft);
                            }
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            if shift {
                                app.ctx.input.on_key(KeyPress::ShiftRight);
                            } else {
                                app.ctx.input.on_key(KeyPress::ArrowRight);
                            }
                        }
                        Key::Named(NamedKey::Home) => {
                            if shift {
                                app.ctx.input.on_key(KeyPress::ShiftHome);
                            } else {
                                app.ctx.input.on_key(KeyPress::Home);
                            }
                        }
                        Key::Named(NamedKey::End) => {
                            if shift {
                                app.ctx.input.on_key(KeyPress::ShiftEnd);
                            } else {
                                app.ctx.input.on_key(KeyPress::End);
                            }
                        }
                        Key::Named(NamedKey::Delete) => {
                            app.ctx.input.on_key(KeyPress::Delete);
                        }
                        Key::Character(s) if !ctrl => {
                            for ch in s.chars() {
                                if !ch.is_control() {
                                    app.ctx.input.on_char(ch);
                                }
                            }
                        }
                        _ => {}
                    }
                    app.window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref app) = self.state {
            app.window.request_redraw();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut handler = Handler { state: None };
    event_loop.run_app(&mut handler)?;

    Ok(())
}
