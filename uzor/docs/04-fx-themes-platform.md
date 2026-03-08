# Uzor Core — Part 4: FX Modules, macOS Theme, and Platform Integration

**Source path:** `uzor/uzor/src/`
**Covers:** `macos/`, `interactive/`, `text_fx/`, `cursor/`, `numbers/`, `scroll_fx/`, `platform/`, `state/`

---

## Table of Contents

1. [macOS Theme System (`macos/`)](#1-macos-theme-system-macos)
2. [Interactive Effects (`interactive/`)](#2-interactive-effects-interactive)
3. [Text Effects (`text_fx/`)](#3-text-effects-text_fx)
4. [Cursor Effects (`cursor/`)](#4-cursor-effects-cursor)
5. [Number Animation (`numbers/`)](#5-number-animation-numbers)
6. [Scroll Effects (`scroll_fx/`)](#6-scroll-effects-scroll_fx)
7. [Platform Abstraction (`platform/`)](#7-platform-abstraction-platform)
8. [State Registry (`state/`)](#8-state-registry-state)

---

## 1. macOS Theme System (`macos/`)

**Source:** `src/macos/`

The macOS module is a complete, pixel-faithful implementation of Apple's Human Interface Guidelines. It provides data — colors, geometry, animation parameters — without performing any rendering itself. Every method returns values (hex strings, floats, booleans) that your renderer consumes.

### 1.1 Color System (`macos/colors/`)

**Source:** `src/macos/colors/mod.rs`, `light.rs`, `dark.rs`, `helpers.rs`

#### Appearance Modes

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AppearanceMode {
    Light,
    #[default]
    Dark,
    VibrantLight,
    VibrantDark,
    AccessibleLight,
    AccessibleDark,
    AccessibleVibrantLight,
    AccessibleVibrantDark,
}
```

Eight modes. `Dark` is the default. Vibrant variants are for materials with backdrop blur (sidebars, panels). Accessible variants increase contrast. Currently only `Light` and `Dark` have distinct palette constants — the other modes fall back to their base.

#### Widget State

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum WidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Disabled,
    Focused,
}
```

Used throughout the theme system to resolve state-dependent colors.

#### ColorPalette

`ColorPalette` is a `struct` of 44 `&'static str` color tokens. Every token is a CSS-compatible hex string (with or without alpha in the last two hex digits).

**Token categories:**

| Category | Fields |
|---|---|
| Labels | `label`, `secondary_label`, `tertiary_label`, `quaternary_label` |
| Text | `text`, `placeholder_text`, `selected_text`, `text_background`, `selected_text_background` |
| Content | `link`, `separator`, `selected_content_background`, `unemphasized_selected_content_background` |
| Menu | `selected_menu_item_text` |
| Table | `grid`, `header_text`, `alternating_even`, `alternating_odd` |
| Controls | `control_accent`, `control`, `control_background`, `control_text`, `disabled_control_text`, `selected_control`, `selected_control_text` |
| Windows | `window_background`, `window_frame_text`, `under_page_background` |
| System accents | `system_blue`, `system_brown`, `system_gray`, `system_green`, `system_indigo`, `system_orange`, `system_pink`, `system_purple`, `system_red`, `system_teal`, `system_yellow` |
| Fills | `fill_primary`, `fill_secondary`, `fill_tertiary`, `fill_quaternary` |
| Shadows | `shadow_color` |

**Light palette values (selected):**

| Token | Value |
|---|---|
| `label` | `#000000` |
| `control_accent` | `#007AFF` |
| `system_blue` | `#007AFF` |
| `system_green` | `#34C759` |
| `system_red` | `#FF3B30` |
| `window_background` | `#ECECEC` |
| `fill_primary` | `#00000014` |
| `shadow_color` | `#00000040` |

**Dark palette values (selected):**

| Token | Value |
|---|---|
| `label` | `#FFFFFF` |
| `control_accent` | `#0A84FF` |
| `system_blue` | `#0A84FF` |
| `system_green` | `#32D74B` |
| `system_red` | `#FF453A` |
| `window_background` | `#1E1E1E` |
| `fill_primary` | `#FFFFFF14` |
| `shadow_color` | `#00000080` |

#### Resolving a palette

```rust
use uzor::macos::colors::{AppearanceMode, palette};

let pal = palette(AppearanceMode::Dark);
println!("{}", pal.control_accent); // "#0A84FF"
```

`palette()` maps all dark variants to `&DARK` and everything else to `&LIGHT`. Both constants are `'static`.

#### Color helper

```rust
use uzor::macos::colors::helpers::color_with_alpha;

let semi = color_with_alpha("#007AFF", 0.5);
// Returns: "rgba(0, 122, 255, 0.50)"
```

Parses RGB from a 6-digit hex string, returns CSS `rgba()`. Alpha is clamped to `0.0..=1.0`.

---

### 1.2 Typography (`macos/typography/`)

**Source:** `src/macos/typography/mod.rs`

12-level hierarchy matching SF Pro's optical sizing in macOS Ventura/Sonoma.

#### Type scale

| Level | Size (px) | Weight | Line Height (px) |
|---|---|---|---|
| `LargeTitle` | 34 | Regular | 41 |
| `Title1` | 28 | Regular | 34 |
| `Title2` | 22 | Regular | 28 |
| `Title3` | 20 | Regular | 25 |
| `Headline` | 17 | **Bold** | 22 |
| `Body` | 13 | Regular | 16 |
| `Callout` | 13 | Regular | 16 |
| `Subheadline` | 12 | Regular | 16 |
| `Footnote` | 11 | Regular | 14 |
| `Caption1` | 11 | **Medium** | 14 |
| `Caption2` | 11 | Regular | 13 |
| `Monospaced` | 13 | Regular | 16 |

`Monospaced` uses `monospace` family; all others use `sans-serif`.

#### Usage

```rust
use uzor::macos::typography::{TypographyLevel, font_size, font_weight, font_string, line_height};

let level = TypographyLevel::Headline;
let px = font_size(level);         // 17.0
let lh = line_height(level);       // 22.0
let css = font_string(level);      // "bold 17px sans-serif"
```

`font_string()` produces a CSS-compatible font shorthand suitable for passing to `RenderContext::set_font()`.

---

### 1.3 Widget Themes (`macos/themes/`)

**Source:** `src/macos/themes/`

Nine widget theme structs. Each is created with an `AppearanceMode`, wraps `palette()` calls internally, and exposes methods that return concrete rendering values for any given `WidgetState`.

#### ButtonTheme

```rust
let theme = ButtonTheme::new(ButtonVariant::Accent, AppearanceMode::Dark);
let theme = theme.with_size(ButtonSize::Large);

// Query rendering values:
theme.bg_color(WidgetState::Normal)   // "#0A84FF"
theme.bg_color(WidgetState::Hovered)  // "#0070E0FF"
theme.bg_color(WidgetState::Pressed)  // "#0068D5FF"
theme.text_color(WidgetState::Normal) // "#FFFFFFFF"
theme.border_color(WidgetState::Normal) // "#00000000" (transparent for Accent)
theme.padding()          // (16.0, 8.0) for Large
theme.min_height()       // 34.0 for Large
theme.border_radius()    // 6.0
theme.border_width()     // 0.0 for Accent/Destructive, 0.5 for Default
theme.focus_ring_color() // system_blue
theme.focus_ring_offset() // 2.0
theme.focus_ring_width()  // 3.0
theme.disabled_opacity() // 0.5
```

Three variants:

- `ButtonVariant::Default` — `control` background, `control_text`, 0.5px border
- `ButtonVariant::Accent` — `system_blue` background, white text, no border
- `ButtonVariant::Destructive` — `system_red` background, white text, no border

Three sizes: `Small` (22px min-height), `Regular` (28px), `Large` (34px).

#### TrafficLightTheme

The window control buttons (close/minimize/maximize).

```rust
let theme = TrafficLightTheme::new(AppearanceMode::Dark);

// Geometry
theme.button_diameter()      // 12.0
theme.button_radius()        // 6.0
theme.button_gap()           // 8.0
theme.container_padding_x()  // 12.0
theme.total_width()          // 76.0

// Colors by state
theme.button_color(
    TrafficLightButton::Close,
    TrafficLightGroupState::Default,
    WidgetState::Normal,
) // "#FF5F57"

theme.button_color(
    TrafficLightButton::Close,
    TrafficLightGroupState::Unfocused,
    WidgetState::Normal,
) // "#80808080" — gray when window unfocused

// Icon visibility: icons appear only when group is hovered
theme.show_icon(TrafficLightGroupState::Hovered) // true
theme.show_icon(TrafficLightGroupState::Default)  // false

// Icon overlay colors (dark tints for contrast)
theme.icon_color(TrafficLightButton::Close)    // "#4C0002"
theme.icon_color(TrafficLightButton::Minimize) // "#995700"
theme.icon_color(TrafficLightButton::Maximize) // "#006500"
```

**Hit testing:**

```rust
let positions = theme.button_positions(container_x, container_y);
// Returns [(close_x, close_y), (minimize_x, minimize_y), (maximize_x, maximize_y)]

let hit = theme.hit_test(container_x, container_y, mouse_x, mouse_y);
// Returns Option<TrafficLightButton>
// Hit radius is 7.0px (slightly larger than visual 6.0px)

let hovered = theme.is_group_hovered(container_x, container_y, mouse_x, mouse_y);
// True if mouse is over the entire 3-button group
```

Traffic light colors by button and widget state:

| Button | Normal | Hovered | Pressed |
|---|---|---|---|
| Close | `#FF5F57` | `#FF6F67` | `#E0443E` |
| Minimize | `#FEBC2E` | `#FFCC4D` | `#DFA52A` |
| Maximize | `#28C840` | `#39D956` | `#1DAD36` |

#### SwitchTheme

```rust
let theme = SwitchTheme::new(AppearanceMode::Light);

// Geometry (per macOS HIG)
theme.width()         // 38.0
theme.height()        // 22.0
theme.thumb_size()    // 18.0
theme.thumb_margin()  // 2.0
theme.border_radius() // 11.0 (pill shape = height/2)

// Colors
theme.track_bg(true, WidgetState::Normal)   // system_green "#34C759"
theme.track_bg(false, WidgetState::Normal)  // fill_secondary "#0000000F"
theme.thumb_bg(WidgetState::Normal)         // "#FFFFFFFF" (always white)

// Thumb positioning
theme.thumb_x_offset(false) // 2.0 (left/off position)
theme.thumb_x_offset(true)  // 18.0 (right/on position = width - thumb_size - margin)
theme.thumb_y_offset()      // 2.0 (vertically centered)

// Animation
theme.animation_duration_ms() // 200
theme.animation_easing()      // "ease-in-out"

// Shadow struct
let shadow = theme.thumb_shadow();
// Shadow { offset_x: 0.0, offset_y: 2.0, blur_radius: 4.0, spread_radius: 0.0, color: "#00000026" }
```

#### ProgressTheme

Supports both bar and ring styles, three sizes each.

```rust
let theme = ProgressTheme::new(AppearanceMode::Dark);

// Bar geometry
theme.bar_height(ProgressSize::Small)    // 2.0
theme.bar_height(ProgressSize::Regular)  // 4.0
theme.bar_height(ProgressSize::Large)    // 6.0
theme.bar_border_radius(ProgressSize::Regular) // 2.0 (pill = height/2)

// Bar colors
theme.bar_fill_color()   // control_accent
theme.bar_track_color()  // fill_secondary
theme.bar_track_bg()     // separator

// Ring geometry
theme.ring_size(ProgressSize::Small)    // 16.0
theme.ring_size(ProgressSize::Regular)  // 32.0
theme.ring_size(ProgressSize::Large)    // 64.0
theme.ring_stroke_width(ProgressSize::Regular) // 3.0

// Ring colors
theme.ring_fill_color()   // control_accent
theme.ring_track_color()  // fill_secondary

// Shared
theme.indeterminate_animation_duration_ms() // 1500
theme.text_color()  // secondary_label
theme.text_font()   // "11px sans-serif"
```

#### Other widget themes

All follow the same pattern: constructed with `AppearanceMode`, methods return concrete values.

| Theme struct | Methods |
|---|---|
| `MenuTheme` | Background, border, item padding, separator height, selected item background |
| `CheckboxTheme` | Size (14x14), border radius (3px), fill/border colors per state, focus ring |
| `RadioTheme` | Size (16x16), fill/border colors per state, dot size |
| `InputTheme` | Background, border, text/placeholder colors, padding, border radius, focus ring |
| `DialogTheme` | Background, border radius, shadow, title/body text colors, padding |
| `TabTheme` | Active/inactive tab colors, border, selected indicator, padding |

---

### 1.4 Animation Presets (`macos/animations/`)

**Source:** `src/macos/animations/`

Six animation preset modules. All return data constants or simple calculation functions.

#### Modal

```rust
use uzor::macos::animations::modal::{OPEN, CLOSE};

// OPEN: 300ms, opacity 0→1, scale 1.08→1.0
// cubic-bezier(0.22, 0.61, 0.36, 1) — smooth ease-out
println!("{}", OPEN.duration_ms);   // 300.0
println!("{}", OPEN.scale_from);    // 1.08
println!("{}", OPEN.bezier.0);      // 0.22

// CLOSE: 200ms, opacity 1→0, scale 1.0→0.95
println!("{}", CLOSE.duration_ms);  // 200.0
println!("{}", CLOSE.scale_to);     // 0.95
```

Both are `pub const` values of `ModalAnimation { duration_ms, opacity_from, opacity_to, scale_from, scale_to, bezier }`.

#### Dock

```rust
use uzor::macos::animations::dock::{
    MAGNIFICATION_SPRING, MAGNIFICATION_SCALE,
    LAUNCH_BOUNCE_SPRING, LAUNCH_BOUNCE_COUNT, LAUNCH_BOUNCE_HEIGHT,
    icon_scale_from_distance,
};

// Magnification spring (stiffness=0.1, damping=0.38, mass=1.0)
// These are low-stiffness values — fluid, minimal overshoot

// Scale factor for icon at cursor: ~2.618x (golden ratio)
let scale = icon_scale_from_distance(0.5, MAGNIFICATION_SCALE);
// Gaussian falloff: scale = 1 + (max - 1) * e^(-2.5 * d^2)
```

`icon_scale_from_distance(distance, max_scale)` computes the scale for a dock icon at `distance` (normalized units) from the cursor using Gaussian falloff with `k = 2.5`.

Launch bounce: `stiffness=0.15, damping=0.3` (bouncier), 3 bounces, 0.5x icon-height displacement.

#### Other animation modules

| Module | Purpose |
|---|---|
| `button` | Press/release spring config for button feedback |
| `menu` | Menu open/close timing |
| `switch_toggle` | On/off slide spring config |
| `traffic_lights` | Hover fade timing (100ms) |

---

### 1.5 Icons (`macos/icons/paths.rs`)

All icons are inline SVG strings. They use `currentColor` for fill/stroke, making them trivially recolorable.

| Constant | ViewBox | Recommended Size |
|---|---|---|
| `CHECKMARK` | 0 0 100 100 | 18×18 (menu), 19×19 (checkbox) |
| `CHECKMARK_MIXED` | 0 0 100 100 | 18×18 |
| `CHEVRON_RIGHT` | 0 0 100 100 | 16×16 |
| `CHEVRON_LEFT` | 0 0 100 100 | 16×16 |
| `CHEVRON_DOWN` | 0 0 100 100 | 16×16 |
| `CHEVRON_UP` | 0 0 100 100 | 16×16 |
| `ARROW_UP` | 0 0 100 100 | 16×16 |
| `ARROW_DOWN` | 0 0 100 100 | 16×16 |
| `TRAFFIC_LIGHT_CLOSE` | 0 0 16 18 | 6×6 |
| `TRAFFIC_LIGHT_MINIMIZE` | 0 0 17 6 | 8×8 |
| `TRAFFIC_LIGHT_MAXIMIZE` | 0 0 17 16 | 8×8 |
| `TRAFFIC_LIGHT_FULLSCREEN` | 0 0 15 15 | 6×6 |
| `RADIO_DOT` | 0 0 100 100 | 7–8px inner dot |
| `CLEAR` | 0 0 100 100 | 12×12 (input), 16×16 (button) |
| `MINIMIZE` | 0 0 100 100 | 16×16 |
| `MAXIMIZE` | 0 0 100 100 | 16×16 |
| `RESTORE` | 0 0 100 100 | 16×16 |
| `SEARCH` | 0 0 100 100 | 16×16 (input), 20×20 (button) |

Traffic light icons have non-square viewboxes matching Apple's actual icon proportions.

Usage:

```rust
use uzor::macos::icons::paths::CHECKMARK;

// Pass to your renderer:
render_ctx.draw_svg(CHECKMARK, x, y, 18.0, 18.0, "#FFFFFF");
```

---

### 1.6 Effects (`macos/effects/`)

**Source:** `src/macos/effects/shadows.rs`, `gradients.rs`

Both files exist as stubs with module structure in place. Shadows and gradients are referenced by widget themes (e.g., `SwitchTheme::thumb_shadow()`) but the effects module itself is a placeholder for future multi-layer shadow and gradient parameter structs.

---

### 1.7 VenturaPreset (`macos/presets/ventura.rs`)

One-stop struct that creates all widget themes for a given appearance mode.

```rust
use uzor::macos::presets::ventura::VenturaPreset;
use uzor::macos::colors::AppearanceMode;

// Construction
let preset = VenturaPreset::dark();        // AppearanceMode::Dark
let preset = VenturaPreset::light();       // AppearanceMode::Light
let preset = VenturaPreset::new(AppearanceMode::AccessibleDark);
let preset = VenturaPreset::default();     // Dark (AppearanceMode default)

// Direct color palette access
let pal = preset.colors(); // &'static ColorPalette

// Widget themes — each creates a new theme struct on call
let btn     = preset.button_theme();           // ButtonVariant::Default
let accent  = preset.accent_button_theme();    // ButtonVariant::Accent
let destroy = preset.destructive_button_theme();
let menu    = preset.menu_theme();
let chk     = preset.checkbox_theme();
let radio   = preset.radio_theme();
let sw      = preset.switch_theme();
let input   = preset.input_theme();
let dialog  = preset.dialog_theme();
let tl      = preset.traffic_light_theme();
let prog    = preset.progress_theme();
let tab     = preset.tab_theme();
```

`VenturaPreset` holds only the `AppearanceMode` and constructs themes lazily. No caching — call once at the start of a render pass and keep the result for the frame.

**Typical usage pattern:**

```rust
fn render_frame(&self, mode: AppearanceMode) {
    let theme = VenturaPreset::new(mode);
    let colors = theme.colors();
    let btn = theme.button_theme();

    // Background
    ctx.fill_rect(0.0, 0.0, w, h, colors.window_background);

    // Button
    let state = if hovered { WidgetState::Hovered } else { WidgetState::Normal };
    ctx.fill_rounded_rect(bx, by, bw, bh, btn.border_radius(), btn.bg_color(state));
    ctx.set_color(btn.text_color(state));
    ctx.draw_text("OK", bx + bw/2.0, by + bh/2.0);
}
```

---

## 2. Interactive Effects (`interactive/`)

**Source:** `src/interactive/`

Four animation state managers for interactive UI components. All compute geometry and animation values only — rendering is the caller's responsibility.

### 2.1 ElasticSlider

**Source:** `src/interactive/elastic_slider.rs`

A slider with rubber-band overflow: dragging beyond min/max applies exponential decay instead of hard-clamping. On release, a spring animates the thumb back.

```rust
pub struct ElasticSlider {
    pub value: f32,        // Current slider value (clamped to min..max)
    pub min: f32,
    pub max: f32,
    pub step: f32,         // 0.0 = continuous
    pub max_overflow: f32, // Pixels before full saturation (default: 50.0)
    // (overflow and spring state are private)
}
```

**Construction:**

```rust
let mut slider = ElasticSlider::new(0.0, 100.0);
let mut slider = ElasticSlider::new(0.0, 10.0)
    .with_step(1.0)
    .with_max_overflow(30.0);
```

**Per-frame update (drag):**

```rust
// Call every frame while the user is dragging
slider.update_from_pointer(pointer_x, slider_width);
// pointer_x is pointer position relative to slider's left edge
// Can be negative (left overflow) or > slider_width (right overflow)
```

**On pointer release:**

```rust
slider.release(current_time); // Starts spring snap-back (with "animation" feature)
```

**Per-frame update (spring):**

```rust
slider.update(current_time); // Advances spring animation
```

**Reading state for rendering:**

```rust
let fill = slider.fill_percentage();    // 0.0..1.0 — fill the track this fraction
let overflow = slider.overflow();       // signed pixels — extend thumb past edge by this amount
let region = slider.overflow_region();  // OverflowRegion::{Left, None, Right}
```

**Overflow algorithm:**

The decay function is `sigmoid(x/max) * max` using `2*(1/(1+e^(-x)) - 0.5)`. At `x = max_overflow`, the result is ~46% of `max_overflow`. This creates increasing resistance as the thumb moves further.

---

### 2.2 SpotlightCard

**Source:** `src/interactive/spotlight.rs`

Tracks cursor position relative to a card and produces radial gradient parameters for a spotlight effect.

```rust
pub struct SpotlightCard {
    pub width: f32,
    pub height: f32,
    pub radius: f32,      // Spotlight radius (default: 200.0)
    // position and active state are private
}
```

**Construction:**

```rust
let mut card = SpotlightCard::new(400.0, 300.0);
let mut card = SpotlightCard::new(400.0, 300.0).with_radius(150.0);
```

**Update on mouse move:**

```rust
// Screen coordinates — the card converts to card-relative internally
card.update_cursor(cursor_x, cursor_y, card_x, card_y);

// Or set directly in card-local space:
card.set_spotlight_position(local_x, local_y);
```

**Reading state for rendering:**

```rust
let active = card.is_active();    // false if cursor is outside card bounds
let (x, y) = card.spotlight_center(); // card-relative position
let (nx, ny) = card.normalized_position(); // 0.0..1.0

// All-in-one: (x, y, radius, opacity) — opacity is 0.0 when inactive
let (x, y, r, opacity) = card.gradient_params();
```

**SpotlightColor helper:**

```rust
let color = SpotlightColor::white(0.25);  // default
let color = SpotlightColor::rgba(100, 149, 237, 0.3); // cornflower blue
let css = color.to_css(); // "rgba(100, 149, 237, 0.3)"
```

**Render pattern:**

```rust
card.update_cursor(mx, my, cx, cy);
let (sx, sy, sr, opacity) = card.gradient_params();
if opacity > 0.0 {
    ctx.fill_radial_gradient(card_x + sx, card_y + sy, sr, opacity);
}
```

---

### 2.3 ElectricBorder

**Source:** `src/interactive/electric_border.rs`

Generates a sequence of displaced points along a rounded rectangle perimeter. Points are displaced using multi-octave noise (10 octaves, lacunarity=1.6, gain=0.7), producing an animated lightning/electric effect.

```rust
pub struct ElectricBorder {
    pub width: f32,
    pub height: f32,
    pub border_radius: f32,  // Default: 24.0
    pub speed: f32,          // Animation speed multiplier (default: 1.0)
    pub chaos: f32,          // Displacement amplitude (default: 0.12)
    // time, sample_count, displacement are private
}
```

**Construction:**

```rust
let mut border = ElectricBorder::new(400.0, 300.0);
let mut border = ElectricBorder::new(400.0, 300.0)
    .with_radius(16.0)
    .with_speed(2.0)
    .with_chaos(0.2);
```

Sample count is auto-calculated as `perimeter / 2`, rounding to an integer. Larger borders have more sample points.

**Per-frame update:**

```rust
border.update(delta_time); // Advances internal time by delta * speed
// Or set absolute time:
border.set_time(t);
```

**Generate border points:**

```rust
let points: Vec<(f32, f32)> = border.generate_points();
// Returns sample_count + 1 points (closed path)
// Connect with line segments or a spline
```

**Render pattern:**

```rust
border.update(dt);
let points = border.generate_points();
ctx.begin_path();
if let Some(&(x, y)) = points.first() {
    ctx.move_to(x, y);
}
for &(x, y) in points.iter().skip(1) {
    ctx.line_to(x, y);
}
ctx.close_path();
ctx.stroke("#00FFFF", 1.5);
```

**Noise implementation:** 2D noise using `sin`-based hash (`(x * 12.9898).sin() * 43758.547 % 1.0`) with bilinear interpolation and smoothstep. Deterministic — same time produces same points.

---

### 2.4 AnimatedList

**Source:** `src/interactive/animated_list.rs`

Staggered entry/exit animations for list items. Each item animates independently with ease-out cubic easing.

```rust
pub struct ItemState {
    pub opacity: f32,    // 0.0 = invisible
    pub y_offset: f32,   // positive = shifted down
    pub scale: f32,      // 1.0 = normal
}
```

Entry start: `(opacity=0.0, y_offset=20.0, scale=0.7)` — invisible, below, small
Visible: `(opacity=1.0, y_offset=0.0, scale=1.0)` — fully shown
Exit end: `(opacity=0.0, y_offset=20.0, scale=0.7)` — same as entry start

```rust
pub struct AnimatedList {
    pub stagger_delay: f32,       // Default: 0.05s per item
    pub animation_duration: f32,  // Default: 0.2s per item
    // item_count and states are private
}
```

**Construction:**

```rust
let mut list = AnimatedList::new(5);           // 5 items, all start in entry state
let mut list = AnimatedList::new(5)
    .with_stagger_delay(0.08)
    .with_duration(0.3);
```

**Per-frame update:**

```rust
list.update(current_time); // Advances all animations; removes fully-exited items
```

**Changing item count:**

```rust
// Triggers entry animations for new items:
list.set_item_count(7, current_time);
// Triggers exit animations for removed items:
list.set_item_count(3, current_time);
```

**Global animate in/out:**

```rust
list.animate_in(current_time);   // All items play entry animation
list.animate_out(current_time);  // All items play exit animation
```

**Reading state for rendering:**

```rust
// Single item:
if let Some(state) = list.get_item_state(i) {
    ctx.set_opacity(state.opacity);
    ctx.translate(0.0, state.y_offset);
    ctx.scale(state.scale, state.scale);
    render_item(i);
}

// Iterator over all visible items:
for (index, state) in list.item_states() {
    // index is position in current list
}

// Check if any animations are running (for frame scheduling):
if list.is_animating() {
    request_redraw();
}
```

---

## 3. Text Effects (`text_fx/`)

**Source:** `src/text_fx/`

Four text animation effects. All use a `Config` + `State` pattern: `Config` holds parameters, `State` holds mutable animation progress. Call `state.update(...)` each tick to advance, then read state values for rendering.

### 3.1 DecryptedText — Scramble/Reveal Effect

**Source:** `src/text_fx/decrypt.rs`

Two modes:
- **Sequential**: reveals characters one by one (left→right, right→left, or center outward). Unrevealed characters are randomly scrambled each iteration.
- **Random scramble**: all characters scramble randomly for `max_iterations` iterations, then snap to original.

```rust
pub struct DecryptedTextConfig {
    pub speed_ms: u64,                   // Delay between iterations (default: 50ms)
    pub max_iterations: usize,           // For random mode (default: 10)
    pub sequential: bool,                // Sequential vs random (default: false)
    pub reveal_direction: RevealDirection,
    pub use_original_chars_only: bool,   // Shuffle from original char set (default: false)
    pub characters: String,              // Scramble character set (default: alphanumeric + symbols)
}

pub enum RevealDirection {
    Start,   // Left to right
    End,     // Right to left
    Center,  // Outward from middle
}
```

```rust
pub struct DecryptedTextState { /* private fields */ }
```

**Usage:**

```rust
let config = DecryptedTextConfig {
    sequential: true,
    reveal_direction: RevealDirection::Start,
    speed_ms: 50,
    ..Default::default()
};

let mut state = DecryptedTextState::new("HELLO WORLD");

// Call every tick (respecting speed_ms in your timing logic):
let chars: Vec<char> = state.update(&config);
let text: String = state.display_text(); // Same as collecting chars

// Check completion:
if state.is_complete() {
    // Text is now fully revealed
}

// Which chars are revealed (for per-char styling):
let revealed: &[bool] = state.revealed_indices(); // true = original char shown

// Reset for replay:
state.reset();
```

**Output to renderer:** render `display_text()` normally. For per-character styling (e.g., color the revealed chars differently), iterate over `revealed_indices()` and apply distinct colors.

---

### 3.2 FuzzyText — Scanline Distortion

**Source:** `src/text_fx/fuzzy.rs`

Per-row horizontal and/or vertical displacement creating a scanline glitch effect. Intensity varies by interaction state with optional periodic glitch spikes.

```rust
pub struct FuzzyTextConfig {
    pub base_intensity: f32,       // Default resting intensity (default: 0.18)
    pub hover_intensity: f32,      // Intensity on hover (default: 0.5)
    pub fuzz_range: f32,           // Max displacement in pixels (default: 30.0)
    pub direction: FuzzyDirection, // Horizontal, Vertical, or Both
    pub transition_duration: f64,  // Seconds to reach target (default: 0.0 = instant)
    pub glitch_mode: bool,         // Periodic intensity spikes (default: false)
    pub glitch_interval: f64,      // Seconds between glitches (default: 2.0)
    pub glitch_duration: f64,      // Seconds glitch lasts (default: 0.2)
}

pub enum FuzzyDirection { Horizontal, Vertical, Both }
```

```rust
pub struct FuzzyTextState { /* private */ }
```

**Usage:**

```rust
let config = FuzzyTextConfig::default();
let mut state = FuzzyTextState::new(config.base_intensity);

// Update interaction:
state.set_hovering(true);
state.set_clicking(true);   // intensity → 1.0 (max)
state.set_glitching(true);  // enables glitch mode tracking

// Per-frame update:
let intensity = state.update(delta_time, &config);

// Calculate per-row displacements (num_rows = text height in pixels):
let displacements: Vec<(f32, f32)> = state.calculate_displacements(num_rows, &config);
// Returns (dx, dy) pairs — dy is scaled by 0.5 relative to dx
```

**Render pattern:**

```rust
state.update(dt, &config);
let displacements = state.calculate_displacements(text_height_px, &config);

for (row, (dx, dy)) in displacements.iter().enumerate() {
    let y = text_origin_y + row as f32;
    // Clip to single row, translate by (dx, dy), draw text
    ctx.save();
    ctx.clip_rect(0.0, y, text_width, 1.0);
    ctx.translate(*dx, *dy);
    ctx.draw_text(&text, text_x, text_y);
    ctx.restore();
}
```

---

### 3.3 GradientText — Animated Multi-Color Gradient

**Source:** `src/text_fx/gradient.rs`

Animates text fill using a moving multi-stop gradient. The gradient is 300% of text width; position moves from 0% to 100%.

```rust
pub struct GradientTextConfig {
    pub colors: Vec<[u8; 3]>,          // RGB stops (default: purple→pink→lavender)
    pub animation_speed: f64,          // Seconds per full cycle (default: 8.0)
    pub direction: GradientDirection,  // Horizontal, Vertical, or Diagonal
    pub yoyo: bool,                    // Reverse after reaching end (default: true)
}

pub enum GradientDirection { Horizontal, Vertical, Diagonal }
```

```rust
pub struct GradientTextState { elapsed: f64 }
```

**Usage:**

```rust
let config = GradientTextConfig::default();
let mut state = GradientTextState::new();

// Per-frame:
let progress: f32 = state.update(delta_time, &config); // 0.0..100.0

// Convert to rendering parameters:
let (bg_x, bg_y) = GradientTextState::background_position(progress, config.direction);
// Returns normalized 0.0..1.0 position

// For seamless looping, duplicate first color at end:
use uzor::text_fx::gradient::seamless_gradient_colors;
let stops = seamless_gradient_colors(&config.colors);

// Get angle for linear gradient:
use uzor::text_fx::gradient::gradient_angle;
let angle = gradient_angle(config.direction); // 90.0/180.0/135.0 degrees
```

**Gradient sizing:** create the gradient at 300% width, position offset by `bg_x * 100%` (or `bg_y * 100%` for vertical). Use `clip` to show only the text shape.

---

### 3.4 ShinyText — Metallic Shine Sweep

**Source:** `src/text_fx/shiny.rs`

Animates a bright highlight band that sweeps across text. The gradient is 200% text width; position sweeps from 150% to -50%.

```rust
pub struct ShinyTextConfig {
    pub speed: f64,             // Seconds per sweep (default: 2.0)
    pub direction_left: bool,   // true = left→right, false = right→left
    pub yoyo: bool,             // Reverse after sweep (default: false)
    pub delay: f64,             // Pause between sweeps (default: 0.0)
    pub spread: f32,            // Gradient angle in degrees (default: 120.0)
}
```

**Usage:**

```rust
let config = ShinyTextConfig::default();
let mut state = ShinyTextState::new(config.direction_left);

// Per-frame:
let progress: f32 = state.update(delta_time, &config); // 0.0..100.0

// Convert to background position (0.0 = off right, 1.0 = off left):
let pos = ShinyTextState::background_position(progress);
// Formula: (150 - progress * 2 + 50) / 200
// progress=0 → pos=1.0 (gradient starts at 150%)
// progress=100 → pos=0.0 (gradient ends at -50%)
```

**Gradient structure:** bright center band at 70% with transparency on both sides. Apply to text using clip masking.

---

## 4. Cursor Effects (`cursor/`)

**Source:** `src/cursor/`

Four cursor interaction effects. All use a `Config` struct (holds parameters, implements the algorithm) separate from a `State` struct (holds mutable per-effect data). Pass state by mutable reference to the config's methods.

### 4.1 BlobCursor

**Source:** `src/cursor/blob_cursor.rs`

Multiple blobs follow the cursor with different lag — the lead blob moves fast, trailing blobs are slow. Merge into a single gooey shape via SVG gaussian blur + color matrix.

```rust
pub struct BlobCursor {
    pub count: usize,            // Number of blobs (default: 3)
    pub sizes: Vec<f32>,         // Diameters in pixels (default: [60, 125, 75])
    pub opacities: Vec<f32>,     // Per-blob opacity (default: [0.6, 0.6, 0.6])
    pub blob_type: BlobType,     // Circle or Square
    pub fast_duration: f64,      // Lead blob lag in seconds (default: 0.1)
    pub slow_duration: f64,      // Trailing blob lag in seconds (default: 0.5)
    pub inner_sizes: Vec<f32>,   // Inner dot diameters (default: [20, 35, 25])
}

pub struct BlobState {
    pub x: f32, pub y: f32,      // Current position
    pub vx: f32, pub vy: f32,    // Velocity
    pub size: f32,
    pub opacity: f32,
}

pub struct BlobCursorState {
    pub blobs: Vec<BlobState>,
}
```

**Usage:**

```rust
let cursor = BlobCursor::new()
    .with_count(3)
    .with_durations(0.08, 0.4);

// Initialize at starting position:
let mut state = cursor.init_state(screen_center_x, screen_center_y);

// Per-frame update:
cursor.update(&mut state, cursor_x, cursor_y, delta_time);

// Render:
for blob in &state.blobs {
    ctx.fill_circle(blob.x, blob.y, blob.size / 2.0, blob.opacity);
}
```

**Smoothing algorithm:** exponential decay via `factor = 1 - e^(-dt / (duration/4))`. Index 0 uses `fast_duration`, indices 1+ use `slow_duration`.

**Gooey filter parameters:**

```rust
pub struct GooeyFilter {
    pub std_deviation: f32,   // Gaussian blur sigma (default: 30.0)
    pub color_matrix: String, // "1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 35 -10"
}
```

Apply gaussian blur then color matrix (boosts contrast) to merge overlapping blobs visually.

---

### 4.2 ClickSpark

**Source:** `src/cursor/click_spark.rs`

Burst of line-segment particles radiating outward from click point. Particles shrink and fade as they travel.

```rust
pub struct ClickSpark {
    pub count: usize,      // Particles per click (default: 8)
    pub radius: f32,       // Travel distance in pixels (default: 15.0)
    pub size: f32,         // Initial line length in pixels (default: 10.0)
    pub duration: f64,     // Animation duration in seconds (default: 0.4)
    pub easing: Easing,    // Linear, EaseIn, EaseOut, EaseInOut (default: EaseOut)
}

pub struct Particle {
    pub origin_x: f32, pub origin_y: f32,
    pub angle: f32,         // Radians
    pub start_time: f64,
}

pub struct ParticleRender {
    pub x1: f32, pub y1: f32,  // Line start
    pub x2: f32, pub y2: f32,  // Line end
    pub opacity: f32,           // 1.0 at start, 0.0 at end
}
```

**Usage:**

```rust
let spark = ClickSpark::new().with_count(12).with_radius(20.0);
let mut state = ClickSparkState::default();

// On click:
spark.handle_click(&mut state, mouse_x, mouse_y, current_time);

// Per-frame:
let particles: Vec<ParticleRender> = spark.update(&mut state, current_time);
// Automatically removes expired particles

// Render:
for p in &particles {
    ctx.draw_line(p.x1, p.y1, p.x2, p.y2, p.opacity);
}
```

**Particle geometry:** 8 angles evenly spaced (`2π * i / count`). At progress `t`:
- `distance = eased(t) * radius`
- `line_length = size * (1 - eased(t))` — shrinks as it moves outward
- `x1 = origin + distance * cos(angle)`, `x2 = origin + (distance + line_length) * cos(angle)`

---

### 4.3 GlareHover

**Source:** `src/cursor/glare_hover.rs`

Bright diagonal shine that sweeps across a hovered element. On hover start, the gradient animates from off-screen to on-screen. Resets on unhover.

```rust
pub struct GlareHover {
    pub angle: f32,         // Gradient angle in degrees (default: -45.0)
    pub duration: f64,      // Sweep duration in seconds (default: 0.65)
    pub size: f32,          // Gradient size as % of element (default: 250.0)
    pub opacity: f32,       // Peak brightness (default: 0.5)
    pub play_once: bool,    // Only play first hover (default: false)
}

pub struct GlareHoverState {
    pub position: f32,               // 0.0..1.0 sweep progress
    pub is_hovering: bool,
    pub hover_start_time: Option<f64>,
    pub has_played: bool,
}
```

**Usage:**

```rust
let glare = GlareHover::new().with_angle(-45.0).with_opacity(0.4);
let mut state = GlareHoverState::default();

// On hover state change:
glare.set_hover(&mut state, is_hovering, current_time);

// Per-frame:
let pos = glare.update(&mut state, current_time); // 0.0..1.0

// Convert to rendering coordinates:
let (gx, gy) = glare.calculate_gradient_position(pos);
// position=0.0 → (-100%, -100%) off-screen
// position=1.0 → (100%, 100%) off-screen other side

// Gradient stops (5 stops):
let stops: Vec<GradientStop> = glare.gradient_stops();
// [0%=transparent, 60%=transparent, 70%=peak_opacity, 80%=transparent, 100%=transparent]

let angle_rad = glare.angle_radians();
```

**Easing:** ease-out quadratic (`t * (2 - t)`).

---

### 4.4 Magnet

**Source:** `src/cursor/magnet.rs`

Elements are attracted toward the cursor when it enters an activation zone. The offset is proportional to distance from center, divided by `strength`.

```rust
pub struct Magnet {
    pub padding: f32,     // Activation radius around element bounds (default: 100.0)
    pub strength: f32,    // Divides the offset (default: 2.0 — lower = stronger)
    pub disabled: bool,
}

pub struct MagnetState {
    pub offset_x: f32,   // Horizontal displacement in pixels
    pub offset_y: f32,   // Vertical displacement in pixels
    pub is_active: bool,
}

pub struct Rect {
    pub left: f32, pub top: f32,
    pub width: f32, pub height: f32,
}
```

**Usage:**

```rust
let magnet = Magnet::new().with_padding(80.0).with_strength(3.0);

// Per-frame (no mutable state required):
let element_rect = Rect { left: 100.0, top: 100.0, width: 200.0, height: 60.0 };
let state: MagnetState = magnet.update(cursor_x, cursor_y, element_rect);

if state.is_active {
    // Apply offset to element transform
    element.translate(state.offset_x, state.offset_y);
}
```

**Activation test:** `|center_x - cursor_x| < half_width + padding && |center_y - cursor_y| < half_height + padding`

**Offset formula:** `offset = (cursor - center) / strength`. Cursor at center → zero offset. Cursor at `strength * N` pixels from center → N pixel displacement.

---

## 5. Number Animation (`numbers/`)

**Source:** `src/numbers/`

Two number display animators. Both require the `animation` feature for spring physics; without it they return instantaneous final values.

### 5.1 Counter — Slot Machine Display

**Source:** `src/numbers/counter.rs`

Rolling digit display. Each digit is a vertical column of 0–9; spring physics smoothly rolls each column to the target digit.

```rust
pub struct Counter {
    pub value: f64,
    pub places: Option<Vec<PlaceValue>>,
    pub spring: Spring,        // (requires "animation" feature)
    pub time_offset: f64,
}

pub enum PlaceValue {
    Dot,          // Decimal separator
    Power(f64),   // 1.0, 10.0, 100.0, 0.1, 0.01, etc.
}

pub struct DigitState {
    pub digit_offsets: [f32; 10],  // Y-offset multipliers for digits 0-9
    pub spring_value: f64,
}

pub struct CounterState {
    pub digits: Vec<(PlaceValue, Option<DigitState>)>,
    // Option<DigitState> is None for PlaceValue::Dot
}
```

**Construction:**

```rust
let counter = Counter::new(1234.56); // Auto-detects places from value
let counter = Counter::new(1234.0)
    .with_places(vec![PlaceValue::Power(1000.0), PlaceValue::Power(100.0)])
    .with_time_offset(0.5); // Delay animation start by 0.5s
```

**Auto-detection:** `Counter::new(12.34)` produces places `[Power(10), Power(1), Dot, Power(0.1), Power(0.01)]`.

**Evaluate at time t:**

```rust
let state: CounterState = counter.evaluate(current_time);

for (place, digit_state) in &state.digits {
    if place.is_dot() {
        // Render a literal "." character
        continue;
    }
    let ds = digit_state.as_ref().unwrap();

    // Render a column of digits 0-9.
    // Each digit's Y position = digit_offsets[digit_num] * digit_height
    // Clip to show only the window at y=0
    for digit_num in 0..10usize {
        let y = ds.digit_offsets[digit_num] * digit_height;
        render_digit(digit_num, column_x, y);
    }
}
```

**Offset algorithm:**

For digit `n` at place value `p` with current animated value `v`:
```
place_value = v % 10
offset = (10 + n - place_value) % 10
if offset > 5: offset -= 10   // Wrap for shortest path
y_position = offset * digit_height
```

Digits within 5 positions of the target are pulled up; those beyond are pushed down. This creates a continuous scrolling effect.

---

### 5.2 CountUp — Spring-Animated Counter

**Source:** `src/numbers/count_up.rs`

Animates a single number from `from` to `to` using spring physics derived from duration.

```rust
pub struct CountUp {
    pub from: f64,
    pub to: f64,
    pub direction: Direction,  // Up or Down
    pub duration: f64,         // Seconds (default: 2.0)
    pub delay: f64,            // Seconds before start (default: 0.0)
    pub spring: Option<Spring>, // Custom spring (overrides duration calc)
}

pub enum Direction { Up, Down }

pub struct CountUpState {
    pub value: f64,
    pub is_complete: bool,
}
```

**Spring auto-calculation from duration:**
```
damping = 20 + 40 * (1 / duration)
stiffness = 100 * (1 / duration)
```

Short durations produce stiffer, more damped springs (fast but less bouncy). Long durations are softer and more elastic.

**Usage:**

```rust
let count_up = CountUp::new(0.0, 10_000.0)
    .with_duration(3.0)
    .with_delay(0.5)
    .with_direction(Direction::Up);

// Per-frame:
let state = count_up.evaluate(current_time);
let display = count_up.format_value(state.value);
// → ("9250", 0) for integer, ("92.50", 2) for decimal

let formatted = count_up.format_with_separator(state.value, ",");
// → "9,250"

if state.is_complete {
    // Spring has settled
}
```

**Decimal handling:** `get_decimal_places()` returns `max(decimals_in_from, decimals_in_to)`. `format_value()` respects this. Numbers like `100.0` are treated as 0 decimal places.

**Direction::Down:** starts at `to` and animates toward `from`. Use when displaying a countdown.

---

## 6. Scroll Effects (`scroll_fx/`)

**Source:** `src/scroll_fx/`

Three scroll-linked animation effects. All are driven by a `scroll_progress: f32` (0.0..1.0) or a `scroll_velocity: f32` (pixels/second).

### 6.1 ScrollFloat — Parallax Character Float

**Source:** `src/scroll_fx/scroll_float.rs`

Characters float up from below as the element scrolls into view. Per-character stagger creates a cascading wave.

```rust
pub struct ScrollFloatConfig {
    pub stagger: f32,           // Per-character delay (default: 0.03)
    pub initial_y_percent: f32, // Starting Y offset as % (default: 120.0%)
    pub initial_scale_y: f32,   // Starting vertical scale (default: 2.3)
    pub initial_scale_x: f32,   // Starting horizontal scale (default: 0.7)
}

pub struct CharState {
    pub opacity: f32,    // 0.0..1.0
    pub y_percent: f32,  // Percentage offset from baseline
    pub scale_y: f32,    // Vertical scale factor
    pub scale_x: f32,    // Horizontal scale factor
}
```

**Usage:**

```rust
let float = ScrollFloat::with_char_count("Hello World".len());

// From scroll progress:
let states: Vec<CharState> = float.compute_char_states(scroll_progress);

// Render each character:
for (i, state) in states.iter().enumerate() {
    let y_offset = state.y_percent / 100.0 * line_height;
    ctx.save();
    ctx.translate(char_x(i), char_y + y_offset);
    ctx.scale(state.scale_x, state.scale_y);
    ctx.set_opacity(state.opacity);
    ctx.draw_char(text_chars[i]);
    ctx.restore();
}
```

**Easing:** GSAP `back.inOut(2)` approximation. Starts slow, overshoots, eases in (overshoot = 1.70158 × 1.525 ≈ 2.59).

**Stagger math:** Total stagger accumulates as `(char_count - 1) * stagger`. The scroll progress range is expanded to `1.0 + total_stagger`, and each character's window is offset by `index * stagger`. Character 0 starts immediately; the last character starts when `scroll_progress` reaches `total_stagger / (1 + total_stagger)`.

---

### 6.2 ScrollReveal — Word-by-Word Reveal

**Source:** `src/scroll_fx/scroll_reveal.rs`

Words progressively reveal with opacity, blur, and container rotation tied to scroll position.

```rust
pub struct ScrollRevealConfig {
    pub enable_blur: bool,       // Apply blur to unrevealed words (default: true)
    pub base_opacity: f32,       // Hidden word opacity (default: 0.1)
    pub base_rotation: f32,      // Container rotation in degrees (default: 3.0)
    pub blur_strength: f32,      // Max blur in pixels (default: 4.0)
    pub stagger: f32,            // Per-word delay (default: 0.05)
}

pub struct WordState {
    pub opacity: f32,    // base_opacity..1.0
    pub blur: f32,       // blur_strength..0.0
}
```

**Usage:**

```rust
let words: Vec<&str> = text.split_whitespace().collect();
let reveal = ScrollReveal::with_word_count(words.len());

let rotation = reveal.compute_rotation(scroll_progress);
let word_states = reveal.compute_word_states(scroll_progress);

// Apply container rotation:
ctx.save();
ctx.rotate(rotation.to_radians());

for (word, state) in words.iter().zip(word_states.iter()) {
    ctx.set_opacity(state.opacity);
    ctx.set_blur(state.blur); // renderer-specific
    ctx.draw_text(word, ...);
}

ctx.restore();
```

**Interpolation:**
- `opacity = base_opacity + (1.0 - base_opacity) * word_progress`
- `blur = blur_strength * (1.0 - word_progress)`
- `rotation = base_rotation * (1.0 - scroll_progress)`

No easing on word reveal — linear interpolation from `base_opacity` to 1.0.

---

### 6.3 ScrollVelocity — Infinite Marquee with Velocity Boost

**Source:** `src/scroll_fx/scroll_velocity.rs`

Infinite horizontal text scroll. Base velocity always moves the text; scroll velocity adds a boost proportional to scroll speed. Spring smoothing prevents abrupt direction changes.

```rust
pub struct ScrollVelocityConfig {
    pub base_velocity: f32,           // px/s base scroll (default: 100.0)
    pub damping: f32,                 // Spring damping (default: 50.0)
    pub stiffness: f32,               // Spring stiffness (default: 400.0)
    pub num_copies: usize,            // Text copies for seamless wrap (default: 6)
    pub velocity_input_range: [f32; 2],  // Default: [0.0, 1000.0]
    pub velocity_output_range: [f32; 2], // Default: [0.0, 5.0]
}
```

**Usage:**

```rust
let mut scroller = ScrollVelocity::default();

// Per-frame update (call with current scroll velocity and delta time):
scroller.update(scroll_velocity_px_per_second, delta_time);

// Get X offset for rendering:
let text_width = measure_text("repeated text ");
let offset = scroller.x_offset(text_width);
// offset is in range [-text_width, 0]

// Render num_copies copies of the text starting at offset:
let n = scroller.num_copies();
for i in 0..n {
    ctx.draw_text(&text, offset + i as f32 * text_width, y);
}
```

**Velocity mapping:** `scroll_velocity` (px/s) is mapped from `velocity_input_range` to `velocity_output_range` linearly, then used as a multiplier on `base_velocity`. At 1000 px/s scroll, text moves at `base_velocity * (1 + 5) = 6x` normal speed.

**Direction:** positive scroll_velocity → `direction_factor = 1.0` (forward). Negative → `-1.0` (reverse). The spring prevents instant direction flips.

**Spring:** semi-implicit Euler integration with configurable stiffness and damping. High stiffness (400) and damping (50) gives crisp but still smoothed response.

---

## 7. Platform Abstraction (`platform/`)

**Source:** `src/platform/`

Defines the contracts that platform backends must fulfill. The core library uses these traits to remain portable across desktop, web, mobile, and TUI targets.

### 7.1 PlatformBackend Trait

**Source:** `src/platform/backends.rs`

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

`poll_events()` is the frame entry point: the application calls it each tick to drain all pending OS/browser events and process them.

### 7.2 SystemIntegration Trait

```rust
pub trait SystemIntegration {
    fn get_clipboard(&self) -> Option<String>;
    fn set_clipboard(&self, text: &str);
    fn get_system_theme(&self) -> Option<SystemTheme>;
}
```

Separate from `PlatformBackend` — backends can implement both or provide a distinct system integration object.

### 7.3 WindowConfig

```rust
pub struct WindowConfig {
    pub title: String,
    pub width: u32,           // Default: 800
    pub height: u32,          // Default: 600
    pub resizable: bool,      // Default: true
    pub decorations: bool,    // Default: true
    pub transparent: bool,    // Default: false
    pub visible: bool,        // Default: true
}

let config = WindowConfig::new("My App"); // All other fields at defaults
```

### 7.4 PlatformEvent

The complete event enum covers all input types across all platforms:

| Category | Events |
|---|---|
| Window | `WindowCreated`, `WindowResized`, `WindowMoved`, `WindowFocused`, `WindowCloseRequested`, `WindowDestroyed`, `RedrawRequested` |
| Pointer | `PointerEntered`, `PointerLeft`, `PointerMoved`, `PointerDown`, `PointerUp` |
| Touch | `TouchStart`, `TouchMove`, `TouchEnd`, `TouchCancel` |
| Keyboard | `KeyDown`, `KeyUp`, `TextInput`, `ModifiersChanged` |
| Scroll | `Scroll { dx, dy }` |
| System | `ClipboardPaste`, `FileDropped`, `FileHovered`, `FileCancelled` |
| IME | `Ime(ImeEvent)` — `Enabled`, `Preedit`, `Commit`, `Disabled` |
| System state | `ThemeChanged { dark_mode }`, `ScaleFactorChanged { scale }` |

### 7.5 Supporting Types

```rust
// Unique window handle — auto-incrementing atomic counter
pub struct WindowId(usize);
impl WindowId {
    pub fn new() -> Self { /* atomic fetch_add */ }
}

pub enum PlatformError {
    WindowNotFound,
    CreationFailed(String),
    NotSupported,
    SystemError(String),
}

// Result of processing one platform event
pub enum EventResult {
    Continue,  // Keep running
    Redraw,    // Request repaint
    Exit,      // Quit application
}

pub enum SystemTheme { Light, Dark }

// Implemented by backends that expose a render surface
pub trait RenderSurface: Send + Sync {
    fn size(&self) -> (u32, u32);
}
```

### 7.6 MockPlatform

A `MockPlatform` is provided for testing and headless use:

```rust
let mut platform = MockPlatform::new();

// Inject synthetic events:
platform.push_event(PlatformEvent::WindowResized { width: 1920, height: 1080 });
platform.set_system_theme(SystemTheme::Dark);

// Create window:
let id = platform.create_window(WindowConfig::default()).unwrap();

// Drain events:
let events = platform.poll_events();
```

### 7.7 How Platform Crates Connect

The four platform crates (`uzor-desktop`, `uzor-web`, `uzor-mobile`, `uzor-tui`) each implement `PlatformBackend`:

| Crate | Backend | Windowing |
|---|---|---|
| `uzor-desktop` | `DesktopPlatform` | winit + wgpu/vello |
| `uzor-web` | `WebPlatform` | Canvas 2D / WebGPU |
| `uzor-mobile` | `MobilePlatform` | Android/iOS views |
| `uzor-tui` | `TuiPlatform` | crossterm terminal |

Application code targets only `PlatformBackend` and `PlatformEvent` — the backend is plugged in at binary entry point.

**Application integration pattern:**

```rust
fn main() {
    let mut platform = DesktopPlatform::new();
    let window_id = platform.create_window(WindowConfig::new("App")).unwrap();
    let mut ctx = Context::new(800.0, 600.0);

    loop {
        for event in platform.poll_events() {
            match event {
                PlatformEvent::WindowCloseRequested => return,
                PlatformEvent::WindowResized { width, height } => {
                    ctx.set_viewport(width as f64, height as f64);
                    platform.request_redraw(window_id);
                }
                PlatformEvent::PointerMoved { x, y } => {
                    ctx.process_mouse_move(x, y);
                    platform.request_redraw(window_id);
                }
                PlatformEvent::RedrawRequested => {
                    render(&mut ctx, &mut platform, window_id);
                }
                _ => {}
            }
        }
    }
}
```

---

## 8. State Registry (`state/`)

**Source:** `src/state/registry.rs`

### 8.1 StateRegistry

Persistent widget state storage keyed by `WidgetId`. Uses `Box<dyn Any + Send + Sync>` to store heterogeneous state types. Each widget stores its own type; mismatched type access panics.

```rust
pub struct StateRegistry {
    states: HashMap<WidgetId, Box<dyn Any + Send + Sync>>,
}
```

`WidgetId` is defined in `src/types/state.rs`.

### 8.2 API

```rust
let mut registry = StateRegistry::new();

// Insert state:
registry.insert(widget_id, ScrollState { offset: 0.0 });

// Read-only access:
let scroll: Option<&ScrollState> = registry.get(&widget_id);

// Read-write access with automatic default:
let state: &mut ScrollState = registry.get_or_insert_with(widget_id, || {
    ScrollState { offset: 0.0 }
});
state.offset += delta;

// Remove when widget is destroyed:
registry.remove(&widget_id);

// Reset everything (e.g., on navigation):
registry.clear();
```

`get_or_insert_with` is the primary access pattern — it inserts the default if the ID is absent, then returns a mutable reference. The closure is only called on first access.

### 8.3 Type Safety

Type correctness is enforced at runtime via `downcast_ref`/`downcast_mut`. If you store a `ScrollState` for a `WidgetId` but then request a `FocusState` for the same ID, `get()` returns `None` and `get_or_insert_with()` panics with "State type mismatch for WidgetId". Each `WidgetId` must be used with a single consistent type throughout its lifetime.

### 8.4 Frame Lifecycle

`StateRegistry` lives on the `Context` and persists across frames. Widget code follows this pattern each frame:

```rust
fn tick_scroll_widget(ctx: &mut Context, id: WidgetId, delta: f64) {
    // Get or create state:
    let state = ctx.state.get_or_insert_with(id, || ScrollWidgetState {
        scroll_offset: 0.0,
        momentum: 0.0,
    });

    // Update:
    state.scroll_offset += delta;
    state.momentum = delta;

    // The updated state persists until next frame automatically.
}
```

State survives:
- Frame boundaries (each `begin_frame` / `end_frame` cycle)
- Widget re-renders (same ID = same state)

State does not survive:
- Explicit `registry.remove(id)` calls
- `registry.clear()`
- Application restart

### 8.5 What Uses the Registry

From the audit, the following behavior requires persistent state:

| Widget | State type | Key data |
|---|---|---|
| Scroll containers | `ScrollState` | `offset: f64`, `velocity: f64` |
| Text inputs | `TextInputState` | cursor position, selection range |
| Expandable panels | `PanelState` | `is_expanded: bool` |
| Tabs | `TabState` | `active_tab: usize` |
| Custom widgets | User-defined | Whatever the widget needs |

The registry is the single source of truth for any widget behavior that must survive a re-render.

---

## Summary

| Module | What it produces |
|---|---|
| `macos/colors` | Hex color strings from semantic tokens; 8 appearance modes |
| `macos/typography` | Font size, weight, line-height, CSS font strings |
| `macos/themes` | Per-state rendering values (colors, geometry, borders) for 9 widget types |
| `macos/animations` | Constants and formulas for modal, dock, switch, menu animations |
| `macos/icons` | Inline SVG strings with `currentColor` |
| `macos/presets` | `VenturaPreset` — single struct to access all macOS themes |
| `interactive` | Per-frame animation state: slider overflow, spotlight position, electric border points, list item transforms |
| `text_fx` | Per-tick scramble chars / intensity / progress values for 4 text effects |
| `cursor` | Per-frame blob positions, particle line segments, glare gradient position, magnet offsets |
| `numbers` | Per-frame digit column offsets and animated float values |
| `scroll_fx` | Per-frame character/word visual states and marquee X offset |
| `platform` | Trait contracts for windowing and events; `MockPlatform` for tests |
| `state` | Persistent `Any`-typed storage keyed by `WidgetId`, survives across frames |
