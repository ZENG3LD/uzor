# Uzor Core — Part 1: Foundational Modules

**Source path:** `uzor/uzor/src/`

This document covers the four foundational modules that every uzor-based application builds on: the central `Context`, the primitive types, the input system, and the layout engine.

---

## Table of Contents

1. [Context (`context.rs`)](#1-context)
2. [Types (`types/`)](#2-types)
3. [Input System (`input/`)](#3-input-system)
4. [Layout (`layout/` and `layout_helpers/`)](#4-layout)

---

## 1. Context

**File:** `src/context.rs`

`Context` is the single entry point to the uzor engine. It owns the four sub-systems that together run each UI frame:

```rust
pub struct Context {
    pub input:      InputState,           // current-frame pointer + keyboard snapshot
    pub layout:     LayoutTree,           // computed widget rectangles
    pub registry:   StateRegistry,        // persistent per-widget data (scroll, focus, …)
    pub animations: AnimationCoordinator, // running tween / spring / decay animations
    pub time:       f64,                  // seconds since startup, sourced from InputState
}
```

### Ownership model

- `input` — rebuilt every frame from the platform event queue. Discarded at frame end.
- `layout` — the full `LayoutNode` tree, recomputed every frame from the current viewport.
- `registry` — survives across frames; stores `Box<dyn Any + Send + Sync>` keyed by `WidgetId`.
- `animations` — survives across frames; cleaned up when animations complete.

### Construction

Pass a root `LayoutNode` that describes the entire UI tree:

```rust
use uzor::{Context, layout::{LayoutNode, LayoutStyle, Display, FlexDirection, SizeSpec}};

let root = LayoutNode::new("root")
    .with_style(LayoutStyle {
        display: Display::Flex,
        direction: FlexDirection::Column,
        width: SizeSpec::Fill,
        height: SizeSpec::Fill,
        ..Default::default()
    })
    .with_child(
        LayoutNode::new("toolbar")
            .with_style(LayoutStyle {
                height: SizeSpec::Fix(40.0),
                width: SizeSpec::Fill,
                ..Default::default()
            })
    )
    .with_child(
        LayoutNode::new("content")
            .with_style(LayoutStyle {
                height: SizeSpec::Fill,
                width: SizeSpec::Fill,
                ..Default::default()
            })
    );

let mut ctx = Context::new(root);
```

### Frame lifecycle

Call `begin_frame` once per render tick. It:
1. Replaces `self.input` with the new snapshot.
2. Copies `input.time` into `self.time`.
3. Ticks all animations via `AnimationCoordinator::update`.
4. Recomputes the entire layout tree against the current viewport.

```rust
// Platform produces an InputState and reports the window size.
let input = build_input_state_from_events(&events);
let viewport = Rect::new(0.0, 0.0, window_width, window_height);

ctx.begin_frame(input, viewport);
```

After `begin_frame`, every widget rectangle is valid and all interaction queries are safe to call.

### Immediate-mode API

`Context` exposes four built-in widget interaction detectors. They all follow the same contract: resolve the widget's rect from the layout tree, test the current-frame `InputState` against it, and return a typed response struct. They never draw anything.

#### `button`

```rust
pub fn button(&mut self, id: impl Into<WidgetId>) -> ButtonResponse
```

```rust
pub struct ButtonResponse {
    pub clicked: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub state:   WidgetState,
    pub rect:    Rect,
}
```

Usage:

```rust
let resp = ctx.button("toolbar:zoom_in");
if resp.clicked {
    zoom_level += 0.1;
}
// resp.state drives the color you pass to your renderer
let bg_color = match resp.state {
    WidgetState::Normal  => COLOR_BUTTON,
    WidgetState::Hovered => COLOR_BUTTON_HOVER,
    WidgetState::Pressed => COLOR_BUTTON_PRESS,
    _ => COLOR_BUTTON,
};
render.rect(resp.rect, bg_color);
render.text(resp.rect, "Zoom In");
```

#### `checkbox`

```rust
pub fn checkbox(&mut self, id: impl Into<WidgetId>, checked: bool) -> CheckboxResponse
```

Returns:
- `toggled: bool` — true if clicked this frame.
- `new_checked: bool` — the value after toggle (pre-applied for you).
- `hovered: bool`, `state: WidgetState`, `rect: Rect`.

```rust
let resp = ctx.checkbox("settings:dark_mode", self.dark_mode);
if resp.toggled {
    self.dark_mode = resp.new_checked;
}
```

#### `scroll_area`

```rust
pub fn scroll_area(
    &mut self,
    id: impl Into<WidgetId>,
    content_height: f64,
) -> (Rect, ScrollState)
```

Handles physics-based scrolling internally. The returned `ScrollState::offset` is the pixel scroll position to apply when rendering content.

```rust
let (viewport_rect, scroll) = ctx.scroll_area("sidebar:list", total_list_height);

// Clip to viewport, then offset content
renderer.set_clip(viewport_rect);
renderer.translate(0.0, -scroll.offset);
for item in &self.list_items {
    render_item(renderer, item);
}
renderer.restore_clip();
```

The engine applies inertia-based deceleration automatically: `velocity *= 0.90` per frame, clamped to content bounds.

#### `icon_button`

```rust
pub fn icon_button(&mut self, id: impl Into<WidgetId>) -> IconButtonResponse
```

Identical semantics to `button`. The distinction (`icon_button` vs `button`) is for readability — it signals to the platform that this widget renders an icon rather than a text label.

Returns `{ clicked, hovered, state }`.

### Accessing persistent state

Use `ctx.state::<T>(id)` to read and mutate per-widget data that persists across frames. The type must implement `Default + Send + Sync + 'static`.

```rust
#[derive(Default)]
struct InputFieldState {
    text: String,
    cursor_pos: usize,
}

let field_state = ctx.state::<InputFieldState>("search_field");
if some_key_was_pressed {
    field_state.text.push(ch);
    field_state.cursor_pos += 1;
}
```

Under the hood this calls `StateRegistry::get_or_insert_with`, which downcasts through `Box<dyn Any>`. If you store a different type under the same `WidgetId` the registry will panic at the downcast.

### Getting widget rectangles

```rust
// After begin_frame, all rects are valid.
let rect: Rect = ctx.widget_rect(&WidgetId::new("toolbar:zoom_in"));
// Returns Rect::default() (zero-sized at origin) if the id is not in the layout tree.
```

---

## 2. Types

**Directory:** `src/types/`
**Modules:** `rect`, `state`, `icon`
**Re-exported from:** `uzor::types::*`

### `Rect`

```rust
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub type WidgetRect = Rect; // compatibility alias
```

All coordinates are in logical pixels (DPI scaling applied by the platform before passing to uzor).

#### Construction

```rust
let r = Rect::new(10.0, 20.0, 200.0, 40.0);
```

#### Geometry accessors

| Method | Returns |
|--------|---------|
| `min_x()` | `x` |
| `min_y()` | `y` |
| `max_x()` | `x + width` |
| `max_y()` | `y + height` |
| `right()` | alias for `max_x()` |
| `bottom()` | alias for `max_y()` |
| `center_x()` | `x + width / 2` |
| `center_y()` | `y + height / 2` |

#### Containment and intersection

```rust
// Point containment (inclusive on all edges)
let inside = rect.contains(mouse_x, mouse_y);

// Intersection — returns a zero-area rect when they don't overlap
let overlap = rect_a.intersect(rect_b);
let do_overlap = overlap.width > 0.0 && overlap.height > 0.0;
```

#### Layout helpers

```rust
// Shrink uniformly by `padding` on all four sides
let inner = outer.inset(8.0);

// Split into two non-overlapping halves
let (left, right)  = rect.split_horizontal(sidebar_width);
let (top, bottom)  = rect.split_vertical(toolbar_height);
```

`split_horizontal(left_width)` clamps `left_width` to `self.width` so the right piece is never negative. Same for `split_vertical`.

### `WidgetId`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(pub String);
```

Widget IDs are plain strings. The coordinator uses them as `HashMap` keys, so they must be stable across frames for state persistence to work.

Construction:

```rust
let id = WidgetId::new("sidebar:search_btn");
let id: WidgetId = "sidebar:search_btn".into(); // From<&str>
let id: WidgetId = some_string.into();           // From<String>
```

**Naming convention used in the codebase:** `"panel:widget"` or `"region:sub:widget"`. Scoped regions (see §3) use a colon prefix automatically: `"chart:btn_zoom_in"`.

### `WidgetState`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Active,    // toggled/selected
    Toggled,   // for checkboxes / toggle buttons (on)
    Disabled,
}
```

Helper predicates:

```rust
state.is_hovered()  // true for Hovered or Pressed
state.is_pressed()  // true only for Pressed
state.is_disabled() // true only for Disabled
```

The `Context` immediate-mode methods return a `WidgetState` so you can drive visual styling without additional logic.

### `WidgetInputState`

`WidgetInputState` is the coordinator-level aggregation of per-widget interaction history. It persists across frames inside `InputCoordinator` and is not normally accessed directly by application code.

```rust
pub struct WidgetInputState {
    pub focus:                    FocusState,
    pub hover:                    HoverState,
    pub drag:                     DragState,
    pub active:                   Option<WidgetId>, // pressed, not yet released
    pub last_click_time:          f64,
    pub last_click_pos:           (f64, f64),
    pub last_click_widget:        Option<WidgetId>,
    pub double_click_threshold_ms: f64,             // default 500 ms
    pub double_click_distance:    f64,              // default 5 px
    pub click_count:              u8,               // 1 / 2 / 3
    pub triple_click_threshold_ms: f64,             // default 300 ms
}
```

The coordinator uses `WidgetInputState::mouse_release(x, y, now)` to classify mouse releases:

- Single click: no prior click within threshold on the same widget.
- Double click: second click within `double_click_threshold_ms` and `double_click_distance`.
- Triple click: third click within `triple_click_threshold_ms`.

**Also available (from `types/state.rs`) for direct use:**

- `FocusState` — tracks which widget has keyboard focus, supports deferred `request_focus` → resolved at frame end.
- `HoverState` — tracks the currently hovered widget and mouse position.
- `DragState` — tracks the dragged widget, start/current position, and initial value (for slider mapping).
- `ScrollState` — full scroll physics for manual scrollable containers (wheel, drag, track click, clamp).

### `ScrollState`

Used when you want scrolling without going through `Context::scroll_area`:

```rust
pub struct ScrollState {
    pub offset:             f64,
    pub velocity:           f64,
    pub content_size:       f64,
    pub is_dragging:        bool,
    pub drag_start_y:       Option<f64>,
    pub drag_start_offset:  Option<f64>,
}
```

Key methods:

```rust
// Mouse wheel: returns false if content fits entirely in viewport
state.handle_wheel(delta_y, content_height, viewport_height) -> bool;

// Scrollbar thumb drag
state.start_drag(y);
state.handle_drag(y, track_height, content_height, viewport_height);
state.end_drag();

// Click on scrollbar track (jump-to)
state.handle_track_click(click_y, track_y, track_height, content_height, viewport_height);

// Clamp after external offset mutation
state.clamp(content_height, viewport_height);

// Reset to top (e.g. when content changes)
state.reset();
```

### `IconId`

```rust
pub struct IconId(pub String);
```

A string handle to an icon asset. Backends resolve `IconId` to actual texture/path lookups. Constructed with `IconId::new("chevron_down")` or `"chevron_down".into()`.

---

## 3. Input System

**Directory:** `src/input/`

The input system has three distinct layers:

| Layer | Type | Purpose |
|-------|------|---------|
| Frame snapshot | `InputState` | One-frame copy of all pointer + keyboard state |
| Event router | `InputCoordinator` | Hit testing, z-order, scoped regions, response generation |
| Persistent state | `WidgetInputState` | Per-widget hover/drag/focus history, click counting |

### `InputState` — the frame snapshot

Platforms build one `InputState` per frame by translating native OS events:

```rust
pub struct InputState {
    pub pointer:     PointerState,
    pub modifiers:   ModifierKeys,
    pub scroll_delta: (f64, f64),  // (dx, dy) in logical pixels
    pub drag:        Option<DragState>,
    pub dt:          f64,           // seconds since last frame
    pub time:        f64,           // seconds since startup
    pub multi_touch: Option<TouchState>,
}
```

`PointerState`:

```rust
pub struct PointerState {
    pub pos:            Option<(f64, f64)>, // None = cursor outside window
    pub button_down:    Option<MouseButton>, // held this frame
    pub clicked:        Option<MouseButton>, // released this frame
    pub double_clicked: Option<MouseButton>,
    pub triple_clicked: Option<MouseButton>,
    pub prev_pos:       Option<(f64, f64)>,
}
```

`ModifierKeys`:

```rust
pub struct ModifierKeys {
    pub shift: bool,
    pub ctrl:  bool,
    pub alt:   bool,
    pub meta:  bool, // Cmd on macOS, Win on Windows
}

// Cross-platform command key
modifiers.command() // → meta on macOS, ctrl elsewhere
```

Convenience predicates on `InputState`:

```rust
input.is_hovered(&rect)      // true if pointer is inside rect
input.is_clicked()           // left mouse released this frame
input.is_double_clicked()    // left mouse double-clicked this frame
input.is_right_clicked()
input.is_middle_clicked()
input.is_mouse_down()        // left button held
input.is_dragging()          // drag: Some(_)
input.drag_delta()           // Option<(dx, dy)>
input.shift()                // modifier shortcuts
input.ctrl()
input.alt()

// Consume events to prevent double-handling
input.consume_click()  -> bool   // takes clicked, returns true if there was one
input.consume_scroll() -> (f64, f64)

// Clear per-frame transient state — called by the coordinator at end_frame
input.end_frame()
```

### `InputCoordinator` — the event router

`InputCoordinator` connects the per-frame `InputState` snapshot to individual widget registrations. It is the source of truth for z-order and hit testing.

```rust
pub struct InputCoordinator {
    widgets:        Vec<RegisteredWidget>,   // cleared each frame
    layers:         Vec<Layer>,              // cleared each frame (rebuilt during render)
    widget_state:   WidgetInputState,        // persists across frames
    input:          InputState,              // current frame
    frame:          u64,
    scoped_regions: Vec<ScopedRegion>,       // persists, updated each frame
}
```

#### Frame cycle

```rust
// 1. Start of frame: clear widget registrations, propagate input to scoped regions
coordinator.begin_frame(input);

// 2. During render: register every widget you draw
coordinator.register("btn_close", close_btn_rect, Sense::CLICK);
coordinator.register("slider_volume", slider_rect, Sense::DRAG);

// 3. End of frame: generate interaction responses
let responses: Vec<(WidgetId, WidgetResponse)> = coordinator.end_frame();

for (id, resp) in responses {
    if id.0 == "btn_close" && resp.clicked { /* ... */ }
    if id.0 == "slider_volume" && resp.dragged { /* ... */ }
}
```

#### Widget registration

```rust
// Register on the default main layer
coordinator.register(id, rect, sense);

// Register on a specific layer (modals, popups, tooltips)
coordinator.register_on_layer(id, rect, sense, &LayerId::modal());
```

### `LayerId` — z-order layering

```rust
pub struct LayerId(pub String);

// Four predefined layers (z-order: main=0, modal=1, popup=2, tooltip=3)
LayerId::main()     // z=0, base UI
LayerId::modal()    // z=1, modal dialogs
LayerId::popup()    // z=2, dropdowns, context menus
LayerId::tooltip()  // z=3, hover tooltips
```

Push a layer before registering widgets on it:

```rust
coordinator.push_layer(LayerId::modal(), z_order: 1, modal: true);
coordinator.register_on_layer("modal:close_btn", close_rect, Sense::CLICK, &LayerId::modal());
```

The `modal: true` flag blocks all hit tests on lower layers. Any click that does not land on a widget registered to the modal layer or above returns `None` from hit testing. Use this for "click outside to close" patterns:

```rust
if coordinator.is_point_in_modal_layer(click_x, click_y) {
    // Point hit the modal's blocking zone but not any widget on it
    close_modal();
}

// Or for drag/panel-separator blocking:
if coordinator.is_blocked_by_modal(mouse_x, mouse_y) {
    // Do not pass this drag through to panel splitters below the modal
}
```

Hit test z-order resolution: layers are sorted descending by `z_order`. Within a layer, the last registered widget wins (painter's model — last drawn = on top).

### `ScopedRegion` — panel-local namespaces

A `ScopedRegion` wraps its own `InputCoordinator` and translates parent (screen) coordinates into region-local coordinates. This lets a sub-panel (e.g., a chart toolbar) own its widget namespace without polluting the global coordinator.

```rust
pub struct ScopedRegion {
    pub rect:        Rect,             // bounding box in screen coordinates
    pub coordinator: InputCoordinator, // child coordinator (local coordinates)
    pub id:          String,
}
```

Widget IDs from scoped regions are returned with a `"{region_id}:"` prefix in `end_frame` responses.

#### Using scoped regions

```rust
// Register or retrieve the region (updates rect if it already exists)
let chart_coord = coordinator.push_scoped_region("chart_toolbar", toolbar_rect);

// Register widgets using LOCAL coordinates (origin at toolbar top-left)
chart_coord.register("btn_zoom_in",  Rect::new(4.0, 4.0, 32.0, 32.0), Sense::CLICK);
chart_coord.register("btn_zoom_out", Rect::new(40.0, 4.0, 32.0, 32.0), Sense::CLICK);

// Responses come back with prefixed IDs
for (id, resp) in coordinator.end_frame() {
    // id.0 == "chart_toolbar:btn_zoom_in"
    if id.0 == "chart_toolbar:btn_zoom_in" && resp.clicked { zoom_in(); }
}
```

Pointer propagation: `begin_frame` automatically translates the screen-space pointer into region-local coordinates for each scoped region. If the pointer is outside the region's bounding rect, the child coordinator receives `pointer.pos = None` so widgets inside report no hover.

Query with prefix:

```rust
// Check hover on a scoped widget by prefixed ID
let is_hovered = coordinator.is_hovered(&WidgetId::new("chart_toolbar:btn_zoom_in"));
```

Remove a region when the panel is closed:

```rust
coordinator.remove_scoped_region("chart_toolbar");
```

### `Sense` — interaction flags

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Sense {
    pub click:  bool,
    pub drag:   bool,
    pub hover:  bool,
    pub focus:  bool,
}
```

Predefined constants:

| Constant | click | drag | hover | focus | Use for |
|----------|-------|------|-------|-------|---------|
| `Sense::NONE` | | | | | decorative/invisible |
| `Sense::HOVER` | | | Y | | tooltips, highlight-only |
| `Sense::CLICK` | Y | | Y | | buttons, checkboxes |
| `Sense::DRAG` | | Y | Y | | sliders, scrollbars |
| `Sense::CLICK_AND_DRAG` | Y | Y | Y | | items with both interactions — adds latency |
| `Sense::FOCUSABLE` | | | Y | Y | keyboard-navigable inputs |
| `Sense::ALL` | Y | Y | Y | Y | full interaction |

Builder API:

```rust
// Start from NONE and add flags
let sense = Sense::none()
    .with_click()
    .with_drag()
    .with_focus();

// Combine with | operator
let sense = Sense::CLICK | Sense::DRAG;

// In-place
let mut sense = Sense::CLICK;
sense |= Sense::DRAG;
```

Query predicates:

```rust
sense.interactive()     // click || drag || focus
sense.has_click_and_drag() // click && drag (has latency warning)
sense.is_passive()      // purely hover or none — no keyboard/click/drag
```

### `WidgetResponse` — per-frame interaction result

Every interactive widget should consult `WidgetResponse` to determine what happened to it this frame.

```rust
pub struct WidgetResponse {
    pub id:             WidgetId,
    pub rect:           WidgetRect,
    pub sense:          Sense,

    // hover
    pub hovered:        bool,
    pub hover_started:  bool,   // entered this frame
    pub hover_ended:    bool,   // left this frame

    // click (gated by Sense::click)
    pub clicked:        bool,
    pub double_clicked: bool,
    pub triple_clicked: bool,
    pub right_clicked:  bool,
    pub middle_clicked: bool,

    // drag (gated by Sense::drag)
    pub drag_started:   bool,
    pub dragged:        bool,
    pub drag_stopped:   bool,
    pub drag_delta:     (f64, f64),  // delta since last frame
    pub drag_total:     (f64, f64),  // total delta since drag began

    // focus (gated by Sense::focus)
    pub has_focus:      bool,
    pub gained_focus:   bool,
    pub lost_focus:     bool,

    // value change
    pub changed:        bool,

    // enabled
    pub enabled:        bool,
}
```

Builder methods (for constructing responses in custom widgets):

```rust
let resp = WidgetResponse::new(id, rect, Sense::CLICK)
    .with_hover(true)
    .with_click()
    .with_focus(false)
    .with_changed()
    .disabled();
```

Aggregation:

```rust
// Logical OR across two responses (rect and id taken from left)
let combined = resp_a.union(resp_b);
let combined = resp_a | resp_b;  // operator alias
```

Convenience predicates:

```rust
resp.any_click()        // clicked || right_clicked || middle_clicked
resp.interacted()       // any_click() || drag_started || gained_focus
resp.is_active()        // dragged || has_focus
resp.is_pointer_over()  // hovered || dragged
```

### Hit testing flow

`end_frame` performs hit testing in this order:

1. **Scoped regions** — iterated in registration order (last = top). Each region's child coordinator runs its own hit test in region-local coordinates. Matched IDs are prefixed and included first in the response list.
2. **Global layers** — sorted descending by `z_order`. Within a layer, the last registered widget wins. If a modal layer is active and the pointer does not land on a widget at or above the modal z-level, the hit returns `None`.
3. **Hover state tracking** — `WidgetInputState.hover` is updated; `hover_started` / `hover_ended` transitions are derived.
4. **Drag tracking** — if a drag is in progress and the button remains held, a `dragged` response is emitted. On button release, `drag_stopped`.
5. **Scroll delivery** — if `scroll_delta != (0, 0)` and the hovered widget has `Sense::DRAG`, it receives a response.

### Focus management

```rust
coordinator.set_focus("my_input");
coordinator.clear_focus();
coordinator.focus_next();   // Tab: advances through Sense::FOCUSABLE widgets in registration order
coordinator.focus_prev();   // Shift+Tab: reverses

coordinator.focused_widget() -> Option<&WidgetId>
coordinator.is_focused(&id) -> bool
```

### Utility queries

```rust
// Is the cursor over any registered UI widget?
// Chart canvas is NOT registered, so this cleanly separates "UI" from "chart".
coordinator.is_over_ui() -> bool

// Z-order of the layer under the cursor
coordinator.hovered_widget_z_order() -> Option<u32>
coordinator.hovered_widget_layer_id() -> Option<LayerId>

// Imperative click dispatch (for platforms that report clicks as separate events)
coordinator.process_click(x, y) -> Option<WidgetId>
```

---

## 4. Layout

**Directories:** `src/layout/`, `src/layout_helpers/`

The layout engine computes widget rectangles from a declarative `LayoutNode` tree. It runs once per frame inside `Context::begin_frame` and writes results into a flat `HashMap<WidgetId, LayoutComputed>`.

### Core types

#### `LayoutNode`

```rust
pub struct LayoutNode {
    pub id:       WidgetId,
    pub kind:     LayoutKind,   // Container | Widget | Overlay
    pub style:    LayoutStyle,
    pub children: Vec<LayoutNode>,
    pub flags:    LayoutFlags,
}
```

Constructed with a builder API:

```rust
let node = LayoutNode::new("my_panel")
    .with_kind(LayoutKind::Container)
    .with_style(LayoutStyle {
        display:   Display::Flex,
        direction: FlexDirection::Column,
        height:    SizeSpec::Fill,
        width:     SizeSpec::Fill,
        padding:   Insets::all(8.0),
        gap:       4.0,
        ..Default::default()
    })
    .with_flags(LayoutFlags::CLIP_CONTENT)
    .with_child(header_node)
    .with_child(body_node);
```

#### `LayoutStyle`

```rust
pub struct LayoutStyle {
    pub display:         Display,       // Flex | Stack | Grid | None
    pub direction:       FlexDirection, // Row | Column
    pub align_items:     AlignItems,    // Stretch | Start | End | Center
    pub justify_content: JustifyContent,// Start|End|Center|SpaceBetween|SpaceAround|SpaceEvenly

    pub position: Position, // Relative (flow) | Absolute (out-of-flow)

    pub gap:     f64,
    pub padding: Insets,
    pub margin:  Insets,

    pub width:      SizeSpec,
    pub height:     SizeSpec,
    pub min_width:  Option<f64>,
    pub max_width:  Option<f64>,
    pub min_height: Option<f64>,
    pub max_height: Option<f64>,

    pub offset_x: f64,  // additional translation applied after margin
    pub offset_y: f64,
    pub z_index:  i32,
}
```

#### `SizeSpec`

```rust
pub enum SizeSpec {
    Fix(f64),   // fixed pixel size
    Pct(f64),   // fraction of parent (0.0–1.0)
    Fill,       // consume remaining flex space (flex-grow: 1)
    Content,    // size to content — currently treated as Fix(0) for rows, Fix(30) for columns
}
```

`Fill` is the primary layout primitive. In a `Column` flex container with two children, one `Fix(50)` header and one `Fill` content area, the content area gets all remaining space:

```
total_height - 50px - gap = content height
```

#### `Insets`

```rust
pub struct Insets { pub top: f64, pub right: f64, pub bottom: f64, pub left: f64 }

Insets::all(8.0)              // uniform
Insets::symmetric(v, h)       // (top=bottom=v, left=right=h)
insets.width()  // left + right
insets.height() // top + bottom
```

#### `LayoutFlags`

Bit flags controlling node behavior:

```rust
LayoutFlags::NONE
LayoutFlags::CLIP_CONTENT  // clip children to this node's content rect
LayoutFlags::SCROLL_Y      // (declarative; physics handled by Context::scroll_area)
LayoutFlags::SCROLL_X
LayoutFlags::IS_ROOT
```

Combine with `|`:

```rust
let flags = LayoutFlags::CLIP_CONTENT | LayoutFlags::SCROLL_Y;
```

### `LayoutTree` and `compute`

```rust
pub struct LayoutTree {
    pub root:     LayoutNode,
    pub computed: HashMap<WidgetId, LayoutComputed>,
}
```

`compute` is called every frame by `Context::begin_frame`:

```rust
pub fn compute(&mut self, viewport: Rect) {
    self.computed.clear();
    // Recursively visits every node in the tree
    layout_node_at(&self.root, viewport, &mut ctx, z=0, clip=None);
}
```

`LayoutComputed` holds the result:

```rust
pub struct LayoutComputed {
    pub rect:         WidgetRect, // absolute position + size (screen coordinates)
    pub content_rect: WidgetRect, // rect minus padding
    pub clip_rect:    Option<WidgetRect>, // intersection of all ancestor clips
    pub z_order:      i32,        // accumulated z_index from root
}
```

#### Algorithm

For each node the engine:

1. **Applies margin and `offset_x`/`offset_y`** to produce the border box within the rect provided by the parent.
2. **Subtracts padding** to produce `content_rect`.
3. **Clips** — if `CLIP_CONTENT`, intersects the current clip with `content_rect` and propagates it down.
4. **Accumulates z-order** — `z_order = parent_z + node.style.z_index`.
5. **Stores** the result in `computed`.
6. **Lays out children** based on `display`:
   - `Flex` → `layout_flex`
   - `Stack` → `layout_stack`
   - `None` → skip all children

#### Flex layout

Flex distributes space along the main axis (`direction`):

1. Measure fixed-size children (`Fix`, `Pct`).
2. Count `Fill` children.
3. Add gaps: `(visible_children - 1) * gap`.
4. `flex_unit = (available - total_fixed) / fill_count`.
5. Walk children left-to-right (Row) or top-to-bottom (Column), advancing a cursor by each child's computed main-axis size plus gap.

Cross-axis behavior:
- `Fill` cross-axis → stretches to the content rect cross dimension.
- `Content` cross-axis → same stretch (placeholder; no measure pass yet).

Absolute children (`Position::Absolute`) are placed relative to the parent's content rect at `z + 100` and do not contribute to the flex cursor.

#### Stack layout

```
Display::Stack → all children receive the full content_rect
```

Children overlap. Z-order increments by `child.style.z_index + index` so later children in the slice are rendered on top.

#### Grid layout

`Display::Grid` currently falls back to `layout_flex`. The `grid_layout` helper in `layout_helpers` provides manual grid computation instead (see below).

### Computed layout access

```rust
// Via LayoutTree directly
let rect: Option<Rect> = tree.get_rect(&WidgetId::new("my_panel"));
let comp: Option<&LayoutComputed> = tree.get_computed(&WidgetId::new("my_panel"));

// Via Context (returns Rect::default() on miss)
let rect: Rect = ctx.widget_rect(&WidgetId::new("my_panel"));
```

### Example: toolbar + scrollable list

```rust
let root = LayoutNode::new("root")
    .with_style(LayoutStyle {
        display:   Display::Flex,
        direction: FlexDirection::Column,
        width:     SizeSpec::Fill,
        height:    SizeSpec::Fill,
        ..Default::default()
    })
    .with_child(
        LayoutNode::new("toolbar")
            .with_style(LayoutStyle {
                height: SizeSpec::Fix(40.0),
                width:  SizeSpec::Fill,
                padding: Insets::symmetric(4.0, 8.0),
                ..Default::default()
            })
    )
    .with_child(
        LayoutNode::new("list_panel")
            .with_style(LayoutStyle {
                display:   Display::Flex,
                direction: FlexDirection::Column,
                height:    SizeSpec::Fill,
                width:     SizeSpec::Fill,
                ..Default::default()
            })
            .with_flags(LayoutFlags::CLIP_CONTENT | LayoutFlags::SCROLL_Y)
    );
```

### Layout helpers (`layout_helpers/`)

Standalone functions that compute rects without a tree. Useful for imperative layout inside a render callback where you already have a rect and need to position children.

**Re-exported from `uzor::layout_helpers::*`:**

#### Alignment (`helpers/alignment.rs`)

```rust
// Center a child inside a parent
center_rect(parent: Rect, child_width: f64, child_height: f64) -> Rect

// Align to edges with margin
align_left(parent, child_width, child_height, margin)  -> Rect
align_right(parent, child_width, child_height, margin) -> Rect
align_top(parent, child_width, child_height, margin)   -> Rect
align_bottom(parent, child_width, child_height, margin)-> Rect
```

Example — centering a modal close button in the modal's top-right area:

```rust
let close_btn = align_right(modal_rect, 24.0, 24.0, 8.0);
```

#### Stacking and distribution (`helpers/layout.rs`)

```rust
// Generate N equal-height rects stacked vertically
stack_vertical(container: Rect, item_height: f64, spacing: f64, count: usize) -> Vec<Rect>

// Generate N equal-width rects stacked horizontally
stack_horizontal(container: Rect, item_width: f64, spacing: f64, count: usize) -> Vec<Rect>

// Create a rows x cols grid with uniform cell size
grid_layout(container: Rect, cols: usize, rows: usize, spacing: f64) -> Vec<Rect>

// Divide container width evenly among item_count slices (no spacing)
distribute_space(container: Rect, item_count: usize) -> Vec<Rect>
```

Example — a 5-item toolbar divided evenly:

```rust
let buttons = distribute_space(toolbar_rect, 5);
for (i, rect) in buttons.iter().enumerate() {
    render_toolbar_button(renderer, rect, TOOLBAR_LABELS[i]);
}
```

Example — a 4×3 thumbnail grid:

```rust
let cells = grid_layout(panel_rect, cols: 4, rows: 3, spacing: 8.0);
for (cell, thumb) in cells.iter().zip(thumbnails.iter()) {
    render_thumbnail(renderer, *cell, thumb);
}
```

#### Sizing (`helpers/sizing.rs`)

```rust
// Derive height from width and aspect ratio
aspect_ratio(width: f64, ratio: f64) -> (f64, f64)   // returns (width, height)

// Scale content to fit within bounds while preserving aspect ratio
fit_in_bounds(content_w: f64, content_h: f64, max_w: f64, max_h: f64) -> (f64, f64)

// Compute centered modal rect in screen
modal_rect(screen_w: f64, screen_h: f64, modal_w: f64, modal_h: f64) -> Rect
```

Example — centering a 640×480 dialog in a 1920×1080 window:

```rust
let dialog_rect = modal_rect(1920.0, 1080.0, 640.0, 480.0);
// → Rect { x: 640.0, y: 300.0, width: 640.0, height: 480.0 }
```

Example — fitting a 4:3 image into a 200×150 thumbnail slot:

```rust
let (w, h) = fit_in_bounds(1920.0, 1440.0, 200.0, 150.0);
// → (200.0, 150.0) — fits perfectly since ratios match
// If image were 1920×1080: (200.0, 112.5) — letterboxed
```

---

## Integration example: minimal frame loop

```rust
use uzor::{
    Context,
    layout::{LayoutNode, LayoutStyle, Display, FlexDirection, SizeSpec},
    input::InputState,
    types::Rect,
    input::{InputCoordinator, LayerId},
    input::sense::Sense,
};

struct App {
    ctx: Context,
    coord: InputCoordinator,
    counter: u32,
}

impl App {
    fn new() -> Self {
        let root = LayoutNode::new("root")
            .with_style(LayoutStyle {
                display:   Display::Flex,
                direction: FlexDirection::Column,
                width:     SizeSpec::Fill,
                height:    SizeSpec::Fill,
                ..Default::default()
            })
            .with_child(
                LayoutNode::new("increment_btn")
                    .with_style(LayoutStyle {
                        height: SizeSpec::Fix(40.0),
                        width:  SizeSpec::Fix(120.0),
                        ..Default::default()
                    })
            );

        Self {
            ctx: Context::new(root),
            coord: InputCoordinator::new(),
            counter: 0,
        }
    }

    fn tick(&mut self, input: InputState, viewport: Rect, renderer: &mut dyn Renderer) {
        // 1. Begin frame
        self.ctx.begin_frame(input.clone(), viewport);
        self.coord.begin_frame(input);

        // 2. Get layout results
        let btn_rect = self.ctx.widget_rect(&"increment_btn".into());

        // 3. Register widgets with coordinator
        self.coord.register("increment_btn", btn_rect, Sense::CLICK);

        // 4. Render
        let btn_resp = self.ctx.button("increment_btn");
        renderer.rect(btn_rect, btn_resp.state.into());
        renderer.text(btn_rect, &format!("Count: {}", self.counter));

        // 5. Process responses
        let responses = self.coord.end_frame();
        for (id, resp) in responses {
            if id.0 == "increment_btn" && resp.clicked {
                self.counter += 1;
            }
        }
    }
}
```

Note: `Context::button` and `InputCoordinator::end_frame` both detect clicks, but serve different purposes. `Context::button` is a lightweight per-widget convenience for simple cases. `InputCoordinator::end_frame` is the authoritative response when using layering, scoped regions, drag tracking, or multi-widget routing.

---

*Part 2 of this guide covers rendering (`render/`), panels (`panels/` and `panel_api/`), widgets, containers, and animation in depth.*
