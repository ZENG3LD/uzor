# Uzor Framework — Current State

## Tier breakdown (L1/L2/L3/L4)

| Tier | Crate / Module | Role |
|------|---------------|------|
| **L1** | `uzor::input::core::coordinator` | `InputCoordinator` — raw widget registration, Z-order hit-test, drag/hover/scroll state |
| **L2** | `uzor::ui::widgets::atomic::*` / `composite::*` | Individual widget render/input helpers; `register_layout_manager_*` functions |
| **L3** | `uzor::layout::LayoutManager` + `uzor::app_context::ContextManager` | Macro layout (chrome/edges/dock/overlays), composite state maps, typed handles, `solve()`, `dispatch_click()` |
| **L4** | `uzor-framework` (`App` trait, `Runtime`, `lm::*` builders) | Frame loop, chainable builders, typed `on_*` dispatch hooks |

---

## L3 LayoutManager API

`LayoutManager<P: DockPanel>` is constructed once and lives in `Runtime`.

**One-time setup (in `App::init`):**
```rust
let modal_h = layout.add_modal("settings");   // returns ModalHandle
let popup_h = layout.add_popup("color");
let dd_h    = layout.add_dropdown("file-menu");
let tb_h    = layout.add_toolbar("top-bar");
let sb_h    = layout.add_sidebar("left");
layout.chrome_mut().height = 30.0;
layout.edges_mut().add(EdgeSlot { id: "top-toolbar".into(), side: EdgeSide::Top, thickness: 40.0, visible: true, .. Default::default() });
```

**Per-frame use (in `App::ui`):**
```rust
// 1. Solve macro rects
layout.solve(window_rect);                // produces dock_area, chrome, edge rects, overlay rects

// 2. Reset dispatch table + composite registry
layout.dispatcher_begin_frame();

// 3. Register composites (each call → one CompositeRegistration pushed)
register_layout_manager_modal(layout, render, modal_node_id, &modal_h, view, &settings, kind);
register_layout_manager_chrome(layout, render, parent, view, &settings, kind);

// 4. Route click (auto in runtime, or manual)
if let Some(ev) = layout.dispatch_click(x, y) { match ev { ... } }
// or unified:
match layout.handle_click(pos) {
    ClickOutcome::DismissOverlay(handle) => { /* close overlay */ }
    ClickOutcome::DispatchEvent(ev)      => { /* match ev */ }
    ClickOutcome::Unhandled { .. }       => {}
}
```

**State access via typed handles:**
```rust
layout.modal(&modal_h)          // &ModalState — read-only
layout.modal_mut(&modal_h)      // &mut ModalState — mutate (open/close/title/tab)
layout.modal_rect(&modal_h)     // Option<Rect> — full frame rect
layout.modal_body_rect(&modal_h)// Option<Rect> — inner content area
```

**Overlay Z-order and dismiss:**
```rust
layout.overlays_in_draw_order()       // sorted slice for painter
layout.dismiss_topmost_at(pos)        // returns WidgetId of overlay to close
layout.push_dismiss_frame(DismissFrame { z, rect, overlay_id })
```

---

## L4 Chainable Builders (`lm::*`)

Located in `uzor-framework::widgets::lm`. Entry points shadow underlying
`register_layout_manager_*` calls behind builder structs.

**Modal:**
```rust
lm::modal(&modal_h)
    .title("Settings")
    .resizable(true)
    .rect(Rect::new(200.0, 150.0, 600.0, 400.0))
    .build(&mut layout, &mut render);
```

**Chrome (singleton — no handle):**
```rust
lm::chrome()
    .tabs(&tabs)
    .active_tab("dashboard")
    .cursor((mx, my))
    .time_ms(now_ms)
    .build(&mut layout, &mut render);
```

**Popup (with anchor auto-resolve):**
```rust
lm::popup(&popup_h)
    .anchor_to("toolbar:tb-help")   // resolves widget rect from coordinator
    .build(&mut layout, &mut render);
```

