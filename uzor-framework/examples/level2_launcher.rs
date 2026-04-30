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
//! cargo run --example level2_launcher -p uzor-framework
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
use uzor::types::{Rect, WidgetId, WidgetState};

// ── widgets ──────────────────────────────────────────────────────────────────
use uzor::ui::widgets::atomic::button::input::register_context_manager_button;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonTheme, ButtonView};

use uzor::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox;
use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

use uzor::ui::widgets::atomic::toggle::input::register_context_manager_toggle;
use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

use uzor::ui::widgets::atomic::radio::input::register_context_manager_radio;
use uzor::ui::widgets::atomic::radio::settings::RadioSettings;
use uzor::ui::widgets::atomic::radio::types::{
    DotShape, RadioDotView, RadioRenderKind,
};

use uzor::ui::widgets::atomic::slider::input::register_context_manager_slider;
use uzor::ui::widgets::atomic::slider::render::SliderView;
use uzor::ui::widgets::atomic::slider::settings::SliderSettings;
use uzor::ui::widgets::atomic::slider::types::{DualSliderHandle, SliderType};

use uzor::ui::widgets::atomic::scrollbar::input::register_context_manager_scrollbar;
use uzor::ui::widgets::atomic::scrollbar::settings::ScrollbarSettings;

use uzor::ui::widgets::atomic::separator::input::{
    register_context_manager_separator, SeparatorKind,
};
use uzor::ui::widgets::atomic::separator::render::SeparatorView;
use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};

use uzor::ui::widgets::atomic::text_input::render::{
    draw_input, draw_input_cursor, InputView,
};
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::types::InputType;

use uzor::ui::widgets::atomic::color_swatch::input::register_context_manager_color_swatch;
use uzor::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
use uzor::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};

use uzor::ui::widgets::atomic::shape_selector::input::register_context_manager_shape_selector;
use uzor::ui::widgets::atomic::shape_selector::settings::ShapeSelectorSettings;
use uzor::ui::widgets::atomic::shape_selector::types::ShapeSelectorRenderKind;

use uzor::ui::widgets::atomic::close_button::input::register_context_manager_close_button;
use uzor::ui::widgets::atomic::close_button::render::CloseButtonView;
use uzor::ui::widgets::atomic::close_button::settings::CloseButtonSettings;
use uzor::ui::widgets::atomic::close_button::types::CloseButtonRenderKind;

use uzor::ui::widgets::atomic::drag_handle::input::register_context_manager_drag_handle;
use uzor::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
use uzor::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};

use uzor::ui::widgets::atomic::text_input::render::InputResult;

// ── GPU render context ────────────────────────────────────────────────────────
use uzor_render_vello_gpu::VelloGpuRenderContext;

// ─────────────────────────────────────────────────────────────────────────────
// Geometry + colours
// ─────────────────────────────────────────────────────────────────────────────

const WIN_W: u32 = 680;
const WIN_H: u32 = 440;

const BG: Color = Color::from_rgb8(0x16, 0x16, 0x1e);
const PANEL_BG: Color = Color::from_rgb8(0x1e, 0x22, 0x2d);
const LABEL_BG: Color = Color::from_rgba8(255, 255, 255, 18);

// Text field widget ID — used to register with coordinator and query state.
const TEXT_FIELD_ID: &str = "text-search";

// Widget rects (constants for hover hit-test).
const BTN_RECT:   Rect = Rect { x: 28.0,  y: 28.0,  width: 130.0, height: 36.0 };
const CLOSE_RECT: Rect = Rect { x: 278.0, y: 28.0,  width: 24.0,  height: 24.0 };
const CB_RECT:    Rect = Rect { x: 28.0,  y: 88.0,  width: 160.0, height: 22.0 };
const TOG_RECT:   Rect = Rect { x: 28.0,  y: 130.0, width: 80.0,  height: 24.0 };
const SLID_RECT:  Rect = Rect { x: 28.0,  y: 200.0, width: 260.0, height: 24.0 };
const RANGE_RECT: Rect = Rect { x: 28.0,  y: 228.0, width: 260.0, height: 24.0 };
const TI_RECT:    Rect = Rect { x: 28.0,  y: 278.0, width: 200.0, height: 28.0 };
const DH_RECT:    Rect = Rect { x: 350.0, y: 28.0,  width: 60.0,  height: 24.0 };

