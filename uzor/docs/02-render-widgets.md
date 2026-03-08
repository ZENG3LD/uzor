# Uzor Core Library — Part 2: Rendering and Widgets

**Source path:** `uzor/uzor/src/`

---

## Table of Contents

1. [RenderContext Trait](#1-rendercontext-trait)
   - [Method Reference by Category](#11-method-reference-by-category)
   - [Default Implementations](#12-default-implementations)
   - [Glass and Blur Effects](#13-glass-and-blur-effects)
   - [RenderContextExt](#14-rendercontextext)
   - [RenderOp — Serializable Draw Calls](#15-renderop--serializable-draw-calls)
   - [SVG Rendering](#16-svg-rendering)
   - [Pixel-Alignment Helpers](#17-pixel-alignment-helpers)
   - [Icon System](#18-icon-system)
   - [Implementing a Custom Backend](#19-implementing-a-custom-backend)
2. [Widget System](#2-widget-system)
   - [The 5-File Structure](#21-the-5-file-structure)
   - [Theme Architecture](#22-theme-architecture)
   - [State Persistence Pattern](#23-state-persistence-pattern)
3. [Button Widget](#3-button-widget)
   - [ButtonType — The 6 Types](#31-buttontype--the-6-types)
   - [ActionVariant — 5 Variants](#32-actionvariant--5-variants)
   - [ToggleVariant — 3 Variants](#33-togglevariant--3-variants)
   - [CheckboxVariant](#34-checkboxvariant)
   - [TabVariant](#35-tabvariant)
   - [ColorSwatchVariant](#36-colorswatchvariant)
   - [DropdownVariant](#37-dropdownvariant)
   - [ButtonTheme Trait](#38-buttontheme-trait)
   - [ButtonState Trait](#39-buttonstate-trait)
   - [ButtonInputHandler Trait](#310-buttoninputhandler-trait)
   - [Default Parameters](#311-default-parameters)
4. [Container Widget](#4-container-widget)
5. [Popup Widget](#5-popup-widget)
6. [Panel Widget](#6-panel-widget)
7. [Overlay Widget](#7-overlay-widget)
8. [Text Input Widget](#8-text-input-widget)
9. [Dropdown Widget](#9-dropdown-widget)
10. [Slider Widget](#10-slider-widget)
11. [Toast Notifications](#11-toast-notifications)
12. [Context Menu](#12-context-menu)
13. [Checkbox](#13-checkbox)
14. [Radio Group](#14-radio-group)
15. [Scrollbar](#15-scrollbar)
16. [Scrollable Container](#16-scrollable-container)

---

## 1. RenderContext Trait

**File:** `src/render/context.rs`

`RenderContext` is the single bridge between uzor's headless geometry/interaction engine and the platform-specific drawing backend. It mirrors the HTML5 Canvas 2D API closely, making it familiar and straightforward to implement on top of any 2D drawing library (vello, tiny-skia, wgpu, WebCanvas, etc.).

All rendering is the caller's responsibility. Uzor computes geometry; you draw it.

```rust
pub trait RenderContext {
    // ... methods grouped below
}
```

### 1.1 Method Reference by Category

#### Dimensions

| Method | Returns | Description |
|--------|---------|-------------|
| `dpr(&self) -> f64` | `f64` | Device pixel ratio. Use to emit crisp per-pixel geometry. On standard displays returns `1.0`; on Retina/HiDPI returns `2.0` or more. |

#### Stroke Style

| Method | Description |
|--------|-------------|
| `set_stroke_color(&mut self, color: &str)` | Hex string `"#RRGGBB"` or `"#RRGGBBAA"`. Applied to all subsequent `stroke()` calls. |
| `set_stroke_width(&mut self, width: f64)` | Pixel width. Applied to subsequent `stroke()` calls. |
| `set_line_dash(&mut self, pattern: &[f64])` | Empty slice = solid line. Pass `&[4.0, 4.0]` for 4px dash/gap. |
| `set_line_cap(&mut self, cap: &str)` | `"butt"`, `"round"`, `"square"`. |
| `set_line_join(&mut self, join: &str)` | `"miter"`, `"round"`, `"bevel"`. |

#### Fill Style

| Method | Description |
|--------|-------------|
| `set_fill_color(&mut self, color: &str)` | Hex color string. Applied to subsequent `fill()` calls. |
| `set_fill_color_alpha(&mut self, color: &str, alpha: f64)` | **Default impl**: calls `set_fill_color` then `set_global_alpha`. Override if your backend has a combined API. |
| `set_global_alpha(&mut self, alpha: f64)` | `0.0` = transparent, `1.0` = opaque. Affects all drawing. |
| `reset_alpha(&mut self)` | **Default impl**: calls `set_global_alpha(1.0)`. |

#### Path Operations

| Method | Description |
|--------|-------------|
| `begin_path(&mut self)` | Discards current path, starts a new one. |
| `move_to(&mut self, x: f64, y: f64)` | Moves pen to `(x, y)` without drawing. |
| `line_to(&mut self, x: f64, y: f64)` | Draws line from current pen to `(x, y)`. |
| `close_path(&mut self)` | Closes path back to the starting point. |
| `rect(&mut self, x: f64, y: f64, w: f64, h: f64)` | Adds a rectangle subpath. Does not stroke or fill. |
| `arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64)` | Adds arc. Angles in radians. |
| `ellipse(&mut self, cx, cy, rx, ry, rotation, start, end)` | Adds an ellipse. |
| `quadratic_curve_to(&mut self, cpx, cpy, x, y)` | Quadratic Bézier. |
| `bezier_curve_to(&mut self, cp1x, cp1y, cp2x, cp2y, x, y)` | Cubic Bézier. |

#### Stroke / Fill / Clip

| Method | Description |
|--------|-------------|
| `stroke(&mut self)` | Strokes the current path. |
| `fill(&mut self)` | Fills the current path. |
| `clip(&mut self)` | Clips to the current path. All subsequent drawing is clipped. Must be inside `save()`/`restore()`. |
| `clip_rect(&mut self, x, y, width, height)` | **Default impl**: calls `begin_path`, `rect`, `clip`. |

#### Shape Helpers (convenience, have defaults)

| Method | Default? | Description |
|--------|----------|-------------|
| `stroke_rect(&mut self, x, y, w, h)` | No (required) | Stroke a rectangle without building a path manually. |
| `fill_rect(&mut self, x, y, w, h)` | No (required) | Fill a rectangle without building a path manually. |
| `fill_rounded_rect(&mut self, x, y, w, h, radius)` | Yes | Calls `begin_path` + `rounded_rect` + `fill`. |
| `stroke_rounded_rect(&mut self, x, y, w, h, radius)` | Yes | Calls `begin_path` + `rounded_rect` + `stroke`. |
| `rounded_rect(&mut self, x, y, w, h, r)` | Yes | Builds the rounded rect path using four arcs. Clamps `r` to `min(w/2, h/2)`. |

#### Text Rendering

| Method | Description |
|--------|-------------|
| `set_font(&mut self, font: &str)` | CSS-style: `"14px sans-serif"`, `"bold 16px monospace"`. |
| `set_text_align(&mut self, align: TextAlign)` | `TextAlign::Left` (default), `Center`, `Right`. |
| `set_text_baseline(&mut self, baseline: TextBaseline)` | `TextBaseline::Middle` (default), `Top`, `Bottom`, `Alphabetic`. |
| `fill_text(&mut self, text: &str, x: f64, y: f64)` | Fills text at position. |
| `stroke_text(&mut self, text: &str, x: f64, y: f64)` | Strokes text (outline). |
| `measure_text(&self, text: &str) -> f64` | Returns text width in pixels at current font. |
| `fill_text_rotated(&mut self, text, x, y, angle)` | **Default impl**: `save`, `translate(x,y)`, `rotate(angle)`, `fill_text(text, 0, 0)`, `restore`. No-ops for `angle < 0.001`. |
| `fill_text_centered(&mut self, text, x, y)` | **Default impl**: sets `Center`/`Middle`, calls `fill_text`. |

#### Transform Operations

| Method | Description |
|--------|-------------|
| `save(&mut self)` | Pushes transform stack. Saves transforms, styles, clip state. |
| `restore(&mut self)` | Pops transform stack. |
| `translate(&mut self, x: f64, y: f64)` | Shifts origin. |
| `rotate(&mut self, angle: f64)` | Rotates around current origin. Angle in radians. |
| `scale(&mut self, x: f64, y: f64)` | Scales from current origin. |

#### Images

| Method | Default? | Description |
|--------|----------|-------------|
| `draw_image(&mut self, image_id, x, y, width, height) -> bool` | Yes (no-op `false`) | Draws a cached image by ID. Returns `true` if drawn, `false` if not yet loaded. |
| `draw_image_rgba(&mut self, data, img_width, img_height, x, y, width, height)` | Yes (no-op) | Draws raw RGBA pixel data. Override for platform support. |

### 1.2 Default Implementations

These methods have working default implementations. You only override them if your backend has a more efficient native API:

```rust
// Default impl of fill_rounded_rect:
fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    self.begin_path();
    self.rounded_rect(x, y, w, h, radius);
    self.fill();
}

// Default impl of clip_rect:
fn clip_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
    self.begin_path();
    self.rect(x, y, width, height);
    self.clip();
}

// Default impl of fill_text_rotated (skips save/restore if angle ~0):
fn fill_text_rotated(&mut self, text: &str, x: f64, y: f64, angle: f64) {
    if angle.abs() < 0.001 {
        self.fill_text(text, x, y);
    } else {
        self.save();
        self.translate(x, y);
        self.rotate(angle);
        self.fill_text(text, 0.0, 0.0);
        self.restore();
    }
}
```

The `rounded_rect` path builder is implemented entirely in terms of other path primitives:
- Computes `r = r.min(w/2).min(h/2)` to prevent overlap
- Uses four `arc()` calls at the corners with `FRAC_PI_2` increments
- Closes the path with `close_path()`

### 1.3 Glass and Blur Effects

The trait includes methods for frosted glass / liquid glass UI styles. These are no-ops by default; only GPU-capable backends (e.g., `VelloGpuRenderContext`) override them.

```rust
// Check availability before calling
fn has_blur_background(&self) -> bool { false }

// Draw the blurred backdrop behind a UI element
fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) { }

// Check if 3D convex glass button style is enabled
fn use_convex_glass_buttons(&self) -> bool { false }
```

The higher-level state rectangle helpers select the correct visual strategy automatically:

```rust
// Automatically selects: 3D glass / flat glass / solid fill
fn draw_hover_rect(&mut self, x, y, width, height, color: &str) {
    if self.use_convex_glass_buttons() {
        self.draw_glass_button_3d(x, y, width, height, 2.0, false, color);
    } else if self.has_blur_background() {
        self.draw_blur_background(x, y, width, height);
        self.set_fill_color(color);
        self.fill_rect(x, y, width, height);
    } else {
        self.set_fill_color(color);
        self.fill_rect(x, y, width, height);
    }
}
```

Four state-rect methods follow the same dispatch logic:

| Method | State |
|--------|-------|
| `draw_hover_rect(x, y, w, h, color)` | Flat hover (no radius) |
| `draw_active_rect(x, y, w, h, color)` | Flat active/pressed |
| `draw_hover_rounded_rect(x, y, w, h, radius, color)` | Rounded hover |
| `draw_active_rounded_rect(x, y, w, h, radius, color)` | Rounded active |

Sidebar items add an accent bar on the left:

```rust
fn draw_sidebar_hover_item(
    &mut self,
    x: f64, y: f64, width: f64, height: f64,
    accent_color: &str,   // e.g. theme.colors.accent
    bg_color: &str,       // theme.hover_bg_styled()
    indicator_width: f64, // typically 3.0–4.0px
) {
    // accent bar on left, then hover background fills rest
    self.set_fill_color(accent_color);
    self.fill_rect(x, y, indicator_width, height);
    self.draw_hover_rect(x + indicator_width, y, width - indicator_width, height, bg_color);
}
```

`draw_glass_button_3d` creates an iOS-style raised button (blur backdrop + color overlay + convex highlight + inner shadow + rim lighting). The default trait implementation falls back to `draw_blur_background` + rounded fill. Override in `VelloGpuRenderContext` for the real effect.

### 1.4 RenderContextExt

**File:** `src/render/context.rs`

Extension trait for backends that support blur image management. Kept separate to avoid polluting the core trait.

```rust
pub trait RenderContextExt: RenderContext {
    type BlurImage: Clone;
    fn set_blur_image(&mut self, image: Option<Self::BlurImage>, width: u32, height: u32) {}
    fn set_use_convex_glass_buttons(&mut self, use_convex: bool) {}
}
```

Usage in a GPU backend:

```rust
impl RenderContextExt for VelloGpuRenderContext {
    type BlurImage = Arc<wgpu::Texture>;

    fn set_blur_image(&mut self, image: Option<Arc<wgpu::Texture>>, width: u32, height: u32) {
        self.blur_texture = image;
        self.blur_size = (width, height);
    }

    fn set_use_convex_glass_buttons(&mut self, use_convex: bool) {
        self.convex_glass = use_convex;
    }
}
```

The caller sets the blur image once per frame (after rendering the chart, before rendering UI):

```rust
ctx.set_blur_image(Some(blur_tex.clone()), width, height);
```

### 1.5 RenderOp — Serializable Draw Calls

**File:** `src/render/ops.rs`

`RenderOp` is a serializable enum representing a single draw call. Use it to:
- Record draw call lists for deferred rendering
- Serialize UI draw commands over a network or IPC channel
- Replay command lists on a different backend

```rust
pub enum RenderOp {
    // Style
    SetStrokeColor(String),
    SetFillColor(String),
    SetLineWidth(f64),
    SetLineDash(Vec<f64>),

    // Path
    BeginPath,
    MoveTo(f64, f64),
    LineTo(f64, f64),
    QuadraticCurveTo(f64, f64, f64, f64),
    BezierCurveTo(f64, f64, f64, f64, f64, f64),
    Arc(f64, f64, f64, f64, f64),
    Ellipse(f64, f64, f64, f64, f64, f64, f64),
    ClosePath,

    // Draw
    Stroke,
    Fill,
    StrokeRect(f64, f64, f64, f64),
    FillRect(f64, f64, f64, f64),

    // Text
    SetFont(String),
    SetTextAlign(TextAlign),
    FillText(String, f64, f64),
    StrokeText(String, f64, f64),

    // State
    Save,
    Restore,
    Translate(f64, f64),
    Rotate(f64),
    Scale(f64, f64),
    Clip,
}

pub type RenderOps = Vec<RenderOp>;
```

To record then replay:

```rust
// Record into a Vec
let mut ops: RenderOps = Vec::new();
// ... push ops manually or use a recording context wrapper

// Replay onto any RenderContext
execute_ops(&mut ctx, &ops);
```

`execute_ops` pattern-matches each variant and calls the corresponding `RenderContext` method. Note that `RenderOp` covers the core subset of the trait — it does not include blur/glass effects, image drawing, or the convenience rounded-rect helpers.

### 1.6 SVG Rendering

**File:** `src/render/svg.rs`

Draws an SVG string onto any `RenderContext`. No external SVG library dependency — the parser is built-in and handles the common subset used by icon SVGs.

```rust
pub fn draw_svg_icon(
    ctx: &mut dyn RenderContext,
    svg: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
)
```

Also available:

```rust
pub fn draw_svg_icon_rotated(
    ctx: &mut dyn RenderContext,
    svg: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
    angle: f64, // radians
)
```

**How it works:**

1. Parses the `viewBox` attribute to get source dimensions (default `24×24`).
2. Computes uniform scale to fit the icon within `(width, height)`, centering it.
3. Checks if root SVG has `fill="none"` — determines whether child elements default to stroke-only or fill mode.
4. Sets a fixed stroke width of `1.5 * scale` for consistent crispness at any size.
5. Parses and renders `<path>`, `<circle>`, `<rect>`, `<line>`, `<polyline>`, `<polygon>` elements.

Supported SVG elements:
- `path` with `d` attribute (full path command set)
- `circle` with `cx`, `cy`, `r`
- `rect` with `x`, `y`, `width`, `height`, `rx`, `ry`
- `line` with `x1`, `y1`, `x2`, `y2`
- `polyline` and `polygon` with `points`

Usage pattern:

```rust
use uzor::render::icons::aviation;
use uzor::render::draw_svg_icon;

// Draw a jet icon at (10, 10), 24×24, white
draw_svg_icon(ctx, aviation::JET, 10.0, 10.0, 24.0, 24.0, "#ffffff");

// Draw rotated (north-up radar convention: icon points up, rotate to heading)
use uzor::render::draw_svg_icon_rotated;
let heading_radians = 45_f64.to_radians();
draw_svg_icon_rotated(ctx, aviation::JET, cx, cy, 32.0, 32.0, "#00ff00", heading_radians);
```

### 1.7 Pixel-Alignment Helpers

**File:** `src/render/helpers.rs`

Two functions for snapping coordinates to device pixel boundaries. Critical for crisp 1px lines on HiDPI displays.

```rust
/// Snap a single value to device pixel center
pub fn crisp(val: f64, dpr: f64) -> f64 {
    (val * dpr).round() / dpr + 0.5 / dpr
}

/// Snap a rect to device pixel boundaries
pub fn crisp_rect(x: f64, y: f64, w: f64, h: f64, dpr: f64) -> (f64, f64, f64, f64) {
    let x1 = (x * dpr).round() / dpr;
    let y1 = (y * dpr).round() / dpr;
    let x2 = ((x + w) * dpr).round() / dpr;
    let y2 = ((y + h) * dpr).round() / dpr;
    (x1, y1, x2 - x1, y2 - y1)
}
```

The `+ 0.5 / dpr` offset in `crisp` places coordinate values at the pixel center rather than its left/top edge, which is what 2D rasterizers need to draw exactly 1px lines without antialiasing blur.

Usage:

```rust
let dpr = ctx.dpr();
let line_y = crisp(rect.y + rect.height / 2.0, dpr);
ctx.begin_path();
ctx.move_to(rect.x, line_y);
ctx.line_to(rect.x + rect.width, line_y);
ctx.stroke();

// For rectangles:
let (x, y, w, h) = crisp_rect(rect.x, rect.y, rect.width, rect.height, dpr);
ctx.fill_rect(x, y, w, h);
```

### 1.8 Icon System

**File:** `src/render/icons/`

Domain-specific SVG icon catalog. Icons are stored as `&'static str` constants holding raw SVG markup. All icons use top-down orientation conventions matching radar/map displays.

Available modules:

| Module | Contents |
|--------|----------|
| `icons::aviation` | `JET`, `JET_LARGE`, `PROP`, `HELICOPTER`, and others — top-down aircraft silhouettes |
| `icons::maritime` | Ships, vessels, boats |
| `icons::markers` | Location pins, points of interest |
| `icons::weather` | Storm cells, wind, precipitation |
| `icons::infrastructure` | Buildings, towers, installations |
| `icons::military` | Military unit markers |

Usage:

```rust
use uzor::render::icons::aviation;
use uzor::render::icons::military;
use uzor::render::{draw_svg_icon, draw_svg_icon_rotated};

// Static icon (no rotation needed)
draw_svg_icon(ctx, markers::PIN, x, y, 24.0, 24.0, "#ff4444");

// Rotated icon (aircraft heading)
draw_svg_icon_rotated(ctx, aviation::JET, cx, cy, 32.0, 32.0, "#00aaff", heading_rad);
```

All icons are `pub const` — zero runtime allocation, no file I/O.

### 1.9 Implementing a Custom Backend

To integrate uzor with a new rendering backend, implement `RenderContext`. You must implement all required methods (those without default implementations in the trait definition). Optional methods can be left as defaults or overridden for performance or feature support.

**Required methods (no defaults):**

```rust
impl RenderContext for MyBackend {
    // Dimensions
    fn dpr(&self) -> f64 { self.device_pixel_ratio }

    // Stroke style
    fn set_stroke_color(&mut self, color: &str) { /* parse hex, set state */ }
    fn set_stroke_width(&mut self, width: f64) { /* set state */ }
    fn set_line_dash(&mut self, pattern: &[f64]) { /* set state */ }
    fn set_line_cap(&mut self, cap: &str) { /* set state */ }
    fn set_line_join(&mut self, join: &str) { /* set state */ }

    // Fill style
    fn set_fill_color(&mut self, color: &str) { /* parse hex, set state */ }
    fn set_global_alpha(&mut self, alpha: f64) { /* set state */ }

    // Path
    fn begin_path(&mut self) { /* clear current path */ }
    fn move_to(&mut self, x: f64, y: f64) { /* move pen */ }
    fn line_to(&mut self, x: f64, y: f64) { /* add line segment */ }
    fn close_path(&mut self) { /* close subpath */ }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) { /* add rect subpath */ }
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start: f64, end: f64) { }
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, start: f64, end: f64) { }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) { }
    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) { }

    // Stroke/fill
    fn stroke(&mut self) { /* stroke current path with current stroke state */ }
    fn fill(&mut self) { /* fill current path with current fill state */ }
    fn clip(&mut self) { /* set clip from current path */ }

    // Direct shape draw
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) { }
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) { }

    // Text
    fn set_font(&mut self, font: &str) { }
    fn set_text_align(&mut self, align: TextAlign) { }
    fn set_text_baseline(&mut self, baseline: TextBaseline) { }
    fn fill_text(&mut self, text: &str, x: f64, y: f64) { }
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) { }
    fn measure_text(&self, text: &str) -> f64 { /* measure width at current font */ }

    // Transforms
    fn save(&mut self) { }
    fn restore(&mut self) { }
    fn translate(&mut self, x: f64, y: f64) { }
    fn rotate(&mut self, angle: f64) { }
    fn scale(&mut self, x: f64, y: f64) { }
}
```

**Optional overrides for GPU/glass features:**

```rust
// Only if backend supports image textures
fn draw_image(&mut self, image_id: &str, x: f64, y: f64, width: f64, height: f64) -> bool {
    /* look up cached texture, draw it, return true */
}

// Only if backend supports raw pixel upload
fn draw_image_rgba(&mut self, data: &[u8], img_width: u32, img_height: u32,
                   x: f64, y: f64, width: f64, height: f64) {
    /* upload to GPU or blit into software buffer */
}

// Only if backend supports blur effects
fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
    /* blit blurred texture region */
}

fn has_blur_background(&self) -> bool { self.blur_texture.is_some() }
fn use_convex_glass_buttons(&self) -> bool { self.convex_glass && self.has_blur_background() }

fn draw_glass_button_3d(&mut self, x: f64, y: f64, width: f64, height: f64,
                         radius: f64, is_active: bool, color: &str) {
    /* full 3D glass effect: blur + color + convex + specular + shadow + rim */
}
```

---

## 2. Widget System

### 2.1 The 5-File Structure

Every complex widget follows a consistent 5-file module structure:

```
widgets/{widget}/
├── types.rs    — semantic type enum (WHAT the widget is — no colors, no rendering)
├── state.rs    — interaction state trait + simple implementation
├── theme.rs    — color contract trait + default implementation
├── input.rs    — input handler trait + default implementation
├── defaults.rs — default sizes and prototype color constants
└── mod.rs      — re-exports
```

Simpler widgets (checkbox, radio group, scrollbar, scrollable) use a single `.rs` file with config + response structs.

### 2.2 Theme Architecture

Themes are **traits**, not data structs. This makes the theme system open for extension without modifying uzor core.

```
System Theme Manager (e.g., ToolbarTheme)
  implements ButtonTheme trait
        ↓
ButtonTheme trait (color contract)
        ↓
Factory render functions — accept &dyn ButtonTheme, call trait methods
```

The pattern separates:
- **Where colors are stored** (your app's theme struct)
- **Which colors are required** (the trait definition in uzor)
- **How colors are used** (factory render code)

To use a custom theme:

```rust
struct MyAppTheme {
    accent: String,
    bg_hover: String,
    // ...
}

impl ButtonTheme for MyAppTheme {
    fn button_accent(&self) -> &str { &self.accent }
    fn button_bg_hover(&self) -> &str { &self.bg_hover }
    // ... rest of required methods
}

// Pass to render function
render_action_button(ctx, &button, &my_theme, &state);
```

### 2.3 State Persistence Pattern

Widget state traits use a string ID system. The ID is typically the widget's field name or a path like `"toolbar.lock_button"`.

```rust
// Read state
let is_hovered = state.is_hovered("my_button_id");

// Write state — clearing hover passes None
state.set_hovered(Some("my_button_id")); // set hover
state.set_hovered(None);                  // clear hover
```

The provided `SimpleButtonState`, `SimpleSliderState`, etc. use `HashMap<String, bool>` internally and are suitable for prototyping. For production, implement the trait on your app's existing state struct to avoid the extra allocation.

---

## 3. Button Widget

**Files:** `src/widgets/button/`

The button system is the largest widget subsystem, covering 141 buttons across 6 types and 19 variants in the trading terminal.

### 3.1 ButtonType — The 6 Types

```rust
pub enum ButtonType {
    Action { variant: ActionVariant, position: (f64, f64), width: f64, height: f64 },
    Toggle { variant: ToggleVariant, position: (f64, f64), width: f64, height: f64 },
    Checkbox { variant: CheckboxVariant, position: (f64, f64), width: f64, height: f64 },
    Tab { variant: TabVariant, position: (f64, f64), width: f64, height: f64 },
    ColorSwatch { variant: ColorSwatchVariant, position: (f64, f64), width: f64, height: f64 },
    Dropdown { variant: DropdownVariant, position: (f64, f64), width: f64, height: f64 },
}
```

All variants carry the same geometry fields. Access them uniformly:

```rust
let pos = button.position();  // (f64, f64)
let w   = button.width();     // f64
let h   = button.height();    // f64
```

### 3.2 ActionVariant — 5 Variants

An Action button fires a one-shot action on click. It has no ON/OFF state.

```rust
pub enum ActionVariant {
    IconOnly    { icon: IconId, disabled: bool },
    Text        { text: String, style: ButtonStyle, disabled: bool },
    IconText    { icon: IconId, text: String, style: ButtonStyle, disabled: bool },
    LineText    { line_width: u32, text: String, style: ButtonStyle, disabled: bool },
    CheckboxText { checkbox_checked: bool, text: String, style: ButtonStyle, disabled: bool },
}
```

`ButtonStyle` controls the visual weight:

```rust
pub enum ButtonStyle {
    Default,  // border + transparent bg — standard secondary action
    Primary,  // filled accent background — "OK", confirm
    Danger,   // red hover state — delete, remove
    Ghost,    // no border, only hover highlight — subtle actions
}
```

Coverage in trading terminal:
- `IconOnly` — 47 buttons (Close, Delete, Settings, Alert, Lock, Add, More icons)
- `Text` — 5 buttons ("Template", "Cancel", "Apply to All", "Default")
- `IconText` — 3 buttons (Primary "OK" with icon)
- `LineText` — 1 button (line width selector: `━━━ 2`)
- `CheckboxText` — 7 buttons (theme/UI selection with radio-style checkbox)

Creating buttons:

```rust
let close_btn = ButtonType::Action {
    variant: ActionVariant::IconOnly { icon: IconId::Close, disabled: false },
    position: (x, y),
    width: 28.0,
    height: 28.0,
};

let ok_btn = ButtonType::Action {
    variant: ActionVariant::IconText {
        icon: IconId::Check,
        text: "OK".to_string(),
        style: ButtonStyle::Primary,
        disabled: false,
    },
    position: (x, y),
    width: 80.0,
    height: 28.0,
};
```

### 3.3 ToggleVariant — 3 Variants

A Toggle button maintains ON/OFF state.

```rust
pub enum ToggleVariant {
    IconSwap {
        icon_off: IconId,
        icon_on: IconId,
        toggled: bool,
    },
    Switch {
        toggled: bool,
        label: Option<String>,
    },
    ButtonToggle {
        content: ButtonContent,
        toggled: bool,
        show_active_border: bool,
    },
}

pub struct ButtonContent {
    pub text: Option<String>,
    pub icon: Option<IconId>,
    pub style: ButtonStyle,
}
```

- `IconSwap` — swaps icons (Eye/EyeOff) without changing background. Typically used for visibility toggles.
- `Switch` — iOS-style oval track with sliding ball. Reserved for future settings toggles.
- `ButtonToggle` — full button with active background highlight. Used for lock toggle with blue background.

```rust
let visibility = ButtonType::Toggle {
    variant: ToggleVariant::IconSwap {
        icon_off: IconId::EyeOff,
        icon_on: IconId::Eye,
        toggled: layer.visible,
    },
    position: (x, y),
    width: 16.0,
    height: 16.0,
};
```

### 3.4 CheckboxVariant

```rust
pub enum CheckboxVariant {
    Standard { checked: bool },
    Cross    { checked: bool },  // reserved
    Circle   { checked: bool },  // reserved
}
```

25 standard checkboxes in the Settings modal (Instrument, Scales, Status Line tabs).

### 3.5 TabVariant

```rust
pub enum TabVariant {
    Vertical   { label: Option<String>, icon: Option<IconId>, active: bool },
    Horizontal { label: Option<String>, icon: Option<IconId>, active: bool },
}
```

- `Vertical` — left accent bar (3px) + active background. Used in sidebar tabs (Settings: 7, Search categories: 8, Indicator tabs: 5).
- `Horizontal` — bottom underline or active background. Used in Primitive Settings tabs (Style, Coordinates, Visibility).

```rust
// Sidebar navigation tab
let settings_tab = ButtonType::Tab {
    variant: TabVariant::Vertical {
        label: Some("Settings".to_string()),
        icon: Some(IconId::Settings),
        active: current_tab == Tab::Settings,
    },
    position: (sidebar_x, tab_y),
    width: sidebar_width,
    height: 44.0,
};
```

### 3.6 ColorSwatchVariant

```rust
pub enum ColorSwatchVariant {
    Square        { color: String },
    IconWithBar   { icon: IconId, color: String },
    SwatchWithLabel { color: String, label: String }, // reserved
}
```

- `Square` — 24×24px colored square. Opens color picker on click. Used for 15 color fields in chart settings.
- `IconWithBar` — Icon + color bar below the icon. Used in inline drawing toolbar (ColorFill, TextColor buttons).

```rust
let body_up_color = ButtonType::ColorSwatch {
    variant: ColorSwatchVariant::Square {
        color: "#26a69a".to_string(), // current color value
    },
    position: (x, y),
    width: 24.0,
    height: 24.0,
};
```

### 3.7 DropdownVariant

```rust
pub enum DropdownVariant {
    TextChevron     { current_label: String, cycle_on_click: bool },
    Text            { current_label: String, cycle_on_click: bool },   // reserved
    IconTextChevron { current_icon: IconId, current_label: String, cycle_on_click: bool }, // reserved
    IconChevron     { current_icon: IconId, cycle_on_click: bool },    // reserved
    ChevronOnly     { direction: ChevronDirection },
}

pub enum ChevronDirection { Up, Down, Left, Right }
```

`TextChevron` is the primary pattern (10-12 buttons in settings dropdowns):
```
[current_value_text................|▼]
```
- Clicking the text body (left area) cycles through values if `cycle_on_click: true`
- Clicking the chevron (right ~20px) opens the dropdown menu

```rust
let bar_style = ButtonType::Dropdown {
    variant: DropdownVariant::TextChevron {
        current_label: "Candles".to_string(),
        cycle_on_click: true,
    },
    position: (x, y),
    width: 140.0,
    height: 28.0,
};
```

### 3.8 ButtonTheme Trait

**File:** `src/widgets/button/theme.rs`

Defines 18 required color methods:

```rust
pub trait ButtonTheme {
    // Backgrounds (5)
    fn button_bg_normal(&self) -> &str;
    fn button_bg_hover(&self) -> &str;
    fn button_bg_pressed(&self) -> &str;
    fn button_bg_active(&self) -> &str;
    fn button_bg_disabled(&self) -> &str;

    // Text (4)
    fn button_text_normal(&self) -> &str;
    fn button_text_hover(&self) -> &str;
    fn button_text_active(&self) -> &str;
    fn button_text_disabled(&self) -> &str;

    // Icons (4)
    fn button_icon_normal(&self) -> &str;
    fn button_icon_hover(&self) -> &str;
    fn button_icon_active(&self) -> &str;
    fn button_icon_disabled(&self) -> &str;

    // Borders (3)
    fn button_border_normal(&self) -> &str;
    fn button_border_hover(&self) -> &str;
    fn button_border_focused(&self) -> &str;

    // Semantic (4)
    fn button_accent(&self) -> &str;
    fn button_danger(&self) -> &str;
    fn button_success(&self) -> &str;
    fn button_warning(&self) -> &str;
}
```

`DefaultButtonTheme` is provided for prototyping (dark theme, blue accent):

```rust
use uzor::widgets::button::theme::DefaultButtonTheme;
let theme = DefaultButtonTheme::default();
// button_accent() → "#2962ff"
// button_danger() → "#ef5350"
// button_bg_active() → "#1e3a5f"
```

### 3.9 ButtonState Trait

**File:** `src/widgets/button/state.rs`

```rust
pub trait ButtonState {
    fn is_hovered(&self, button_id: &str) -> bool;
    fn is_pressed(&self, button_id: &str) -> bool;
    fn is_focused(&self, button_id: &str) -> bool;

    fn set_hovered(&mut self, button_id: Option<&str>);
    fn set_pressed(&mut self, button_id: Option<&str>);
    fn set_focused(&mut self, button_id: Option<&str>);
}
```

`SimpleButtonState` stores three `Option<String>` fields (only one button can be in each state at a time). This is usually correct since hover/press state is naturally exclusive.

```rust
let mut state = SimpleButtonState::new();
state.set_hovered(Some("close_btn"));
assert!(state.is_hovered("close_btn"));
assert!(!state.is_hovered("ok_btn"));
state.set_hovered(None); // clear
```

### 3.10 ButtonInputHandler Trait

**File:** `src/widgets/button/input.rs`

Provides hit testing and keyboard navigation. Default implementations are sufficient for most cases.

```rust
pub trait ButtonInputHandler {
    // Hit testing
    fn hit_test(&self, mouse_x: f64, mouse_y: f64, rect: &Rect) -> bool { /* AABB */ }

    // Click detection (press + release both inside rect)
    fn is_click(&self, press_x, press_y, release_x, release_y, rect: &Rect) -> bool { }

    // Tab navigation
    fn next_focus(&self, current_id: &str, all_ids: &[String]) -> String { /* wraps */ }
    fn prev_focus(&self, current_id: &str, all_ids: &[String]) -> String { /* wraps */ }

    // Keyboard activation
    fn is_activation_key(&self, key: &str) -> bool { /* "Enter" | "Space" | " " */ }
}
```

`DefaultButtonInputHandler` implements the trait using all defaults — zero-cost struct.

### 3.11 Default Parameters

**File:** `src/widgets/button/defaults.rs`

Per-variant size and prototype color structs. These are for rapid prototyping only — the terminal uses custom inline values. All structs implement `Default`.

| Struct | Key defaults |
|--------|-------------|
| `IconOnlyDefaults` | `icon_size: 16.0`, `hover_bg_radius: 4.0`, `icon_gap: 6.0` |
| `TextDefaults` | `height: 28.0`, `font_size: 13.0`, `border_radius: 4.0`, `padding_x: 12.0` |
| `IconTextDefaults` | `height: 28.0`, `icon_size: 16.0`, `icon_text_gap: 6.0` |
| `LineTextDefaults` | `width: 36.0`, `line_length_ratio: 0.6`, `number_font_size: 11.0` |
| `CheckboxTextDefaults` | `height: 28.0`, `checkbox_size: 16.0`, `font_size: 12.0` |
| `IconSwapDefaults` | `icon_size: 16.0`, `button_area: 16.0` |
| `ButtonToggleDefaults` | `button_size: 28.0`, `active_border_width: 3.0` |
| `CheckboxDefaults` | `checkbox_size: 16.0`, `border_radius: 3.0`, `row_height: 32.0` |
| `VerticalTabDefaults` | `tab_width: 70.0`, `tab_height: 44.0`, `icon_size: 20.0`, `active_bar_width: 3.0` |
| `HorizontalTabDefaults` | `tab_height: 32.0`, `padding_x: 12.0`, `underline_height: 2.0` |
| `ColorSwatchSquareDefaults` | `swatch_size: 24.0`, `border_radius: 4.0` |
| `ColorSwatchIconBarDefaults` | `button_size: 24.0`, `bar_height: 3.0` |
| `DropdownTextChevronDefaults` | `width: 140.0`, `height: 28.0`, `chevron_area_width: 20.0` |
| `DropdownChevronOnlyDefaults` | `button_width: 24.0`, `chevron_size: 12.0` |

Each defaults struct has a matching `*PrototypeColors` struct with dark theme colors for that state combination.

---

## 4. Container Widget

**Files:** `src/widgets/container/`

Containers wrap other content. They handle overflow and scrolling geometry.

```rust
pub enum ContainerType {
    Scrollable {
        scroll_offset: f64,
        content_height: f64,
        viewport_height: f64,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    Plain {
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}
```

Construction:

```rust
// Scrollable container: content taller than viewport
let container = ContainerType::scrollable(x, y, width, height, content_height);

// Plain container: no scrolling
let container = ContainerType::plain(x, y, width, height);
```

Key methods:

```rust
container.needs_scrollbar()  // true if content_height > viewport_height
container.position()         // (f64, f64)
container.width()            // f64
container.height()           // f64
```

Rendering pattern:

```rust
let (cx, cy) = container.position();
let cw = container.width();
let ch = container.height();

// Background
ctx.set_fill_color("#1e222d");
ctx.fill_rect(cx, cy, cw, ch);

// Clip to container bounds, render content, restore
ctx.save();
ctx.clip_rect(cx, cy, cw, ch);
render_content(ctx, scroll_offset);
ctx.restore();

// Scrollbar if needed
if container.needs_scrollbar() {
    render_scrollbar(ctx, &container);
}
```

---

## 5. Popup Widget

**Files:** `src/widgets/popup/`

Popups are click-triggered overlays that must be explicitly closed. Distinct from overlays (which are hover-triggered and auto-dismiss).

```rust
pub enum PopupType {
    ContextMenu {
        position: (f64, f64),
        selected_index: Option<usize>,
        width: f64,
        height: f64,
    },
    ColorPicker {
        position: (f64, f64),
        selected_color: Option<String>,
        custom_mode: bool,
        width: f64,
        height: f64,
    },
    Custom {
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}
```

Construction:

```rust
let ctx_menu = PopupType::context_menu(cursor_x, cursor_y, 180.0, 120.0);
let color_picker = PopupType::color_picker(swatch_x, swatch_y + swatch_height, 240.0, 300.0);
let confirm_dialog = PopupType::custom(center_x, center_y, 320.0, 160.0);
```

Popups are positioned absolutely at screen coordinates. The caller is responsible for viewport clamping (preventing the popup from going off-screen).

---

## 6. Panel Widget

**Files:** `src/widgets/panel/`

Panels are large structural containers that organize the UI into sections.

```rust
pub enum PanelType {
    Toolbar { variant: ToolbarVariant, position: (f64, f64), width: f64, height: f64 },
    Sidebar { variant: SidebarVariant, position: (f64, f64), width: f64, height: f64 },
    Modal   { variant: ModalVariant,   position: (f64, f64), width: f64, height: f64 },
    Hideable { is_hidden: bool,        position: (f64, f64), width: f64, height: f64 },
}
```

Variants:

```rust
pub enum ToolbarVariant { Top, Bottom, Left, Right }
pub enum SidebarVariant { Left, Right, Bottom }
pub enum ModalVariant   { Search, Settings, Simple, Primitive }
```

Construction:

```rust
let top_toolbar = PanelType::toolbar(ToolbarVariant::Top, 0.0, 0.0, viewport_w, 48.0);
let left_sidebar = PanelType::sidebar(SidebarVariant::Left, 0.0, 48.0, 240.0, viewport_h - 48.0);
let search_modal = PanelType::modal(ModalVariant::Search, modal_x, modal_y, 600.0, 400.0);
let indicator_menu = PanelType::hideable(panel_x, panel_y, 280.0, 360.0);
```

Query methods:

```rust
panel.is_toolbar()  // bool
panel.is_sidebar()  // bool
panel.is_modal()    // bool
panel.is_hideable() // bool
panel.position()    // (f64, f64)
panel.width()       // f64
panel.height()      // f64
```

---

## 7. Overlay Widget

**Files:** `src/widgets/overlay/`

Overlays render outside the normal UI layout. They appear automatically on hover and auto-dismiss. Distinct from popups (which require explicit click-to-open/close).

```rust
pub enum OverlayType {
    Tooltip {
        text: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    InfoOverlay {
        text: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}
```

Construction:

```rust
let tooltip = OverlayType::tooltip("Close panel", cursor_x + 12.0, cursor_y - 8.0, 80.0, 24.0);
let info = OverlayType::info_overlay("No data available", x, y, 160.0, 32.0);
```

Access:

```rust
overlay.position() // (f64, f64)
overlay.text()     // &str
overlay.width()    // f64
overlay.height()   // f64
```

Overlays are typically rendered after all main UI, always on top. The caller decides when to show them based on hover state from the input system.

---

## 8. Text Input Widget

**Files:** `src/widgets/text_input/`

Two-layer system: `TextInputType` describes what field exists; `TextInputState` manages the actual editing session.

### TextInputType

```rust
pub enum TextInputType {
    Text     { value, placeholder, focused, disabled, position, width, height },
    Number   { value, placeholder, focused, disabled, position, width, height },
    Search   { value, placeholder, focused, position, width, height },
    Password { value, placeholder, focused, position, width, height },
}
```

Construction:

```rust
let name_field = TextInputType::text("Enter name", x, y, 200.0, 32.0);
let price_field = TextInputType::number("0.00", x, y, 120.0, 32.0);
let search_box = TextInputType::search("Search symbols...", x, y, 300.0, 36.0);

// With initial value
let edit_field = TextInputType::text_with_value("existing text", "Placeholder", x, y, 200.0, 32.0);
let num_field  = TextInputType::number_with_value("42.5", "0.0", x, y, 120.0, 32.0);
```

Accessors (all variants):

```rust
input.value()       // &str
input.is_focused()  // bool
input.is_disabled() // bool
input.position()    // (f64, f64)
input.width()       // f64
input.height()      // f64
```

### TextInputState

`TextInputState` is the concrete editing state (not a trait). It manages the actual text buffer, cursor, selection, and blink timing.

```rust
pub struct TextInputState {
    pub is_active: bool,
    pub field_id: Option<String>,
    pub text: String,
    pub cursor: usize,            // character index, NOT byte index
    pub selection_start: Option<usize>,
    pub original_text: String,    // for cancel/undo
    pub blink_time: u64,          // ms timestamp for cursor blink
}
```

Typical lifecycle:

```rust
let mut state = TextInputState::new();

// User clicks on field → start editing
state.start_editing("price_input", "42.50");

// Check which field is active
if state.is_editing("price_input") {
    // Render cursor, highlight field border
}

// Handle keyboard events
state.insert_char('5');
state.backspace();
state.move_left(false);
state.move_right(true); // with_selection = true → extends selection
state.select_all();
state.move_home(false);
state.move_end(false);

// Clipboard
let copied = state.get_selected_text();
let cut_text = state.cut(); // returns text and deletes selection
state.paste("new content");

// Cursor blink
let visible = state.is_cursor_visible(current_time_ms);

// On Enter → confirm
let result: Option<String> = state.finish_editing();

// On Escape → cancel (restores original_text)
let restored: Option<String> = state.cancel_editing();
```

`TextInputStateTrait` is the trait version for dependency injection. `SimpleTextInputState` implements it using `HashMap<String, ...>` and enforces single-field focus (setting a new field focused clears all others).

---

## 9. Dropdown Widget

**Files:** `src/widgets/dropdown/`

Note: `DropdownType` (in `widgets/dropdown/`) describes the structural layout of a dropdown list. It is distinct from `DropdownVariant` in `widgets/button/types.rs` which describes the button that triggers the dropdown.

```rust
pub enum DropdownType {
    Standard {
        selected_index: Option<usize>,
        placeholder: String,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    Grid {
        selected_index: Option<usize>,
        columns: usize,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    Layout {
        selected_index: Option<usize>,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}
```

- `Standard` — vertical list, one item per row. Used for text option lists.
- `Grid` — multi-column grid layout. Used for icon/symbol pickers.
- `Layout` — preview-based layout picker. Used for chart layout selection.

Construction:

```rust
// Simple list, nothing selected yet
let dd = DropdownType::standard("Select option", x, y, 200.0, 28.0);

// With initial selection
let dd = DropdownType::standard_with_selection(2, "Select option", x, y, 200.0, 28.0);

// Grid picker, 4 columns
let dd = DropdownType::grid(4, x, y, 240.0, 200.0);

// Layout picker
let dd = DropdownType::layout(x, y, 280.0, 200.0);
```

The dropdown item list, rendering, and interaction are handled by the caller — `DropdownType` only describes geometry and selection state.

---

## 10. Slider Widget

**Files:** `src/widgets/slider/`

```rust
pub enum SliderType {
    Single {
        value: f64,
        min: f64,
        max: f64,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    Dual {
        min_value: f64,
        max_value: f64,
        min: f64,
        max: f64,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}
```

Construction:

```rust
// 0.0–1.0 normalized range
let vol = SliderType::single(0.75, x, y, 200.0, 20.0);

// Explicit range
let brightness = SliderType::single_with_range(128.0, 0.0, 255.0, x, y, 200.0, 20.0);

// Range slider (two handles)
let range = SliderType::dual(20.0, 80.0, x, y, 200.0, 20.0);
let range = SliderType::dual_with_range(20.0, 80.0, 0.0, 100.0, x, y, 200.0, 20.0);
```

### SliderState Trait

```rust
pub enum SliderHandle { Single, Min, Max }

pub trait SliderState {
    fn is_dragging(&self, slider_id: &str, handle: SliderHandle) -> bool;
    fn is_hovered(&self, slider_id: &str, handle: SliderHandle) -> bool;
    fn set_dragging(&mut self, slider_id: &str, handle: SliderHandle, dragging: bool);
    fn set_hovered(&mut self, slider_id: &str, handle: SliderHandle, hovered: bool);
}
```

`SimpleSliderState` uses `HashMap<(String, SliderHandle), bool>` internally.

Rendering pattern:

```rust
let (sx, sy) = slider.position();
let sw = slider.width();

// Compute handle position from value
if let SliderType::Single { value, min, max, .. } = &slider {
    let t = (value - min) / (max - min);
    let handle_x = sx + t * sw;
    let is_drag = state.is_dragging("volume_slider", SliderHandle::Single);
    // draw track, draw handle at handle_x
}
```

---

## 11. Toast Notifications

**Files:** `src/widgets/toast/`

```rust
pub enum ToastType {
    Info    { message: String, duration_ms: u32, position: (f64, f64), width: f64, height: f64 },
    Success { message: String, duration_ms: u32, position: (f64, f64), width: f64, height: f64 },
    Warning { message: String, duration_ms: u32, position: (f64, f64), width: f64, height: f64 },
    Error   { message: String, duration_ms: u32, position: (f64, f64), width: f64, height: f64 },
}
```

Default durations: `Info`/`Success` = 3000ms, `Warning` = 4000ms, `Error` = 5000ms.

Construction:

```rust
let t = ToastType::info("Settings saved", x, y, 240.0, 48.0);
let t = ToastType::success("Order filled", x, y, 240.0, 48.0);
let t = ToastType::warning("Connection slow", x, y, 280.0, 48.0);
let t = ToastType::error("API key invalid", x, y, 280.0, 48.0);
```

### ToastState Trait

```rust
pub trait ToastState {
    fn is_visible(&self, toast_id: &str) -> bool;
    fn opacity(&self, toast_id: &str) -> f64;
    fn remaining_duration_ms(&self, toast_id: &str) -> u32;
    fn vertical_offset(&self, toast_id: &str) -> f64;
    fn is_hovered(&self, toast_id: &str) -> bool;
    fn set_visible(&mut self, toast_id: &str, visible: bool);
    fn set_opacity(&mut self, toast_id: &str, opacity: f64);
    fn set_remaining_duration_ms(&mut self, toast_id: &str, ms: u32);
    fn set_vertical_offset(&mut self, toast_id: &str, offset: f64);
    fn set_hovered(&mut self, toast_id: &str, hovered: bool);
}
```

`SimpleToastState` uses five `HashMap<String, _>` maps. Clean up expired toasts with `remove_toast(id)`.

Toast lifecycle:

```rust
let mut state = SimpleToastState::new();

// Show a toast
state.set_visible("order_fill", true);
state.set_opacity("order_fill", 1.0);
state.set_remaining_duration_ms("order_fill", 3000);

// Each frame: reduce remaining duration, fade out when near expiry
let remaining = state.remaining_duration_ms("order_fill");
if remaining > 0 {
    state.set_remaining_duration_ms("order_fill", remaining.saturating_sub(frame_ms));
    if remaining < 500 {
        // Fade out over last 500ms
        state.set_opacity("order_fill", remaining as f64 / 500.0);
    }
} else {
    state.set_visible("order_fill", false);
    state.remove_toast("order_fill");
}
```

---

## 12. Context Menu

**File:** `src/widgets/context_menu.rs`

A complete right-click menu system with state management, hover tracking, and input handling.

### Key Types

```rust
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: Option<KeyboardShortcut>,
    pub enabled: bool,
    pub separator_after: bool,
    pub icon: Option<String>,
}

pub struct ContextMenuRequest {
    pub position: (f64, f64),
    pub items: Vec<ContextMenuItem>,
    pub source_widget: Option<WidgetId>,
}

pub struct ContextMenuState {
    active: Option<ContextMenuRequest>,
    hovered_item: Option<usize>,
    menu_rect: Option<(f64, f64, f64, f64)>,
}

pub struct ContextMenuResult {
    pub should_close: bool,
    pub clicked_item: Option<String>,
    pub hovered_index: Option<usize>,
}
```

### Building Items

```rust
// Builder pattern — methods consume and return Self
let items = vec![
    ContextMenuItem::new("copy", "Copy")
        .with_shortcut(KeyboardShortcut::command(KeyCode::C))
        .with_icon("clipboard"),
    ContextMenuItem::new("paste", "Paste")
        .with_shortcut(KeyboardShortcut::command(KeyCode::V)),
    ContextMenuItem::separator(),          // non-interactive divider
    ContextMenuItem::new("delete", "Delete")
        .disabled()
        .with_separator(),                 // separator after this item
];
```

### Lifecycle

```rust
let mut menu = ContextMenuState::new();

// Open on right-click
if right_click {
    menu.open((cursor_x, cursor_y), items);
}

// Optionally track which widget triggered it
menu.open_for_widget((cursor_x, cursor_y), items, widget_id);

// Query state for rendering
if menu.is_open() {
    let request = menu.get_active().unwrap();
    for (i, item) in request.items.iter().enumerate() {
        let is_hovered = menu.get_hovered() == Some(i);
        // render item at computed rect
    }
    // After computing layout, register rect for hit testing
    menu.set_menu_rect((menu_x, menu_y, menu_w, menu_h));
}

// Handle input each frame
let item_rects: Vec<(usize, (f64, f64, f64, f64))> = /* compute per-item rects */;
let result = handle_context_menu_input(
    &mut menu,
    &item_rects,
    Some((cursor_x, cursor_y)),
    mouse_clicked,
    clicked_outside,
);

if result.should_close {
    menu.close();
}
if let Some(item_id) = result.clicked_item {
    match item_id.as_str() {
        "copy"   => copy_selection(),
        "paste"  => paste_from_clipboard(),
        "delete" => delete_selected(),
        _ => {}
    }
}
```

The `handle_context_menu_input` free function processes mouse position against item rects, updates hover state, and returns a result struct. Clicking a disabled item or separator returns no `clicked_item`.

---

## 13. Checkbox

**File:** `src/widgets/checkbox.rs`

Simple config + response structs, no sub-traits.

```rust
pub struct CheckboxConfig {
    pub label: String,
    pub checked: bool,
    pub disabled: bool,
}

pub struct CheckboxResponse {
    pub toggled: bool,
    pub new_checked: bool,
    pub hovered: bool,
    pub state: WidgetState,
    pub rect: Rect,
}
```

Construction:

```rust
let chk = CheckboxConfig::new("Show grid")
    .with_checked(true)
    .with_disabled(false);
```

Both structs derive `Serialize`/`Deserialize`. The `CheckboxResponse.rect` field carries the computed bounding rect back to the caller for hit testing and rendering.

---

## 14. Radio Group

**File:** `src/widgets/radio_group.rs`

```rust
pub struct RadioOption {
    pub key: String,
    pub label: String,
    pub description: String,  // secondary text shown below label
}

pub struct RadioGroupConfig {
    pub options: Vec<RadioOption>,
    pub selected_index: usize,
    pub disabled: bool,
    pub item_height: f64,    // default: 52.0
    pub gap: f64,            // default: 8.0
    pub circle_radius: f64,  // default: 8.0
}

pub struct RadioGroupResponse {
    pub changed: Option<usize>,          // new index if selection changed
    pub hovered_index: Option<usize>,
    pub state: WidgetState,
    pub rect: Rect,                      // overall bounding rect
    pub option_rects: Vec<Rect>,         // per-option hit zones
}
```

Construction:

```rust
let options = vec![
    RadioOption::new("dark",   "Dark",   "Low-light trading environment"),
    RadioOption::new("light",  "Light",  "Bright daytime display"),
    RadioOption::new("custom", "Custom", "Your personalized theme"),
];
let config = RadioGroupConfig::new(options)
    .with_selected(0)
    .with_disabled(false);

// Compute total height for layout
let total_h = config.total_height(); // n * item_height + (n-1) * gap

// Get current selected key
if let Some(key) = config.selected_key() {
    println!("Selected: {}", key);
}
```

The `option_rects` field in `RadioGroupResponse` is populated by the caller after layout and used for per-option hit testing.

---

## 15. Scrollbar

**File:** `src/widgets/scrollbar.rs`

Pure geometry computation — no rendering, no state.

```rust
pub struct ScrollbarConfig {
    pub content_size: f64,
    pub viewport_size: f64,
    pub scroll_offset: f64,
    pub min_handle_size: f64, // default: 30.0
    pub horizontal: bool,     // default: false (vertical)
}

pub struct ScrollbarResponse {
    pub track_rect: Rect,
    pub handle_rect: Rect,
    pub scroll_offset: f64,
    pub dragged: bool,
}
```

Construction and geometry calculation:

```rust
let config = ScrollbarConfig::new(content_height, viewport_height, current_offset);

if config.needs_scrollbar() {
    let track_rect = Rect::new(container_right - 8.0, container_y, 8.0, container_height);

    // On idle frame
    let response = config.calculate_geometry(track_rect, None);

    // On drag (pass cursor Y position)
    let response = config.calculate_geometry(track_rect, Some(cursor_y));

    // Render track and handle
    ctx.set_fill_color("#2a2a2a");
    ctx.fill_rect(response.track_rect.x, response.track_rect.y,
                  response.track_rect.width, response.track_rect.height);
    ctx.set_fill_color("#4a4a4a");
    ctx.fill_rounded_rect(response.handle_rect.x, response.handle_rect.y,
                          response.handle_rect.width, response.handle_rect.height, 4.0);

    // Update scroll offset if dragging
    if response.dragged {
        scroll_offset = response.scroll_offset;
    }
}

// Query max scrollable distance
let max = config.max_scroll();
```

The geometry calculation maps `scroll_offset → handle_y` using visible ratio (handle height proportional to `viewport / content`) and scroll ratio (handle position proportional to `offset / max_scroll`).

---

## 16. Scrollable Container

**File:** `src/widgets/scrollable.rs`

High-level abstraction combining viewport management and scrollbar reservation.

```rust
pub struct ScrollableConfig {
    pub scrollbar_size: f64,        // default: 8.0
    pub always_show_scrollbar: bool, // default: false
}

pub struct ScrollableResponse {
    pub content_size: f64,
    pub viewport_size: f64,
    pub has_scrollbar: bool,
    pub viewport: Rect,
    pub content_area: Rect,   // viewport minus scrollbar space
}
```

Usage:

```rust
let viewport = Rect::new(x, y, width, height);
let container = ScrollableContainer::new(
    viewport,
    &scroll_state,           // contains scroll_state.offset and is_dragging
    Some(ScrollableConfig { scrollbar_size: 6.0, ..Default::default() }),
);

// Query content area before rendering content (to get available width)
let content_area = container.content_area();
let content_y_start = container.content_y(); // viewport.y - scroll_offset

// After measuring content height, calculate final geometry
let response = container.calculate(measured_content_height);

if response.has_scrollbar {
    // render content clipped to response.content_area
    // render scrollbar in the right 6px of response.viewport
}
```

`ScrollableContainer` differs from `ContainerType::Scrollable` in that it provides the actual rendering geometry helpers (`content_y()`, `content_width()`) rather than just the semantic description.

---

## Type Aliases

```rust
// From render/ops.rs
pub type RenderOps = Vec<RenderOp>;
```

## Text Alignment Enums

**File:** `src/render/types.rs`

```rust
pub enum TextAlign {
    Left,    // default
    Center,
    Right,
}

pub enum TextBaseline {
    Top,
    Middle,  // default
    Bottom,
    Alphabetic,
}
```

Both derive `Clone, Copy, Debug, Default, PartialEq, Eq`.