All builders share the same pattern: zero-argument constructor, optional
chainable setters, terminal `.build(layout, render)`. Every field has a
sensible default; callers override only what differs.

---

## Atomic Widgets (`lm::button`, `lm::text`, `lm::checkbox`, `lm::toggle`, `lm::separator`)

Atomics take an explicit `id` + `rect` (no handle — no persisted composite state):

```rust
lm::button("save-btn", btn_rect)
    .text("Save")
    .on_click(|| { self.save(); })      // closure, fires if clicked this frame
    .build(&mut layout, &mut render);

lm::checkbox("dark-mode", cb_rect)
    .bind(&mut self.dark_mode)          // read flag for paint + toggle on click
    .label("Dark mode")
    .build(&mut layout, &mut render);

lm::text("label-1", label_rect, "Hello")
    .color("#cccccc")
    .align(TextAlign::Center)
    .build(&mut layout, &mut render);
```

`.on_click(closure)` and `.bind(&mut T)` are the reactive wiring points —
they call `layout.was_clicked(&self.id)` internally before invoking.

---

## Layout Engine

**Flex types** live in `uzor::app_context::layout::types`:
- `Display::{Flex, Stack, Grid, None}`
- `FlexDirection::{Row, Column}`
- `AlignItems`, `JustifyContent`, `Position`
- `SizeSpec::{Fix(f64), Pct(f64), Fill, Content}`
- `Insets`, `LayoutStyle` (full CSS-flex-like struct)
- `LayoutNode` — tree node with `id`, `kind`, `style`, `children: Vec<LayoutNode>`, `flags`
- `LayoutComputed` — solved `rect`, `content_rect`, `clip_rect`, `z_order`

**When it runs:**
`LayoutManager::solve(window: Rect)` is called at the top of every frame by the runtime.
It calls `solve_layout(window, &chrome, &edges, &mut tree)` (internal),
then `panels.layout(dock_pr)` for the docking pass.
Result stored as `LayoutSolved { chrome, dock_area, floating_area, edges }`.

The flex engine computes `LayoutNode` trees on the `ContextManager` side
(retained-mode). `LayoutManager` holds the macro-level pass; per-widget flex
layout runs inside composites via `ContextManager`.

---

## Widget ID System

```rust
// In uzor::core::types::state:
pub struct WidgetId(pub(crate) String);   // inner String is crate-private

pub fn unsafe_widget_id(s: impl Into<String>) -> WidgetId { WidgetId(s.into()) }
```

Three sanctioned ways to obtain a `WidgetId`:

| Source | How | When |
|--------|-----|------|
| Typed handle | `layout.add_modal("id")` → `ModalHandle { id: WidgetId }` | L3/L4 composites |
| `From<&str>` / `From<String>` | Used inside `uzor` crate only (pub(crate) `new`) | L1/L2 internals |
| `unsafe_widget_id("str")` | Explicit escape hatch (`#[doc(hidden)]`) | L1/L2 legacy, L2-inside-L3 blocks, tests |

**Node handles** (`ModalNode`, `PopupNode`, …) wrap a `LayoutNodeId` (slot in the layout tree).
**State handles** (`ModalHandle`, `PopupHandle`, …) wrap a `WidgetId` (key into LM state maps).
`ModalHandle::id_str()` exposes the raw string for framework-internal builders only.

---

## State Ownership

| Widget class | State lives in | Lifetime |
|--------------|---------------|----------|
| Composites (modal, popup, dropdown, toolbar, sidebar, context_menu) | `LayoutManager` — `HashMap<WidgetId, XxxState>` fields | Persistent across frames |
| Chrome | `LayoutManager::chrome_widget_state: ChromeState` | Persistent |
| Atomic buttons / text / checkbox / separator | Caller-owned (app struct fields for `bind`) | App owns it; LM has no storage |
| Overlay geometry | `LayoutManager::overlays: OverlayStack` | Persistent until cleared |
| Dispatch patterns | `LayoutManager::dispatcher: ClickDispatcher` | Cleared each frame by `dispatcher_begin_frame()` |
| Hit-test registrations | `InputCoordinator::widgets: Vec<RegisteredWidget>` | Cleared each frame by `begin_frame()` |