// ─────────────────────────────────────────────────────────────────────────────
// Custom visible button theme (default has bg_normal=transparent)
// ─────────────────────────────────────────────────────────────────────────────

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
    Slider(f64),                              // start value (0-100)
    RangeMin(f64),                            // start range_min (0-100)
    RangeMax(f64),                            // start range_max (0-100)
    Scroll(f64),                              // start scroll_off
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
    shape_sel:           usize,
    drag_handle_hovered: bool,

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

// Scrollbar geometry constants — needed in both render and response handling.
const SB_X: f64 = 624.0;
const SB_Y: f64 = 28.0;
const SB_W: f64 = 10.0;
const SB_H: f64 = 380.0;
const CONTENT_H: f64 = 1000.0;

impl AppState {
    fn current_time(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    fn text_focused(&self) -> bool {
        self.ctx.input.text_fields().is_focused(&WidgetId::new(TEXT_FIELD_ID))
    }

    /// Compute which widget is under the pointer, given current mouse position.
    fn compute_hovered(mouse: (f64, f64)) -> Option<String> {
        let (mx, my) = mouse;
        if rect_contains(BTN_RECT,   mx, my) { return Some("btn-connect".into()); }
        if rect_contains(CLOSE_RECT, mx, my) { return Some("btn-close".into()); }
        if rect_contains(CB_RECT,    mx, my) { return Some("cb-setting-a".into()); }
        if rect_contains(TOG_RECT,   mx, my) { return Some("tog-enable".into()); }
        if rect_contains(SLID_RECT,  mx, my) { return Some("slider-main".into()); }
        if rect_contains(RANGE_RECT, mx, my) { return Some("range-slider".into()); }
        if rect_contains(TI_RECT,    mx, my) { return Some("text-search".into()); }
        if rect_contains(DH_RECT,    mx, my) { return Some("drag-handle".into()); }
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
        // Shape selector buttons
        for i in 0..3_usize {
            let shx = 28.0 + i as f64 * 80.0;
            if rect_contains(Rect::new(shx, 392.0, 72.0, 28.0), mx, my) {
                return Some(format!("shape-{i}"));
            }
        }
        // Scrollbar
        let sb_track = Rect::new(SB_X, SB_Y, SB_W, SB_H);
        if rect_contains(sb_track, mx, my) { return Some("sb-track".into()); }
        None
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

        // Build a minimal InputState from app-owned pointer position.
        // No EventProcessor needed — we just tell the coordinator where the
        // cursor is so begin_frame can update hover state internally.
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
        // Must be done AFTER begin_frame and BEFORE end_frame so the coordinator
        // knows about this widget during hit-testing.
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

        // column panels (visual grouping)
        for (px, pw) in [(12.0_f64, 310.0_f64), (336.0_f64, 330.0_f64)] {
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, PANEL_BG, None,
                &RoundedRect::new(px, 12.0, px + pw, height as f64 - 12.0, 8.0),
            );
        }

        // ── Snapshot text field state before the render borrow ───────────────
        let text_id = WidgetId::new(TEXT_FIELD_ID);
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


        // ── Snapshot hover/press before the render borrow (borrow checker) ────
        let hovered = self.hovered_widget_id.clone();
        let pressed = self.pressed_widget_id.clone();
        let ws = |id: &str| -> WidgetState {
            match (hovered.as_deref() == Some(id), pressed.as_deref() == Some(id)) {
                (_, true)     => WidgetState::Pressed,
                (true, false) => WidgetState::Hovered,
                _             => WidgetState::Normal,
            }
        };

        // ── widgets ───────────────────────────────────────────────────────────
        // The block returns (char_positions, input_result) for post-frame use.
        let (captured_char_positions, captured_input_result): (Vec<f64>, InputResult) = {
            let mut render = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);
            let layer = LayerId::main();

            // ── 1. Button ─────────────────────────────────────────────────────
            let btn_state = {
                let base = ws("btn-connect");
                if base == WidgetState::Normal && self.connected {
                    WidgetState::Active
                } else {
                    base
                }
            };
            let btn_view = ButtonView {
                text:          Some(if self.connected { "Disconnect" } else { "Connect" }),
                icon:          None,
                active:        self.connected,
                disabled:      false,
                active_border: None,
            };
            register_context_manager_button(
                &mut self.ctx, &mut render,
                "btn-connect", BTN_RECT, &layer,
                btn_state,
                &btn_view,
                &ButtonSettings::default().with_theme(Box::new(VisibleButtonTheme)),
            );

            // ── 2. Close button ───────────────────────────────────────────────
            register_context_manager_close_button(
                &mut self.ctx, &mut render,
                "btn-close", CLOSE_RECT, &layer,
                ws("btn-close"),
                &CloseButtonView { hovered: false },
                &CloseButtonSettings::default(),
                &CloseButtonRenderKind::Default,
            );

            // ── 3. Checkbox ───────────────────────────────────────────────────
            register_context_manager_checkbox(
                &mut self.ctx, &mut render,
                "cb-setting-a", CB_RECT, &layer,
                ws("cb-setting-a"),
                &CheckboxView { checked: self.checked, label: Some("Setting A") },
                &CheckboxSettings::default(),
                &CheckboxRenderKind::Standard,
                "13px sans-serif",
            );

            // ── 4. Toggle ─────────────────────────────────────────────────────
            register_context_manager_toggle(
                &mut self.ctx, &mut render,
                "tog-enable", TOG_RECT, &layer,
                ws("tog-enable"),
                &ToggleView { toggled: self.toggled, label: Some("ON"), disabled: false },
                &ToggleSettings::default(),
                &ToggleRenderKind::Switch,
            );

            // ── 5. Radio group (3 dots inline) ────────────────────────────────
            for (i, cx) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                let dot_rect = Rect::new(*cx, 175.0, 28.0, 28.0);
                let radio_id = format!("radio-opt-{i}");
                register_context_manager_radio(
                    &mut self.ctx, &mut render,
                    radio_id.as_str(), dot_rect, &layer,
                    ws(&radio_id),
                    &RadioSettings::default(),
                    &RadioRenderKind::Dot {
                        shape: DotShape::Circle,
                        cx: cx + 14.0,
                        cy: 175.0 + 14.0,
                        view: RadioDotView { selected: self.radio_sel == i },
                    },
                );
            }

