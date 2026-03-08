# UZOR Core Library — Full Module Audit

**Date:** 2026-03-08
**Source path:** `uzor/uzor/src/`
**Architecture:** Headless, platform-agnostic UI engine. Geometry + interaction only; all rendering delegated to platform backends via the `RenderContext` trait.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [types/](#2-types)
3. [context.rs](#3-contextrs)
4. [input/](#4-input)
5. [render/](#5-render)
6. [animation/](#6-animation)
7. [panels/](#7-panels)
8. [panel_api/](#8-panel_api)
9. [widgets/](#9-widgets)
10. [layout/](#10-layout)
11. [layout_helpers/](#11-layout_helpers)
12. [containers/](#12-containers)
13. [state/](#13-state)
14. [platform/](#14-platform)
15. [macos/](#15-macos)
16. [interactive/](#16-interactive)
17. [text_fx/](#17-text_fx)
18. [cursor/](#18-cursor)
19. [numbers/](#19-numbers)
20. [scroll_fx/](#20-scroll_fx)
21. [Module Dependency Graph](#21-module-dependency-graph)
22. [Public Re-exports from lib.rs](#22-public-re-exports-from-librs)

---

## 1. Architecture Overview

Uzor is a **headless UI engine**. It computes:
- Geometry: where widgets are (via `layout/`)
- Interaction: what the user is doing (via `input/`)
- Animation state: how things should be animating (via `animation/`)
- Persistent state: scroll positions, focus, per-widget data (via `state/`)

It does NOT render. Rendering is the caller's responsibility via `RenderContext`.

**Frame lifecycle:**
```
1. Platform delivers events → InputState snapshot
2. Context::begin_frame(input, viewport) → runs animations, computes layout
3. Caller queries Context for rects, hover/click state → renders accordingly
4. InputCoordinator::begin_frame → registers widgets → end_frame → returns WidgetResponse list
```

**Central struct:** `Context` (in `context.rs`) owns `InputState`, `LayoutTree`, `StateRegistry`, and `AnimationCoordinator`.

---

## 2. `types/`

**Purpose:** Fundamental types used across all other modules. No dependencies on other uzor modules.

### Submodules

#### `types/rect.rs` — `Rect`

Core rectangle type for all geometry.

```rust
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
pub type WidgetRect = Rect;  // compatibility alias
```

Key methods:
- `new(x, y, width, height)` — constructor
- `min_x()`, `min_y()`, `max_x()`, `max_y()` — edge accessors
- `right()`, `bottom()` — opposite edges
- `center_x()`, `center_y()` — center point
- `contains(x, y) -> bool` — point-in-rect test
- `inset(padding) -> Rect` — shrink by uniform padding
- `intersect(other) -> Rect` — intersection (returns zero-size if no overlap)
- `split_horizontal(left_width) -> (Rect, Rect)` — split into left/right
- `split_vertical(top_height) -> (Rect, Rect)` — split into top/bottom

Derives: `Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq`

#### `types/state.rs` — Interaction state types

All widget interaction tracking types live here.

**`WidgetState`** (enum) — coarse per-widget visual state:
- `Normal`, `Hovered`, `Pressed`, `Active`, `Toggled`, `Disabled`
- `is_hovered()`, `is_pressed()`, `is_disabled()` helpers

**`WidgetId`** (struct) — string-based unique identifier:
- `WidgetId(pub String)`
- Implements `From<&str>`, `From<String>`, `Hash`, `Eq`

**`FocusState`** — manages keyboard focus:
- `focused: Option<WidgetId>`, `pending_focus: Option<WidgetId>`
- `set_focus()`, `clear_focus()`, `is_focused()`, `request_focus()`, `process_pending()`

**`HoverState`** — tracks which widget is hovered and mouse position:
- `hovered: Option<WidgetId>`, `mouse_pos: (f64, f64)`, `mouse_pressed: bool`

**`DragState`** — active drag tracking:
- `dragging: Option<WidgetId>`, `start_pos`, `current_pos`, `offset`, `initial_value`
- `start()`, `start_with_value()`, `update()`, `end()`, `delta()`, `delta_from()`

**`ScrollState`** — per-container scroll state (embed in modal/widget structs):
- `offset: f64`, `velocity: f64`, `content_size: f64`
- `handle_wheel(delta_y, content_height, viewport_height) -> bool`
- `handle_drag(y, track_height, content_height, viewport_height)`
- `handle_track_click(...)`, `clamp()`, `reset()`, `start_drag()`, `end_drag()`

**`WidgetInteraction`** (enum): `None, Hover, Press, Drag, Click, DoubleClick, Focus`

**`WidgetData`** — per-widget data storage:
- `value: f64, text: String, cursor_pos, selection_start, checked, open, selected_index, scroll_offset`

**`WidgetInputState`** — combined hover + focus + drag + per-widget data store. Owns a `HashMap<WidgetId, WidgetData>` for per-widget mutable data.

#### `types/icon.rs` — `IconId`

```rust
pub struct IconId(pub String);
```
String wrapper for icon asset references. Implements `From<&str>`, `Display`.

---

## 3. `context.rs`

**Purpose:** The primary entry point for the uzor immediate-mode API. Manages the frame lifecycle, connects input → layout → state → animation.

### `ButtonResponse`

Returned by `Context::button()`:
```rust
pub struct ButtonResponse {
    pub clicked: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub state: WidgetState,
    pub rect: Rect,
}
```

### `Context`

```rust
pub struct Context {
    pub input: InputState,
    pub layout: LayoutTree,
    pub registry: StateRegistry,
    pub animations: AnimationCoordinator,
    pub time: f64,
}
```

**Constructor:** `Context::new(root_node: LayoutNode) -> Self`

**Frame lifecycle:**
- `begin_frame(input: InputState, viewport: Rect)` — updates animations, recomputes layout
- `state::<T>(id) -> &mut T` — access typed persistent state for a widget (uses `StateRegistry`)
- `widget_rect(id) -> Rect` — get computed rect from layout

**Immediate-mode widget API (interaction detection only, no rendering):**
- `button(id) -> ButtonResponse`
- `checkbox(id, checked) -> CheckboxResponse`
- `scroll_area(id, content_height) -> (Rect, ScrollState)` — with momentum physics (velocity, 90% decay per frame)
- `icon_button(id) -> IconButtonResponse`

**Dependencies:** `animation::AnimationCoordinator`, `input::InputState`, `layout::tree::LayoutTree`, `state::StateRegistry`, `types::*`, `widgets`

---

## 4. `input/`

**Purpose:** Platform-agnostic input state and routing. The most critical module for interactivity — converts raw platform events into per-widget responses.

### Submodules

#### `input/state.rs` — Raw input snapshot

**`MouseButton`** (enum): `Left, Right, Middle`

**`ModifierKeys`** (struct): `shift, ctrl, alt, meta` booleans
- `any()`, `ctrl_shift()`, `ctrl_alt()`, `command()` (platform-aware: uses `meta` on macOS, `ctrl` elsewhere)

**`PointerState`** (struct):
- `pos: Option<(f64, f64)>`, `button_down: Option<MouseButton>`, `clicked: Option<MouseButton>`
- `double_clicked`, `triple_clicked`, `prev_pos`
- `delta() -> (f64, f64)`, `is_present() -> bool`

**`DragState`** (struct): `start, current, delta, total_delta, button, initial_value`
- `new()`, `update(x, y)`, `delta_tuple()`

**`InputState`** (struct) — the single input snapshot per frame:
```rust
pub struct InputState {
    pub pointer: PointerState,
    pub modifiers: ModifierKeys,
    pub scroll_delta: (f64, f64),
    pub drag: Option<DragState>,
    pub dt: f64,
    pub time: f64,
    pub multi_touch: Option<TouchState>,
}
```
Key methods: `is_hovered(&Rect)`, `is_clicked()`, `is_double_clicked()`, `is_right_clicked()`, `is_mouse_down()`, `is_dragging()`, `drag_delta()`, `shift()`, `ctrl()`, `alt()`, `consume_click()`, `consume_scroll()`, `end_frame()`

#### `input/events.rs` — `KeyCode`

Platform-agnostic keyboard key codes enum covering letters (A-Z), numbers (0-9), function keys (F1-F12), navigation (arrows, Home, End, PgUp/Dn), editing (Backspace, Delete, Enter, Tab, Space, Escape), symbols (Plus, Minus, brackets).

#### `input/sense.rs` — `Sense`

Bitflag-style struct declaring what interactions a widget responds to.

```rust
pub struct Sense { pub click: bool, pub drag: bool, pub hover: bool, pub focus: bool }
```

Predefined constants: `NONE, HOVER, CLICK, DRAG, CLICK_AND_DRAG, FOCUSABLE, ALL`

Builder: `Sense::click()`, `Sense::drag()`, `Sense::hover()`, `Sense::focusable()`, `Sense::all()`

Composition: `union()`, `intersection()`, `with_click()`, `with_drag()`, `with_focus()`, implements `BitOr` and `BitOrAssign`

Query: `interactive()`, `has_click_and_drag()`, `is_passive()`

#### `input/response.rs` — `WidgetResponse`

Unified response returned by all interactive widgets:
```rust
pub struct WidgetResponse {
    pub id: WidgetId, pub rect: WidgetRect, pub sense: Sense,
    // Hover
    pub hovered: bool, pub hover_started: bool, pub hover_ended: bool,
    // Click
    pub clicked: bool, pub double_clicked: bool, pub triple_clicked: bool,
    pub right_clicked: bool, pub middle_clicked: bool,
    // Drag
    pub drag_started: bool, pub dragged: bool, pub drag_stopped: bool,
    pub drag_delta: (f64, f64), pub drag_total: (f64, f64),
    // Focus
    pub has_focus: bool, pub gained_focus: bool, pub lost_focus: bool,
    // Value
    pub changed: bool, pub enabled: bool,
}
```

Builder: `with_hover()`, `with_click()`, `with_focus()`, `with_changed()`, `disabled()`

Query: `any_click()`, `interacted()`, `is_active()`, `is_pointer_over()`

Composition: `union()`, implements `BitOr` (combines two responses with OR semantics)

Free function: `create_response(id, rect, sense, input, prev_hovered, prev_focused) -> WidgetResponse`

#### `input/coordinator.rs` — `InputCoordinator`, `LayerId`, `ScopedRegion`

**The central input router.** Manages widget registration, Z-order layering, hit testing, and event dispatch.

**`LayerId`** — string-based layer identifier. Predefined layers:
- `LayerId::main()` → z=0 (default)
- `LayerId::modal()` → blocks lower layers
- `LayerId::popup()` → above modal
- `LayerId::tooltip()` → highest

**`ScopedRegion`** — a bounded screen region with its own child `InputCoordinator`:
- `rect: Rect` — bounding rect in screen coordinates
- `coordinator: InputCoordinator` — child coordinator for widgets inside
- `id: String` — region identifier
- `contains(x, y)`, `to_local(x, y)` — coordinate conversion
- `prefix_id(widget_id)` — returns `"{region_id}:{widget_id}"`

**`InputCoordinator`** — the main event router:

Holds: `widgets: Vec<RegisteredWidget>`, `layers: Vec<Layer>`, `widget_state: WidgetInputState`, `input: InputState`, `scoped_regions: Vec<ScopedRegion>`

Key methods:
- `new()` — creates with main layer pre-registered
- `begin_frame(input: InputState)` — clears widget list, propagates input to scoped regions with coordinate conversion (sets `pointer.pos = None` for regions the cursor is outside)
- `register(id, rect, sense)` — register widget on main layer
- `register_on_layer(id, rect, sense, layer)` — register on specific layer
- `push_layer(id, z_order, modal)` — add Z layer (modal=true blocks lower layers)
- `pop_layer(id)` — no-op (layers live until next `begin_frame`)
- `push_scoped_region(id, rect) -> &mut InputCoordinator` — register/update a scoped region, returns child coordinator (reuses existing region if same id)
- `remove_scoped_region(id)`, `scoped_region(id)`, `scoped_region_coordinator_mut(id)`
- `end_frame() -> Vec<(WidgetId, WidgetResponse)>` — processes all widgets:
  1. Collects scoped region responses (prefixed IDs)
  2. Z-order-aware hit test finds hovered widget
  3. Generates hover_started/hover_ended/clicked/drag_started responses
  4. Handles ongoing drag (dragged / drag_stopped)
  5. Updates persistent hover state
- `process_click(x, y) -> Option<WidgetId>` — Z-order + modal + scoped-region aware click routing
- `is_hovered(id)`, `is_focused(id)`, `is_dragging(id)` — widget state queries
- `hovered_widget()`, `focused_widget()` — current focus/hover
- `hovered_widget_z_order()`, `hovered_widget_layer_id()` — layer info for hovered widget
- `is_over_ui() -> bool` — true if cursor is over any registered widget (chart canvas is NOT registered, so this distinguishes UI vs chart)
- `is_point_in_modal_layer(x, y) -> bool` — for "click outside to close" logic
- `is_blocked_by_modal(x, y) -> bool` — for drag blocking during modals
- `topmost_modal_layer()` — returns topmost modal LayerId
- `focus_next()`, `focus_prev()` — Tab/Shift+Tab keyboard navigation
- `set_focus(id)`, `clear_focus()`
- `widget_rect(id) -> Option<Rect>` — rect of registered widget from last frame

**Hit testing algorithm:** Sorts layers by z_order descending, finds last-registered widget in topmost layer containing the point. If a layer is `modal=true`, stops searching lower layers on miss.

#### Other `input/` submodules

- `input/animation.rs` — animation state tied to input events (e.g., hover-triggered animations)
- `input/cursor.rs` — cursor style tracking
- `input/event_processor.rs` — `EventProcessor` for converting platform events to InputState
- `input/handlers.rs` — handler utilities
- `input/shortcuts.rs` — keyboard shortcut definitions
- `input/tooltip.rs` — tooltip state
- `input/touch.rs` — `TouchState` for multi-touch
- `input/widget_state.rs` — re-exports from `types/state.rs` (`WidgetId`, `FocusState`, `HoverState`, `DragState`, `ScrollState`, `WidgetInputState`)

---

## 5. `render/`

**Purpose:** Defines the `RenderContext` trait — the single interface between uzor and any rendering backend. Also provides SVG icon rendering and a serializable render operation list.

### `render/context.rs` — `RenderContext` trait

The core rendering abstraction. Backends implement this. ~80 required/default methods across categories:

**Dimensions:**
- `dpr() -> f64` — device pixel ratio

**Stroke style:**
- `set_stroke_color(&str)`, `set_stroke_width(f64)`, `set_line_dash(&[f64])`, `set_line_cap(&str)`, `set_line_join(&str)`

**Fill style:**
- `set_fill_color(&str)`, `set_fill_color_alpha(&str, f64)` (default impl), `set_global_alpha(f64)`, `reset_alpha()` (default)

**Path operations:**
- `begin_path()`, `move_to()`, `line_to()`, `close_path()`, `rect()`, `arc()`, `ellipse()`, `quadratic_curve_to()`, `bezier_curve_to()`

**Stroke/fill:**
- `stroke()`, `fill()`, `clip()`, `clip_rect()` (default), `stroke_rect()`, `fill_rect()`
- `fill_rounded_rect()` (default), `stroke_rounded_rect()` (default), `rounded_rect()` (default — uses arc sequence)

**Text:**
- `set_font(&str)` (CSS-style: "14px sans-serif"), `set_text_align(TextAlign)`, `set_text_baseline(TextBaseline)`
- `fill_text(&str, x, y)`, `stroke_text(&str, x, y)`, `measure_text(&str) -> f64`
- `fill_text_rotated()` (default: save/translate/rotate/fill), `fill_text_centered()` (default)

**Transforms:**
- `save()`, `restore()`, `translate()`, `rotate()`, `scale()`

**Images:**
- `draw_image(image_id, x, y, width, height) -> bool` (default: returns false)
- `draw_image_rgba(data, img_width, img_height, x, y, width, height)` (default: no-op)

**Blur background (FrostedGlass/LiquidGlass):**
- `draw_blur_background(x, y, width, height)` (default: no-op)
- `has_blur_background() -> bool` (default: false)
- `use_convex_glass_buttons() -> bool` (default: false)

**UI state rendering (hover/active with glass effects):**
- `draw_hover_rect(x, y, width, height, color)` — dispatches to convex glass / flat glass / solid
- `draw_active_rect(x, y, width, height, color)` — same
- `draw_hover_rounded_rect(x, y, width, height, radius, color)` — rounded variant
- `draw_active_rounded_rect(x, y, width, height, radius, color)` — rounded variant
- `draw_sidebar_hover_item(x, y, width, height, accent_color, bg_color, indicator_width)` — accent bar + hover bg
- `draw_sidebar_active_item(x, y, width, height, accent_color, bg_color, indicator_width)` — accent bar + active bg
- `draw_glass_button_3d(x, y, width, height, radius, is_active, color)` (default: draw_blur + fill_rounded_rect)

### `render/context.rs` — `RenderContextExt` trait

Extension for type-safe blur image management:
```rust
pub trait RenderContextExt: RenderContext {
    type BlurImage: Clone;
    fn set_blur_image(&mut self, image: Option<Self::BlurImage>, width: u32, height: u32) {}
    fn set_use_convex_glass_buttons(&mut self, use_convex: bool) {}
}
```

### `render/types.rs`

```rust
pub enum TextAlign { Left, Center, Right }
pub enum TextBaseline { Top, Middle, Bottom, Alphabetic }
```

### `render/ops.rs` — `RenderOp`, `RenderOps`, `execute_ops`

Serializable render instruction list — enables recording and playback of draw calls.

```rust
pub enum RenderOp {
    SetStrokeColor(String), SetFillColor(String), SetLineWidth(f64), SetLineDash(Vec<f64>),
    BeginPath, MoveTo(f64, f64), LineTo(f64, f64), QuadraticCurveTo(...), BezierCurveTo(...),
    Arc(...), Ellipse(...), ClosePath, Stroke, Fill, StrokeRect(...), FillRect(...),
    SetFont(String), SetTextAlign(TextAlign), FillText(String, f64, f64), StrokeText(...),
    Save, Restore, Translate(f64, f64), Rotate(f64), Scale(f64, f64), Clip,
}
pub type RenderOps = Vec<RenderOp>;
pub fn execute_ops(ctx: &mut dyn RenderContext, ops: &[RenderOp])
```

### `render/svg.rs` — SVG icon renderer

`draw_svg_icon(ctx, svg, x, y, width, height, color)` — parses an SVG string and renders it scaled to the given rect. Supports `path`, `circle`, `rect`, `line`, `polyline`, `polygon` elements. Respects `fill="none"` on root SVG for stroke-only rendering.

`draw_svg_icon_rotated(ctx, svg, x, y, width, height, color, angle)` — same but with rotation.

### `render/helpers.rs` — Pixel-crisp helpers

`crisp(x: f64) -> f64` — rounds to nearest 0.5 for crisp 1px lines
`crisp_rect(x, y, w, h) -> (f64, f64, f64, f64)` — applies crisp to all four values

### `render/icons/` — SVG icon catalog

Domain-specific SVG icon constants organized by category:
- `aviation/` — jet, aircraft, helicopter, drone icons
- `maritime/` — ship, vessel icons
- `markers/` — map marker icons
- `weather/` — weather condition icons
- `infrastructure/` — building, facility icons
- `military/` — military asset icons

All icons are raw SVG strings passed to `draw_svg_icon`.

---

## 6. `animation/`

**Purpose:** Complete animation engine — spring physics, easing functions, keyframe timelines, decay, color interpolation, scroll physics, stagger, blending, and a central coordinator. All rendering-agnostic.

### Core primitives

#### `animation/spring.rs` — `Spring`

Analytical solution (closed-form) for damped harmonic oscillator. No numerical integration — frame-rate independent and no drift.

```rust
pub struct Spring {
    pub stiffness: f64,  // default 100.0
    pub damping: f64,    // default 10.0
    pub mass: f64,       // default 1.0
    pub initial_velocity: f64,
    pub rest_threshold: f64,  // default 0.001
}
```

Key methods:
- `new()`, builder: `.stiffness()`, `.damping()`, `.mass()`, `.initial_velocity()`, `.rest_threshold()`
- `damping_ratio() -> f64` — ζ = damping / (2√(stiffness × mass))
- `angular_frequency() -> f64` — ω₀ = √(stiffness / mass)
- `evaluate(t: f64) -> (position, velocity)` — position is displacement from target (1.0 at start → 0.0 at rest)
- `is_at_rest(t) -> bool`
- `estimated_duration() -> f64`
- `as_easing(samples) -> Vec<f64>` — converts spring curve to easing lookup table

Presets: `Spring::gentle()` (stiffness=120, damping=14), `Spring::bouncy()` (180/12), `Spring::stiff()` (300/20), `Spring::slow()` (60/14)

Three damping regimes: under-damped (oscillates), critically damped (fastest no-overshoot), over-damped (slow approach).

#### `animation/easing.rs` — `Easing`

All 30 Robert Penner easing equations plus CSS cubic-bezier and steps():

```rust
pub enum Easing {
    Linear,
    EaseInQuad, EaseOutQuad, EaseInOutQuad,
    EaseInCubic, EaseOutCubic, EaseInOutCubic,
    EaseInQuart, EaseOutQuart, EaseInOutQuart,
    EaseInQuint, EaseOutQuint, EaseInOutQuint,
    EaseInSine, EaseOutSine, EaseInOutSine,
    EaseInExpo, EaseOutExpo, EaseInOutExpo,
    EaseInCirc, EaseOutCirc, EaseInOutCirc,
    EaseInBack, EaseOutBack, EaseInOutBack,
    EaseInElastic, EaseOutElastic, EaseInOutElastic,
    EaseInBounce, EaseOutBounce, EaseInOutBounce,
    CubicBezier(f64, f64, f64, f64),  // CSS cubic-bezier(x1,y1,x2,y2)
    Steps(u32, StepPosition),          // CSS steps()
}
```

- `ease(t: f64) -> f64` — evaluate at t ∈ [0,1]
- `ease_f32(t: f32) -> f32` — convenience wrapper
- CSS constants: `Easing::EASE`, `EASE_IN`, `EASE_OUT`, `EASE_IN_OUT`
- CubicBezier uses Newton-Raphson with bisection fallback (Firefox/Chrome compatible)

#### `animation/decay.rs` — `Decay`

Exponential velocity decay for flick/momentum scrolling.

#### `animation/types.rs` — `AnimationKey`, `AnimationDriver`, `ActiveAnimation`

Supporting types for the coordinator:

```rust
pub struct AnimationKey { pub widget_id: WidgetId, pub property: String }
pub enum AnimationDriver { Tween { from, to, start_time, duration, easing }, Spring { spring, start_time, target }, Decay { decay, start_time, initial_value } }
pub struct ActiveAnimation { pub driver: AnimationDriver, pub current_value: f64, pub completed: bool }
```

`ActiveAnimation::update(time_secs)` — advances the animation based on driver type.

#### `animation/coordinator.rs` — `AnimationCoordinator`

Central animation manager. Keyed by `(WidgetId, property_name)`.

```rust
pub struct AnimationCoordinator {
    active: HashMap<AnimationKey, ActiveAnimation>,
    default_interruption: InterruptionStrategy,
}
```

Key methods:
- `update(time_secs: f64) -> bool` — tick all animations, clean up completed, returns true if any still active
- `get(widget_id, property) -> Option<f64>` — current animated value
- `get_or(widget_id, property, default) -> f64` — with fallback
- `tween(widget_id, property, from, to, duration_secs, easing, time_secs)` — start tween
- `spring(widget_id, property, spring, target, time_secs)` — start spring
- `decay(widget_id, property, decay, initial_value, time_secs)` — start decay
- `cancel_widget(widget_id)` — cancel all animations for widget
- `cancel(widget_id, property)` — cancel specific property
- `has_active() -> bool`, `is_animating(widget_id) -> bool`, `active_count() -> usize`
- `set_interruption_strategy(strategy)` — currently only `Instant` (replace) is implemented; Blend/InheritVelocity/Queue are TODO

### Other animation submodules

- `animation/timeline.rs` — `Timeline`, `Tween`, `Animatable`, `TimelinePlayback`, `Position` — keyframe-based animation timeline
- `animation/blend.rs` — `AnimationLayer`, `AnimationSlot`, `AnimationTransition`, `blend()`, `blend_weighted()`, `resolve_layers()`, `CompositeMode`, `InterruptionStrategy`
- `animation/color.rs` — `Color`, `ColorSpace`, `Oklab`, `Oklch` — perceptual color interpolation
- `animation/layers.rs` — `LayerStack`, `ManagedLayer` — animation layer management
- `animation/path.rs` — `MotionPath`, `PathSample`, `PathSegment`, `Point` — motion path animations
- `animation/scroll.rs` — `ScrollTimeline`, `ViewTimeline`, `ScrollTween`, `ParallaxLayer` — scroll-linked animations
- `animation/stagger.rs` — `LinearStagger`, `GridStagger`, `StaggerOrigin`, `DistanceMetric`, `GridOrigin` — staggered entrance/exit animations
- `animation/stroke.rs` — `StrokeAnimation`, `StrokeState` — animated stroke draw-on effect
- `animation/recipes/` — pre-built animation recipes for specific widget types:
  - `buttons/` — hover, press, tap animations for buttons
  - `charts/` — chart-specific animations
  - `lists/` — list item enter/exit stagger
  - `loading/` — loading/spinner animations
  - `modals/` — modal open/close transitions
  - `scroll/` — scroll momentum effects
  - `toasts/` — toast notification animations
  - `transitions/` — page/view transition animations
  Each recipe folder contains: `types.rs` (config/state types), `builders.rs` (constructors), `presets.rs` (named presets), `defaults.rs` (default values)

---

## 7. `panels/`

**Purpose:** N-ary docking panel layout engine with tabs, splits, grids, drag-and-drop, separator resize, floating windows, and layout presets. Rendering-agnostic — only geometry, hit-testing, and layout.

### Key trait

```rust
pub trait DockPanel: Clone + Send + Sync {
    fn title(&self) -> &str;
    fn type_id(&self) -> &'static str;
    fn min_size(&self) -> (f32, f32) { (200.0, 200.0) }
    fn closable(&self) -> bool { true }
}
```
Consumers implement this for domain-specific panel types.

### Submodules

#### `panels/id.rs`
- `LeafId(u64)` — unique leaf panel identifier
- `BranchId(u64)` — unique branch identifier
- `NodeId` — enum of LeafId or BranchId

#### `panels/rect.rs` — `PanelRect`

f32-based rectangle (distinct from `types::Rect` which is f64). Has `ZERO` constant, `contains()`, `intersects()`.

#### `panels/tree.rs` — `PanelTree`, `PanelStore`, `Tile`, `Container`, etc.

Core tree data types. A panel tree is an N-ary tree where:
- **Leaves** hold one or more panels (multiple = tabs)
- **Branches** hold children in linear or grid layouts

Key types exported: `PanelTree`, `PanelStore`, `Tile`, `Container`, `Tabs`, `Linear`, `Grid`, `Shares`, `LinearDirection`, `GridLayout`

#### `panels/grid.rs` — `DockingTree<P>`, `Leaf<P>`, `Branch<P>`, `PanelNode<P>`

The main panel tree implementation:

```rust
pub struct Leaf<P> { pub id: LeafId, pub panels: Vec<P>, pub active_tab: usize, pub hidden: bool }
pub struct Branch<P> { pub id: u64, pub children: Vec<PanelNode<P>>, pub direction: SplitDirection, pub shares: Vec<f32> }
pub enum PanelNode<P> { Leaf(Leaf<P>), Branch(Branch<P>) }
```

`DockingTree<P>` methods: `new()`, `with_single_leaf()`, `add_leaf()`, `add_tab()`, `remove_leaf()`, `remove_tab()`, `leaf()`, `leaf_mut()`, `root()`, `active_leaf_id()`, `set_active_leaf()`, `visible_leaf_count()`, `move_leaf_to_branch()`, `move_leaf_to_root_split()`, `compute_child_rects()` (static — layout algorithm)

#### `panels/manager.rs` — `DockingManager<P>`

The orchestration layer. Bridges tree data structures with layout computation, separator generation, drag-and-drop, and floating windows.

```rust
pub struct DockingManager<P: DockPanel> {
    tree: DockingTree<P>,
    separators: Vec<Separator>,
    panel_rects: HashMap<LeafId, PanelRect>,
    panel_headers: HashMap<LeafId, PanelRect>,
    tab_bars: Vec<TabBarInfo>,
    corners: Vec<CornerHandle>,
    layout_area: PanelRect,
    window_edge_rects: Option<[PanelRect; 4]>,
    panel_drag: Option<PanelDragState>,
    tab_reorder: Option<TabReorderState>,
    snap_animations: Vec<SnapBackAnimation>,
    floating_windows: Vec<FloatingWindow<P>>,
    floating_drag: Option<FloatingDragState>,
    next_floating_id: u64,
    hovered_header: Option<LeafId>,
    active_leaf: Option<LeafId>,
    header_height: f32,   // default 24.0
}
```

Construction: `new()`, `from_tree(tree)`, `with_panel(panel)`

**Layout:**
- `layout(area: PanelRect)` — recomputes all rects, separators, tab bars, corners. Walks tree recursively. Multi-tab leaves get tab bars; single-tab leaves get header rects.

**Hit testing:**
- `hit_test(x, y) -> HitResult` — priority: corners > separators > panels > None
- `update_separator_hover(x, y) -> bool` — updates hover state on all separators
- `hovered_separator_orientation() -> Option<SeparatorOrientation>`

**Panel drag-and-drop:**
- `start_panel_drag(leaf_id, x, y)` — begins drag from header
- `update_panel_drag(x, y)` — updates target detection: headers → tab bars → window edges → panel body (with drop zone algorithm: center/left/right/top/bottom based on 20% inset)
- `end_panel_drag(area_width, area_height) -> Option<FloatingWindowId>` — commits drop or floats leaf if no target
- `cancel_panel_drag()`
- `panel_drag_state() -> Option<&PanelDragState>`

**Tab management:**
- `set_active_tab(container_id, tab_id)`, `close_tab(container_id, tab_id)`
- `start_tab_reorder(container_id, tab_id, x)`, `update_tab_reorder(x)`, `end_tab_reorder()`

**Floating windows:**
- `float_leaf(leaf_id, x, y, area_width, area_height) -> Option<FloatingWindowId>` — extract leaf from tree
- `dock_floating(fw_id, target_id, zone, is_window_edge)` — re-insert floating window
- `close_floating(fw_id)`
- `start_floating_drag()`, `update_floating_drag()`, `update_floating_dock_target()`, `end_floating_drag()`
- `hit_test_floating_header()`, `hit_test_floating_body()`, `hit_test_floating_close()`

**Snap-back:** `update_snap_animations(dt)` — updates springback animations for separators that violated constraints.

**Window edges:** `compute_window_edge_rects()` — computes 4 drop indicator rects [top, bottom, left, right] at window midpoints.

**Accessors:** `tree()`, `tree_mut()`, `separators()`, `panel_rects()`, `panel_headers()`, `tab_bars()`, `corners()`, `floating_windows()`, `snap_animations()`, `active_leaf()`, `set_active_leaf()`, `layout_area()`, `window_edge_rects()`, `hovered_header()`, `set_hovered_header()`, `tab_reorder_state()`, `floating_drag_state()`

#### Other panels submodules

- `panels/separator.rs` — `Separator` (position, orientation, state, level), `SeparatorOrientation` (Horizontal/Vertical), `SeparatorState` (Idle/Hover/Dragging), `SeparatorLevel` (Node { parent_id, child_a, child_b }), `SeparatorController`
- `panels/drag.rs` — `DragDropState`, `LockState`, `DragSource`, `HoverTarget`, `PanelDragState`
- `panels/drop_zone.rs` — `DropZone` (Center/Up/Down/Left/Right), `DropZoneDetector`, `CompassZone`
- `panels/tabs.rs` — `TabBar`, `TabInfo`, `TabHit`, `TabDragController`, `TabDragState`, `TabReorderState`, `TabBarInfo`, `TabItem`
- `panels/floating.rs` — `FloatingWindow<P>` (id, panels, active_tab, x, y, width, height), `FloatingWindowId`, `FloatingDragState`
- `panels/hit_test.rs` — `HitResult` (Panel/Separator/Corner/None), `CornerHandle` (v_separator_idx, h_separator_idx, x, y)
- `panels/presets.rs` — `WindowLayout`, `SplitKind`, `PANEL_GAP` constant
- `panels/layout.rs` — layout computation helpers
- `panels/snap_back.rs` — `SnapBackAnimation` for separator constraint violations
- `panels/serialize.rs` — `LayoutSnapshot`, `SerializedNode`, `SerializedNodeType` — serialization/deserialization of panel layouts

---

## 8. `panel_api/`

**Purpose:** Contract between the terminal orchestrator and autonomous panel crates. Each panel crate (chart, map, trading-panels) implements `PanelApp` to become self-contained.

### `panel_api/traits.rs` — `PanelApp` trait, `ToolbarPosition`

```rust
pub enum ToolbarPosition { Top, Left, Right, Bottom }

pub trait PanelApp {
    fn title(&self) -> &str;
    fn type_id(&self) -> &'static str;
    fn min_size(&self) -> (f64, f64) { (200.0, 200.0) }
    fn toolbar_def(&self) -> Option<PanelToolbarDef> { None }
    fn toolbar_position(&self) -> ToolbarPosition { ToolbarPosition::Top }
    fn render_toolbar(&self, ctx: &mut dyn RenderContext, rect: PanelRect, theme: &PanelTheme, input: &PanelInput) -> Vec<HitZone> { vec![] }
    fn render_content(&mut self, ctx: &mut dyn RenderContext, rect: PanelRect, input: &PanelInput) {}
    fn as_any_mut(&mut self) -> &mut dyn Any { panic!(...) }
    fn handle_toolbar_click(&mut self, item_id: &str) -> Option<String> { None }
    fn handle_dropdown_select(&mut self, dropdown_id: &str, item_id: &str) -> Option<String> { None }
    fn supports_toolbar_grouping(&self) -> bool { false }
}
```

Rendering model: terminal carves out toolbar space based on `toolbar_def().size`, translates context to panel origin (0,0 = top-left), calls `render_toolbar` then `render_content`.

Lifecycle:
```
Terminal creates panel → panel.toolbar_def() → allocate toolbar space
Each frame:
  panel.render_toolbar(ctx, toolbar_rect, theme, input)
  panel.render_content(ctx, content_rect, input)
On click:
  panel.handle_toolbar_click(item_id)
```

### `panel_api/types.rs`

- `PanelRect` — f64-based rectangle for panel API (self-contained, mirrors `types::Rect`)
- `HitZone { id: String, rect: PanelRect }` — returned by toolbar rendering for click dispatch
- `MouseButton` (enum: Left/Right/Middle)
- `PanelInput` — input snapshot per frame including mouse position (panel-local and screen), click state, scroll delta, time, dpr
- `PanelOutput` — optional output: toolbar hit zones, content hit zones, cursor style, toolbar height
- `PanelTheme` — theme colors: toolbar_bg, separator, hover/active bg/text, accent, sidebar_style

### `panel_api/toolbar.rs`

`PanelToolbarDef` — describes what toolbar a panel wants (size, items, layout).

---

## 9. `widgets/`

**Purpose:** Platform-agnostic widget definitions — types, state, themes, and input logic. 9 top-level widget categories plus auxiliary widgets.

### Major widget categories (each has a 5-file structure)

Each major widget category follows the pattern:
- `types.rs` — type enum (e.g., `ButtonType`)
- `state.rs` — per-widget persistent state
- `theme.rs` — visual parameters (colors, sizes, radii)
- `input.rs` — interaction logic
- `defaults.rs` — default configurations

#### `widgets/button/` — `ButtonType`

6 button types with 19 variants, covering 141 buttons in the application:

```rust
pub enum ButtonType {
    Action { variant: ActionVariant, position, width, height }
    Toggle { variant: ToggleVariant, position, width, height }
    Checkbox { variant: CheckboxVariant, position, width, height }
    Tab { variant: TabVariant, position, width, height }
    ColorSwatch { variant: ColorSwatchVariant, position, width, height }
    Dropdown { variant: DropdownVariant, position, width, height }
}
```

**ActionVariant:** `IconOnly`, `Text`, `IconText`, `LineText` (line width preview), `CheckboxText`

**ButtonStyle:** `Default`, `Primary` (filled accent), `Danger` (red hover), `Ghost` (no border)

**ToggleVariant:** `IconSwap` (icon changes, no bg), `Switch` (iOS oval), `ButtonToggle` (full bg toggle)

**CheckboxVariant:** `Standard`, `Cross`, `Circle`

**TabVariant:** `Vertical` (left bar indicator), `Horizontal` (bottom underline)

**ColorSwatchVariant:** `Square` (most common — 15 buttons), `IconWithBar`, `SwatchWithLabel`

**DropdownVariant:** `TextChevron` (main pattern — body cycles + chevron opens), `Text`, `IconTextChevron`, `IconChevron`, `ChevronOnly`

#### `widgets/container/` — `ContainerType`

Scrollable and plain containers.

#### `widgets/popup/` — `PopupType`

Context menus, color pickers, custom popups. Has `defaults.rs` for preset popup configurations.

#### `widgets/panel/` — `PanelType`, `ToolbarVariant`, `SidebarVariant`, `ModalVariant`

Large container panels:
- `ToolbarVariant` — horizontal/vertical toolbars
- `SidebarVariant` — collapsible sidebars
- `ModalVariant` — modal dialog variants

#### `widgets/overlay/` — `OverlayType`

Tooltips and info overlays.

#### `widgets/text_input/` — `TextInputType`

Text, Number, Search, Password inputs. Has `behavior.rs` for text editing logic (cursor, selection).

#### `widgets/dropdown/` — `DropdownType`

Standard, Grid, Layout dropdowns.

#### `widgets/slider/` — `SliderType`

Single and dual-point sliders.

#### `widgets/toast/` — `ToastType`

Info, Success, Warning, Error notification toasts.

### Auxiliary widgets

- `widgets/checkbox.rs` — `CheckboxResponse { toggled, new_checked, hovered, state, rect }`
- `widgets/radio_group.rs` — `RadioGroup`, `RadioGroupState`, `RadioItem` — single-selection radio groups
- `widgets/icon_button.rs` — `IconButtonConfig`, `IconButtonResponse { clicked, hovered, state }` — icon-only toolbar buttons
- `widgets/context_menu.rs` — `ContextMenu`, `ContextMenuItem`, `ContextMenuState` — right-click context menus
- `widgets/input.rs` — `TextInput`, `TextInputState`, `TextInputConfig` — raw text input with cursor/selection
- `widgets/scrollbar.rs` — `Scrollbar`, `ScrollbarState` — custom scrollbar with drag
- `widgets/scrollable.rs` — `Scrollable`, `ScrollableState` — scrollable container with automatic scrollbar
- `widgets/toolbar.rs` — `Toolbar`, `ToolbarItem`, `ToolbarLayout` — toolbar containers
- `widgets/slider_system.rs` — `SliderSystem`, `SliderHandle` — slider system utilities

---

## 10. `layout/`

**Purpose:** CSS-inspired flexbox layout engine. Computes absolute pixel rects for all widgets from a declarative tree of layout nodes.

### `layout/types.rs`

All layout type definitions:

**`Display`** (enum): `Flex` (default), `Stack` (z-overlay), `Grid`, `None`

**`FlexDirection`** (enum): `Row` (default), `Column`

**`AlignItems`** (enum): `Stretch` (default), `Start`, `End`, `Center`

**`JustifyContent`** (enum): `Start` (default), `End`, `Center`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly`

**`Position`** (enum): `Relative` (default), `Absolute` (relative to parent content box)

**`SizeSpec`** (enum): `Fix(f64)`, `Pct(f64)` (0.0-1.0 of parent), `Fill` (flex grow), `Content` (default)

**`Insets`** (struct): `top, right, bottom, left: f64` — for padding and margin
- `all(val)`, `symmetric(v, h)`, `width()`, `height()`

**`LayoutStyle`** (struct): `display, direction, align_items, justify_content, position, gap, padding, margin, width, height, min/max_width/height, offset_x, offset_y, z_index`

**`LayoutKind`** (enum): `Container` (default), `Widget`, `Overlay`

**`LayoutNode`** (struct): `id: WidgetId, kind, style: LayoutStyle, children: Vec<LayoutNode>, flags: LayoutFlags`

Builder: `new(id)`, `with_style()`, `with_child()`, `with_children()`, `with_kind()`, `with_flags()`

**`LayoutFlags`** (bitfield): `NONE, CLIP_CONTENT, SCROLL_Y, SCROLL_X, IS_ROOT`

**`LayoutComputed`** (struct): `rect: WidgetRect, content_rect: WidgetRect, clip_rect: Option<WidgetRect>, z_order: i32`

### `layout/tree.rs` — `LayoutTree`

```rust
pub struct LayoutTree {
    pub root: LayoutNode,
    pub computed: HashMap<WidgetId, LayoutComputed>,
}
```

- `new(root_node)`, `compute(viewport: Rect)` — walks tree, computes all rects
- `get_rect(id) -> Option<Rect>`, `get_computed(id) -> Option<&LayoutComputed>`

**Layout algorithm:**
1. Apply margin to get border box
2. Determine actual size (parent did flex math)
3. Compute content rect (minus padding)
4. Handle clipping (CLIP_CONTENT flag intersects with parent clip)
5. Track z-order
6. Recurse into children via Flex or Stack algorithm

**Flex algorithm:** Fixed/percentage/fill children. Counts fill children, distributes remaining space equally. Respects gaps. Absolute-positioned children are placed in parent content rect.

**Stack algorithm:** All children get full content rect, z-index increments by registration order.

---

## 11. `layout_helpers/`

**Purpose:** Level 2 layout utilities — calculate widget positions without any rendering. Use these for quick manual layout without a full `LayoutTree`.

### `layout_helpers/helpers/` — Free functions

- `center_rect(container: Rect, width, height) -> Rect` — center a rect within container
- `align_left(container, width, height) -> Rect`
- `align_right(container, width, height) -> Rect`
- `align_top(container, width, height) -> Rect`
- `align_bottom(container, width, height) -> Rect`
- `stack_vertical(rects: &[Rect], gap) -> Vec<Rect>` — vertical stack with gap
- `stack_horizontal(rects: &[Rect], gap) -> Vec<Rect>` — horizontal stack with gap
- `grid_layout(container, columns, item_width, item_height, gap) -> Vec<Rect>` — grid layout
- `distribute_space(container, count, direction, gap) -> Vec<Rect>` — evenly distribute
- `aspect_ratio(container, ratio) -> Rect` — fit while preserving aspect ratio
- `fit_in_bounds(container, width, height) -> Rect` — fit maintaining aspect ratio
- `modal_rect(viewport, width, height) -> Rect` — center modal in viewport

**Dependencies:** `types::rect::Rect`

---

## 12. `containers/`

**Purpose:** Layout container primitives — flex, stack, and scroll containers. These are higher-level abstractions over the raw layout engine.

### Submodules

- `containers/flex.rs` — `FlexContainer` — horizontal/vertical flexbox container with gap, alignment, padding
- `containers/stack.rs` — `StackContainer` — z-axis stack (all children overlapping)
- `containers/scroll.rs` — `ScrollContainer` — scrollable container with overflow clipping and scrollbar integration

All containers compute child rects (using `layout/` internally) and track scroll state.

---

## 13. `state/`

**Purpose:** Persistent behavioral state store. Stores typed state (scroll offsets, focus, any per-widget data) across frames using `Any` type erasure.

### `state/registry.rs` — `StateRegistry`

```rust
pub struct StateRegistry {
    states: HashMap<WidgetId, Box<dyn Any + Send + Sync>>,
}
```

Key methods:
- `get::<T>(id) -> Option<&T>` — immutable typed access
- `get_or_insert_with::<T, F>(id, default) -> &mut T` — get or create default
- `insert::<T>(id, state)` — explicit insert/update
- `remove(id)` — remove widget state
- `clear()` — remove all state

Used by `Context::state::<T>(id)` — the immediate-mode API for accessing persistent widget state.

**Dependencies:** `types::state::WidgetId`

---

## 14. `platform/`

**Purpose:** Platform abstraction layer. Defines traits backends (desktop/web/mobile) must implement.

### `platform/mod.rs` — `WindowConfig`, `PlatformEvent`, `ImeEvent`, `SystemTheme`

**`WindowConfig`**: `title, width, height, resizable, decorations, transparent, visible`

**`PlatformEvent`** (enum) — all platform events:
- Window: `WindowCreated, WindowResized, WindowMoved, WindowFocused, WindowCloseRequested, WindowDestroyed, RedrawRequested`
- Pointer: `PointerEntered, PointerLeft, PointerMoved, PointerDown, PointerUp`
- Touch: `TouchStart, TouchMove, TouchEnd, TouchCancel`
- Input: `Scroll, KeyDown, KeyUp, TextInput, ModifiersChanged, ClipboardPaste`
- File: `FileDropped, FileHovered, FileCancelled`
- System: `Ime(ImeEvent), ThemeChanged, ScaleFactorChanged`

**`ImeEvent`**: `Enabled, Preedit(String, Option<(usize, usize)>), Commit(String), Disabled`

**`SystemTheme`**: `Light, Dark`

### `platform/backends.rs` — `PlatformBackend` trait, `MockPlatform`

```rust
pub trait PlatformBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError>;
    fn close_window(&mut self, id: WindowId) -> Result<(), PlatformError>;
    fn primary_window(&self) -> Option<WindowId>;
    fn poll_events(&mut self) -> Vec<PlatformEvent>;
    fn request_redraw(&self, id: WindowId);
}
```

**`MockPlatform`** — headless implementation for testing. Implements `PlatformBackend` + `SystemIntegration` (clipboard, system theme). `push_event()` injects events.

### `platform/types.rs`

`PlatformError`, `WindowId` (UUID-based), `SystemIntegration` trait (clipboard read/write, system theme query).

---

## 15. `macos/`

**Purpose:** macOS Ventura/Sonoma pixel-perfect styling. Provides colors, typography, widget renderers, animations, icons, effects, and presets — all as data (no rendering calls; rendering is via `RenderContext`).

### Submodules

#### `macos/colors/` — `AppearanceMode`, `WidgetState`, `ColorPalette`

8 appearance modes: `Light, Dark, VibrantLight, VibrantDark, AccessibleLight, AccessibleDark, AccessibleVibrantLight, AccessibleVibrantDark`

`ColorPalette` — 70+ semantic color tokens as `&'static str` (hex): labels, text, content, menu, table, controls, windows, system accent colors (blue/brown/gray/green/indigo/orange/pink/purple/red/teal/yellow), fills, shadows.

`palette(mode) -> &'static ColorPalette` — resolves to light or dark palette.

Static instances: `colors::LIGHT`, `colors::DARK`

#### `macos/typography/` — Typography scale

12-level type scale (caption → display).

#### `macos/themes/` — Widget theme parameters

Per-widget theme definitions:
- `themes/button.rs` — button colors for each state × appearance
- `themes/checkbox.rs`, `themes/dialog.rs`, `themes/input.rs`, `themes/menu.rs`
- `themes/progress.rs`, `themes/radio.rs`, `themes/switch_toggle.rs`, `themes/tabs.rs`, `themes/traffic_lights.rs`

#### `macos/widgets/` — Widget render data

Render geometry/state structures (not actual drawing):
- `widgets/button.rs`, `widgets/checkbox.rs`, `widgets/dialog.rs`, `widgets/input.rs`, `widgets/menu.rs`, `widgets/progress.rs`, `widgets/radio.rs`, `widgets/switch_toggle.rs`, `widgets/tabs.rs`, `widgets/traffic_lights.rs`

#### `macos/animations/` — macOS animation presets

- `animations/button.rs` — button press spring
- `animations/dock.rs` — dock magnification animation
- `animations/menu.rs` — menu slide/fade
- `animations/modal.rs` — modal sheet spring
- `animations/switch_toggle.rs` — toggle switch spring
- `animations/traffic_lights.rs` — traffic light hover animation

#### `macos/icons/` — macOS SF Symbols-style icon paths

`icons/paths.rs` — SVG path data for macOS-style icons.

#### `macos/effects/` — Visual effects data

- `effects/shadows.rs` — multi-layer shadow parameters
- `effects/gradients.rs` — gradient approximation parameters

#### `macos/presets/`

- `presets/ventura.rs` — `VenturaPreset` — complete Ventura/Sonoma appearance preset bundling all colors, typography, and widget parameters.

---

## 16. `interactive/`

**Purpose:** Animation state management for interactive UI components. Computes animation state only — rendering is left to the UI layer.

### `ElasticSlider` (`interactive/elastic_slider.rs`)

Slider with elastic overflow (can pull past min/max) and spring snap-back.

```rust
pub struct ElasticSlider { /* spring params, overflow, value, range */ }
pub enum OverflowRegion { None, Min, Max }
```
Methods: `new()`, `update(mouse_x, track_x, track_width, dt)`, `value()`, `normalized()`, `overflow_region()`, `is_animating()`

### `AnimatedList` (`interactive/animated_list.rs`)

Staggered entry/exit animations for list items.

```rust
pub struct AnimatedList { /* stagger delay, spring params, items */ }
pub struct ItemState { /* opacity, y_offset, scale */ }
```
Methods: `add_item()`, `remove_item()`, `update(dt)`, `item_state(index)`, `is_animating()`

### `SpotlightCard` (`interactive/spotlight.rs`)

Cursor-following spotlight highlight effect on cards.

```rust
pub struct SpotlightCard { /* radius, intensity */ }
pub struct SpotlightColor { /* gradient stops */ }
```
Methods: `update(cursor_x, cursor_y, card_rect)`, `gradient_center()`, `gradient_intensity()`

### `ElectricBorder` (`interactive/electric_border.rs`)

Animated electric/lightning border effect.

Methods: `update(dt)`, `border_segments() -> Vec<(f64, f64)>`, `glow_intensity()`

---

## 17. `text_fx/`

**Purpose:** Rendering-agnostic text animation effects. Each effect computes animation state (positions, opacities, colors, offsets) that a renderer uses.

### `ShinyText` (`text_fx/shiny.rs`)

Metallic shine sweep across text:

```rust
pub struct ShinyTextConfig { pub speed: f64, pub highlight_color: String, pub base_color: String }
pub struct ShinyTextState { pub highlight_position: f64, pub time: f64 }
```

### `DecryptedText` (`text_fx/decrypt.rs`)

Scramble/reveal effect with configurable direction:

```rust
pub struct DecryptedTextConfig { pub speed: f64, pub characters: String }
pub struct DecryptedTextState { pub revealed_chars: Vec<char>, pub progress: f64 }
pub enum RevealDirection { LeftToRight, RightToLeft, Random, Sequential }
```

### `GradientText` (`text_fx/gradient.rs`)

Animated multi-color gradient sweep:

```rust
pub struct GradientTextConfig { pub colors: Vec<String>, pub speed: f64, pub direction: GradientDirection }
pub struct GradientTextState { pub color_stops: Vec<(f64, String)>, pub offset: f64 }
pub enum GradientDirection { Horizontal, Vertical, Diagonal }
```

### `FuzzyText` (`text_fx/fuzzy.rs`)

Scanline-style character displacement:

```rust
pub struct FuzzyTextConfig { pub amplitude: f64, pub frequency: f64, pub direction: FuzzyDirection }
pub struct FuzzyTextState { pub char_offsets: Vec<(f64, f64)>, pub time: f64 }
pub enum FuzzyDirection { Horizontal, Vertical, Both }
```

---

## 18. `cursor/`

**Purpose:** Cursor interaction effects. Rendering-agnostic state computation for custom cursor effects.

### `Magnet` (`cursor/magnet.rs`)

Elements attracted to cursor within a radius:

```rust
pub struct Magnet { pub radius: f64, pub strength: f64 }
pub struct MagnetState { pub offset_x: f64, pub offset_y: f64 }
```
`update(cursor_x, cursor_y, element_x, element_y)` — computes attraction offset.

### `ClickSpark` (`cursor/click_spark.rs`)

Particle burst on click:

```rust
pub struct ClickSpark { pub particle_count: u32, pub duration: f64, pub colors: Vec<String> }
pub struct ClickSparkState { pub particles: Vec<Particle>, pub active: bool }
pub struct Particle { pub x: f64, pub y: f64, pub vx: f64, pub vy: f64, pub opacity: f64, pub color: String }
```
`trigger(x, y)` — starts burst. `update(dt)` — advances particles.

### `BlobCursor` (`cursor/blob_cursor.rs`)

Trailing blob cursor with gooey merge effect:

```rust
pub struct BlobCursor { pub size: f64, pub speed: f64, pub gooey_factor: f64 }
pub struct BlobCursorState { pub blobs: Vec<BlobState>, pub cursor_pos: (f64, f64) }
pub struct BlobState { pub x: f64, pub y: f64, pub size: f64, pub opacity: f64 }
```

### `GlareHover` (`cursor/glare_hover.rs`)

Shiny glare sweep on hover:

```rust
pub struct GlareHover { pub intensity: f64, pub color: String }
pub struct GlareHoverState { pub glare_x: f64, pub glare_y: f64, pub opacity: f64 }
```
`update(cursor_x, cursor_y, element_rect)`.

---

## 19. `numbers/`

**Purpose:** Animated number display state. Rendering-agnostic — computes positions, opacities, and digit values.

### `Counter` (`numbers/counter.rs`)

Rolling slot-machine style digit display with spring physics:

```rust
pub struct Counter { pub spring: Spring, pub digit_height: f64 }
pub struct CounterState { pub digits: Vec<DigitState>, pub target: i64 }
pub struct DigitState { pub value: u8, pub y_offset: f64, pub opacity: f64 }
pub enum PlaceValue { Units, Tens, Hundreds, /* ... */ }
```

### `CountUp` (`numbers/count_up.rs`)

Spring-animated number counting from start to end:

```rust
pub struct CountUp { pub spring: Spring, pub start: f64, pub end: f64 }
pub struct CountUpState { pub current: f64, pub velocity: f64, pub completed: bool }
pub enum Direction { Up, Down }
```

---

## 20. `scroll_fx/`

**Purpose:** Scroll-linked animation effects. All rendering-agnostic state computation.

### `ScrollReveal` (`scroll_fx/scroll_reveal.rs`)

Word-by-word text reveal with opacity, blur, and rotation as content scrolls into view:

```rust
pub struct ScrollRevealConfig { pub blur: f64, pub rotate: f64, pub threshold: f64 }
pub struct ScrollReveal { /* config, words */ }
pub struct WordState { pub opacity: f64, pub blur: f64, pub y_offset: f64, pub rotation: f64 }
```
`update(scroll_y, viewport_height)` — computes per-word state.

### `ScrollVelocity` (`scroll_fx/scroll_velocity.rs`)

Infinite horizontal scroll with velocity-based speed (marquee with physics):

```rust
pub struct ScrollVelocityConfig { pub base_speed: f64, pub velocity_factor: f64 }
pub struct ScrollVelocity { /* config, position, velocity */ }
```
`update(scroll_delta, dt)` — `position()` returns current scroll offset.

### `ScrollFloat` (`scroll_fx/scroll_float.rs`)

Parallax character float effect:

```rust
pub struct ScrollFloatConfig { pub amplitude: f64, pub frequency: f64, pub phase_offset: f64 }
pub struct ScrollFloat { /* config, characters */ }
pub struct CharState { pub x: f64, pub y: f64, pub opacity: f64 }
```

---

## 21. Module Dependency Graph

```
lib.rs (crate root)
│
├── types/                    (no uzor dependencies)
│   ├── rect.rs
│   ├── state.rs
│   └── icon.rs
│
├── platform/                 (depends on: input/events, input/state)
│   ├── mod.rs (WindowConfig, PlatformEvent, ImeEvent, SystemTheme)
│   ├── backends.rs (PlatformBackend, MockPlatform)
│   └── types.rs (PlatformError, WindowId, SystemIntegration)
│
├── render/                   (depends on: types [Rect via helpers])
│   ├── context.rs (RenderContext trait, RenderContextExt)
│   ├── types.rs (TextAlign, TextBaseline)
│   ├── helpers.rs (crisp)
│   ├── ops.rs (RenderOp, execute_ops)
│   ├── svg.rs (draw_svg_icon)
│   └── icons/ (SVG catalogs — no deps)
│
├── input/                    (depends on: types, platform)
│   ├── state.rs (InputState, MouseButton, ModifierKeys, PointerState, DragState)
│   ├── events.rs (KeyCode)
│   ├── sense.rs (Sense)
│   ├── response.rs (WidgetResponse, create_response)
│   ├── coordinator.rs (InputCoordinator, LayerId, ScopedRegion)
│   ├── widget_state.rs (re-exports from types/state.rs)
│   ├── animation.rs, cursor.rs, event_processor.rs
│   ├── handlers.rs, shortcuts.rs, tooltip.rs, touch.rs
│   └── mod.rs
│
├── state/                    (depends on: types)
│   └── registry.rs (StateRegistry)
│
├── layout/                   (depends on: types)
│   ├── types.rs (LayoutNode, LayoutStyle, LayoutComputed, Display, etc.)
│   └── tree.rs (LayoutTree)
│
├── layout_helpers/           (depends on: types)
│   └── helpers/ (alignment/sizing free functions)
│
├── containers/               (depends on: layout, types)
│   ├── flex.rs
│   ├── stack.rs
│   └── scroll.rs
│
├── animation/                (depends on: types)
│   ├── spring.rs, easing.rs, decay.rs, timeline.rs
│   ├── blend.rs, color.rs, layers.rs, path.rs
│   ├── scroll.rs, stagger.rs, stroke.rs
│   ├── coordinator.rs (AnimationCoordinator)
│   ├── types.rs (AnimationKey, AnimationDriver, ActiveAnimation)
│   └── recipes/ (widget-specific presets)
│
├── context.rs                (depends on: animation, input, layout, state, types, widgets)
│
├── widgets/                  (depends on: types, render [via theme], input)
│   ├── button/ (ButtonType, ActionVariant, ToggleVariant, etc.)
│   ├── container/, popup/, panel/, overlay/
│   ├── text_input/, dropdown/, slider/, toast/
│   ├── checkbox.rs, radio_group.rs, context_menu.rs
│   ├── icon_button.rs, input.rs, scrollable.rs, scrollbar.rs
│   ├── slider_system.rs, toolbar.rs
│   └── mod.rs
│
├── panels/                   (depends on: types [rect])
│   ├── id.rs, rect.rs (PanelRect — f32 based)
│   ├── tree.rs, grid.rs (DockingTree, Leaf, Branch)
│   ├── manager.rs (DockingManager)
│   ├── separator.rs, drag.rs, drop_zone.rs, tabs.rs
│   ├── floating.rs, hit_test.rs, presets.rs
│   ├── layout.rs, snap_back.rs, serialize.rs
│   └── mod.rs (DockPanel trait)
│
├── panel_api/                (depends on: render, panels [PanelRect])
│   ├── traits.rs (PanelApp)
│   ├── types.rs (PanelRect, HitZone, PanelInput, PanelOutput, PanelTheme)
│   └── toolbar.rs (PanelToolbarDef)
│
├── macos/                    (depends on: animation [Spring, Easing])
│   ├── colors/ (ColorPalette, AppearanceMode)
│   ├── typography/, themes/, widgets/, animations/
│   ├── icons/, effects/, presets/
│   └── mod.rs
│
├── interactive/              (depends on: animation [Spring])
│   ├── elastic_slider.rs, animated_list.rs
│   ├── spotlight.rs, electric_border.rs
│   └── mod.rs
│
├── text_fx/                  (no uzor dependencies)
│   ├── shiny.rs, decrypt.rs, gradient.rs, fuzzy.rs
│   └── mod.rs
│
├── cursor/                   (depends on: types [Rect])
│   ├── magnet.rs, click_spark.rs, blob_cursor.rs, glare_hover.rs
│   └── mod.rs
│
├── numbers/                  (depends on: animation [Spring])
│   ├── counter.rs, count_up.rs
│   └── mod.rs
│
└── scroll_fx/                (no uzor dependencies)
    ├── scroll_reveal.rs, scroll_velocity.rs, scroll_float.rs
    └── mod.rs
```

---

## 22. Public Re-exports from `lib.rs`

```rust
// Core types — most commonly imported
pub use context::{Context, ButtonResponse};
pub use animation::AnimationCoordinator;
pub use types::{IconId, Rect, WidgetId, WidgetState};
pub use input::{InputState, InputCoordinator, LayerId, ScopedRegion};
pub use widgets::{IconButtonConfig, IconButtonResponse};

// All 9 widget type enums — for type-level widget taxonomy
pub use widgets::{
    ButtonType, ContainerType, PopupType,
    PanelType, ToolbarVariant, SidebarVariant, ModalVariant,
    OverlayType, TextInputType, DropdownType, SliderType, ToastType,
};
```

All other types require explicit `uzor::module::Type` paths.

---

*End of audit. Total source files analyzed: ~170 `.rs` files across 20 top-level modules.*