App-level state (panel model, active tab, open flags) lives in the `App`
struct. The LM's `modals` / `popups` / etc. maps own only the widget's
internal interaction state (open/closed, drag offset, tab index, …).

---

## Frame Loop (`runtime.rs`)

```
Runtime::tick(window, event_loop)
  │
  ├─ FPS cap guard (WaitUntil early return)
  ├─ poll_events → app.on_event(ev) → EventProcessor → InputState
  ├─ layout.solve(window_rect)               // macro rects
  ├─ layout.ctx_mut().begin_frame(input, vp) // retained-mode begin
  ├─ render_state.begin_frame()              // GPU reset
  ├─ layout.set_frame_time_ms(now_ms)
  ├─ app.ui(layout, render_state)            // USER FRAME — register + paint
  ├─ app.route_click(layout, x, y)           // auto left-click dispatch
  │     └─ layout.handle_click(pos)
  │         → DismissOverlay → app.on_dismiss(...)
  │         → DispatchEvent  → app.on_modal_close / on_toolbar_item / ...
  │         → Unhandled      → app.on_unhandled_click(...)
  ├─ layout.ctx_mut().end_frame()            // collect responses
  ├─ submit_frame(render_state, params)      // GPU submit
  └─ input.pointer.clicked = None            // reset per-frame fields
```

`App::route_click` + `App::on_*` hooks eliminate the `match ClickOutcome`
boilerplate in app code. Apps override only the hooks they need.

---

## Friction Points the Macro Should Remove

1. **Manual ID strings for atomics** — `lm::button("save-btn", rect)` requires
   the app to invent and spell a string twice (register + click-match). A
   `view!` macro can derive a stable ID from the structural position.

2. **Explicit rect everywhere** — every atomic and composite call takes an
   explicit `Rect`. Apps must compute or derive pixel geometry manually.
   JSX-style layout with flex children would remove rect arguments for most
   cases.

3. **No children block syntax** — there is no way to express
   "modal contains [button, checkbox, text]" as a tree. Currently the body is
   drawn in a separate imperative block after `.build()`. A `view!` macro could
   let you write:
   ```rust
   view! {
     <modal handle={&modal_h} title="Settings">
       <checkbox id="dark-mode" bind={&mut dark_mode} label="Dark mode"/>
       <button   id="save"      text="Save"  on_click={|| save()}/>
     </modal>
   }
   ```

4. **`ModalSettings` / `PopupSettings` type noise** — callers that want
   defaults still instantiate `ModalSettings::default()` or leave it out and
   get it anyway. The macro can suppress the `settings` param entirely when
   default.

5. **Two-phase init + frame registration** — `layout.add_modal("id")` must be
   called once in `init()` to get a `ModalHandle`, then `lm::modal(&h)...build()`
   every frame. The macro could unify this into a single call-site with
   the handle stored implicitly in a derived field.

6. **`unsafe_widget_id` leaks at L2-inside-L3 boundaries** — any atomic widget
   that needs a raw string id (e.g. inside a scoped region or a blackbox body)
   requires the escape hatch. The macro should generate stable IDs that never
   require `unsafe_widget_id`.

7. **Repetitive `.parent(node_id)` chains** — atomic builders inside a composite
   body must repeat `.parent(modal_node)` on every child. A children block
   implicitly sets the parent.

8. **No static token checking** — `add_modal("settings")` and
   `lm::modal(&h).build(...)` are coupled by a string; mismatches are runtime
   panics. The macro can make the token a compile-time literal.