            // ── 6. Slider (horizontal) ────────────────────────────────────────
            register_context_manager_slider(
                &mut self.ctx, &mut render,
                "slider-main", SLID_RECT, &layer,
                ws("slider-main"),
                &SliderView {
                    kind: SliderType::Single {
                        value: self.slider_val,
                        min: 0.0, max: 100.0, step: 1.0,
                    },
                    hovered: false,
                    disabled: false,
                    dragging_handle: None,
                },
                &SliderSettings::default(),
            );

            // ── 6b. Range slider (dual-handle) ────────────────────────────────
            register_context_manager_slider(
                &mut self.ctx, &mut render,
                "range-slider", RANGE_RECT, &layer,
                ws("range-slider"),
                &SliderView {
                    kind: SliderType::Dual {
                        min_value: self.range_min,
                        max_value: self.range_max,
                        min: 0.0, max: 100.0, step: 1.0,
                    },
                    hovered: hovered.as_deref() == Some("range-slider"),
                    disabled: false,
                    dragging_handle: self.range_drag_handle,
                },
                &SliderSettings::default(),
            );

            // ── 7. Separator (horizontal divider) ─────────────────────────────
            let sep_rect = Rect::new(28.0, 260.0, 260.0, 2.0);
            register_context_manager_separator(
                &mut self.ctx, &mut render,
                "sep-h", sep_rect, SeparatorKind::Divider, &layer,
                &SeparatorView {
                    kind: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal },
                    hovered: false,
                    dragging: false,
                },
                &SeparatorSettings::default(),
            );

            // ── 8. Text input ─────────────────────────────────────────────────
            // Use draw_input directly (single draw pass) to capture InputResult
            // for click→cursor positioning and cursor blink rendering.
            let ti_state = if text_is_focused {
                WidgetState::Active
            } else {
                ws(TEXT_FIELD_ID)
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
            // Draw the field (bg, border, selection highlight, text).
            let input_result = draw_input(&mut render, TI_RECT, ti_state, &ti_view, &ti_settings);
            // Blinking cursor — only when focused and in visible phase.
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
            // The block's return value carries char positions and InputResult
            // out of the render borrow scope for use after the block closes.
            let char_positions = input_result.char_x_positions.clone();
            let ir = input_result;

            // ── 9. Color swatches (4 squares) ─────────────────────────────────
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
                register_context_manager_color_swatch(
                    &mut self.ctx, &mut render,
                    sw_id.as_str(), sw_rect, &layer,
                    ws(&sw_id),
                    &ColorSwatchView {
                        color: *color,
                        hovered: false,
                        selected: self.swatch_sel == i,
                        show_transparency: false,
                        border_color_override: None,
                    },
                    &ColorSwatchSettings::default(),
                    &ColorSwatchRenderKind::Simple,
                );
            }

            // ── 10. Shape selector (3 buttons) ────────────────────────────────
            let shape_labels = ["Rect", "Circle", "Line"];
            for (i, _lbl) in shape_labels.iter().enumerate() {
                let shx = 28.0 + i as f64 * 80.0;
                let sh_rect = Rect::new(shx, 392.0, 72.0, 28.0);
                let sh_id = format!("shape-{i}");
                register_context_manager_shape_selector(
                    &mut self.ctx, &mut render,
                    sh_id.as_str(), sh_rect, &layer,
                    ws(&sh_id),
                    &ShapeSelectorSettings::default(),
                    &ShapeSelectorRenderKind::UIStyle,
                );
            }

            // ── 11. Scrollbar (vertical, right column) ────────────────────────
            let sb_track = Rect::new(SB_X, SB_Y, SB_W, SB_H);
            let viewport_h = SB_H;
            let thumb_ratio = (viewport_h / CONTENT_H).clamp(0.0, 1.0);
            let thumb_h = (thumb_ratio * sb_track.height).max(30.0);
            let scroll_range = sb_track.height - thumb_h;
            let thumb_y = sb_track.y
                + (self.scroll_off / (CONTENT_H - viewport_h).max(1.0)) * scroll_range;
            let sb_thumb = Rect::new(SB_X, thumb_y, SB_W, thumb_h);

            register_context_manager_scrollbar(
                &mut self.ctx, &mut render,
                "sb-track", "sb-thumb",
                sb_track, sb_thumb,
                5.0, &layer,
                CONTENT_H, viewport_h, self.scroll_off,
                &ScrollbarSettings::default(),
            );

            // ── 12. Drag handle ───────────────────────────────────────────────
            register_context_manager_drag_handle(
                &mut self.ctx, &mut render,
                "drag-handle", DH_RECT, &layer,
                &DragHandleView { rect: DH_RECT },
                &DragHandleSettings::default(),
                &DragHandleRenderKind::GripDots,
            );

            (char_positions, ir)
        };

        // ── Update text field geometry for click-to-position / drag-select ────
        // Must happen after the render block (borrow checker) but before any
        // mouse event processing that relies on char positions.
        self.ctx.input.text_fields_mut().update_field(
            &WidgetId::new(TEXT_FIELD_ID),
            (TI_RECT.x, TI_RECT.y, TI_RECT.width, TI_RECT.height),
            captured_char_positions,
        );
        self.last_input_result = captured_input_result;

        // ── shape label overlays (after render borrow released) ───────────────
        let shape_labels = ["Rect", "Circle", "Line"];
        for (i, lbl) in shape_labels.iter().enumerate() {
            let shx = 28.0 + i as f64 * 80.0;
            let lbl_w = lbl.len() as f64 * 6.0;
            let lbl_rect = vello::kurbo::Rect::new(
                shx + (72.0 - lbl_w) / 2.0,
                392.0 + 8.0,
                shx + (72.0 + lbl_w) / 2.0,
                392.0 + 20.0,
            );
            let lbl_alpha = if self.shape_sel == i { 180_u8 } else { 80_u8 };
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY,
                Color::from_rgba8(255, 255, 255, lbl_alpha),
                None, &lbl_rect,
            );
        }

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

        // widget-count badge
        for (i, label) in ["13 widgets", "L2 demo"].iter().enumerate() {
            let lw = label.len() as f64 * 6.5;
            let bx = 340.0 + i as f64 * 100.0;
            let by = height as f64 - 32.0;
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, LABEL_BG, None,
                &RoundedRect::new(bx, by, bx + lw + 12.0, by + 20.0, 4.0),
            );
        }

        // drag-handle hover indicator (visual: blue tint when hovered)
        if self.drag_handle_hovered {
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY,
                Color::from_rgba8(41, 98, 255, 40), None,
                &vello::kurbo::Rect::new(350.0, 28.0, 410.0, 52.0),
            );
        }

        // ── end_frame / responses ─────────────────────────────────────────────
        let responses = self.ctx.end_frame();

        // Scrollbar track geometry — needed for scroll + drag offset calculation
        let viewport_h = SB_H;
        let thumb_h = Self::thumb_h();
        let scroll_range = SB_H - thumb_h;

        for (id, resp) in &responses {
            let id_str = id.0.as_str();

            // ── hover tracking for drag-handle visual ─────────────────────────
            if id_str == "drag-handle" {
                self.drag_handle_hovered = resp.hovered;
            }

            // ── scroll handlers from WidgetResponse (coordinator-routed) ──────
            if resp.scrolled && (id_str == "sb-track" || id_str == "sb-thumb") {
                let dy = resp.scroll_delta.1;
                self.scroll_off = (self.scroll_off + dy * 20.0)
                    .clamp(0.0, CONTENT_H - viewport_h);
            }

            // ── drag handlers from WidgetResponse ─────────────────────────────
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
                            self.scroll_off =
                                (self.scroll_off + off_delta).clamp(0.0, CONTENT_H - viewport_h);
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
            Ok(t) => t,
            Err(_) => return,
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
            shape_sel:           0,
            drag_handle_hovered: false,
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
                app.hovered_widget_id = AppState::compute_hovered((x, y));

                // Text drag — extend selection as mouse moves.
                if app.text_dragging {
                    app.ctx.input.text_fields_mut().on_drag_move(x);
                }

                // Drag in progress — update slider or scrollbar value.
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
                                app.scroll_off =
                                    (v0 + off_delta).clamp(0.0, CONTENT_H - SB_H);
                            }
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

                // Focus text field if hovered, else clear focus.
                if app.hovered_widget_id.as_deref() == Some(TEXT_FIELD_ID) {
                    // on_drag_start focuses the field AND positions the cursor
                    // at the click x coordinate using last-frame char positions.
                    app.ctx.input.text_fields_mut().on_drag_start(x, y);
                    app.text_dragging = true;
                } else {
                    if app.text_focused() {
                        app.ctx.input.clear_focus();
                    }
                    app.text_dragging = false;
                }

                // Record drag origin; classify drag target by hovered widget.
                let target = match app.hovered_widget_id.as_deref() {
                    Some("slider-main") => Some(DragTarget::Slider(app.slider_val)),
                    Some("range-slider") => {
                        // Pick the closer handle.
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

                // process_click uses the coordinator's registered widget list
                // (built during the last render frame) to find what was clicked.
                if let Some(id) = app.ctx.input.process_click(x, y) {
                    match id.0.as_str() {
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
                        s if s.starts_with("shape-") => {
                            if let Ok(n) = s["shape-".len()..].parse::<usize>() {
                                app.shape_sel = n;
                                println!("[L2] shape → {n}");
                            }
                        }
                        _ => {}
                    }
                }

                // End text drag selection.
                if app.text_dragging {
                    app.ctx.input.text_fields_mut().on_drag_end();
                    app.text_dragging = false;
                }

                app.pressed_widget_id = None;
                app.drag_origin = None;
                app.drag_target = None;
                app.range_drag_handle = None;
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
                // Positive dy = scroll up (mlc pattern: LineDelta up = +1.0).
                let (dx_lines, dy_lines) = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => (dx as f64, dy as f64),
                    MouseScrollDelta::PixelDelta(p) => (p.x / 20.0, p.y / 20.0),
                };
                let (mx, _my) = app.last_mouse_pos;

                // Horizontal slider: wheel adjusts value (dy primary, dx fallback).
                // Scroll up (dy > 0) increases value; scroll down decreases.
                if app.hovered_widget_id.as_deref() == Some("slider-main") {
                    let advance = if dx_lines.abs() > dy_lines.abs() { dx_lines } else { dy_lines };
                    app.slider_val = (app.slider_val + advance * 2.0).clamp(0.0, 100.0);
                    println!("[L2] slider_val → {:.1}", app.slider_val);
                }
                // Range slider: wheel adjusts ONLY the handle closest to cursor.
                // Each handle moves independently — like mlc's two-point scroller.
                else if app.hovered_widget_id.as_deref() == Some("range-slider") {
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
                }
                // Right column: scroll the scrollbar.
                else if mx >= 336.0 {
                    // dy_lines positive = wheel up = scroll content up = decrease offset
                    app.scroll_off = (app.scroll_off - dy_lines * 20.0)
                        .clamp(0.0, CONTENT_H - SB_H);
                    println!("[L2] scroll_off → {:.1}", app.scroll_off);
                }
                app.window.request_redraw();
            }

            // ── Keyboard ──────────────────────────────────────────────────────
            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed =>
            {
                let text_id = WidgetId::new(TEXT_FIELD_ID);
                if app.ctx.input.text_fields().is_focused(&text_id) {
                    let shift = app.modifiers_shift;
                    let ctrl  = app.modifiers_ctrl;

                    // Ctrl shortcuts — use physical key for layout independence.
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
