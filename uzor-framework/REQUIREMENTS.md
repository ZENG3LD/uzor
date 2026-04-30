# uzor-framework — first-consumer requirements (mirage-launcher)

The mirage VPN launcher attempted to migrate from raw `winit + softbuffer +
uzor::render` to `uzor-framework::AppBuilder`. We rolled back to plan B
(custom chromeless winit) because the following were missing:

## 1. No ready `WindowProvider` for desktop

`AppBuilder::window(provider: Box<dyn WindowProvider>)` is a required call,
but `uzor-window-desktop` only exports its own `Application` + `EventCallback`
API (an alternative runtime). It does not implement
`uzor_window_hub::lifecycle::WindowProvider`. The trait requires:

```text
fn poll_events(&mut self) -> Vec<PlatformEvent>;
fn window_rect(&self) -> Rect;
fn scale_factor(&self) -> f64;
fn request_redraw(&mut self);
fn should_close(&self) -> bool;
fn raw_window_handle(&self) -> Option<RawHandle>;
```

→ Add `uzor_window_desktop::WinitWindowProvider` (or similar) so consumers
  can `.window(Box::new(WinitWindowProvider::new(event_loop)))`.

## 2. No ready `RenderSurfaceFactory`

`AppBuilder::surface_factory(factory)` is required; without it
`Runtime::run()` returns `RuntimeError::SurfaceWiringRequired`.
None of the render crates (`uzor-render-tiny-skia`, `uzor-render-vello-cpu`,
`uzor-render-vello-gpu`, `uzor-render-vello-hybrid`) export a default factory.

→ Add `uzor_render_tiny_skia::TinySkiaSurfaceFactory`,
  `uzor_render_vello_cpu::VelloCpuSurfaceFactory` etc.
  Without them every consumer reinvents the same wiring.

## 3. No public winit ↔ PlatformEvent mapper

`uzor-window-desktop::event_mapper` is internal. A consumer writing their
own `WindowProvider` over winit has to re-implement the full event tree
(keys, mouse, IME, resize, focus, scroll …).

→ Make it `pub use` so a custom `WindowProvider` impl can call
  `event_mapper::map(winit_event) -> PlatformEvent`.

## 4. AppBuilder docs promise more than the codebase delivers

The doc-comment on `AppBuilder::window` says
"e.g. from `uzor-window-desktop`" — but `uzor-window-desktop` does not
provide one. Either ship a provider or update the docs.

## 5. No `examples/` template

A single working `examples/hello_window.rs` that wires `App` →
`AppBuilder::new(app).window(...).backend(...).surface_factory(...).run()`
would resolve 90 % of the friction.

## 6. No chromeless helpers

`AppConfig::decorations = false` puts the app in chromeless mode but
provides nothing for the resulting needs:

- A `WindowControls` widget (minimize / maximize / close).
- A drag-region API (winit's `start_drag_window()` wrapper exposed
  through `LayoutManager` so a 30 px top strip can be marked draggable).
- A default titlebar layout with title text + controls in correct
  Windows / macOS / Linux order.

Every chromeless consumer reinvents these.

## 7. No icon in `AppConfig`

`AppConfig` has `title`, `initial_size`, `dwm_border_color` … but no
`icon: Option<Icon>`. On Windows the icon shows up in taskbar, alt-tab,
the window caption (when chromed), and the systray entry. Without it
the launcher inherits the generic Rust application icon.

→ Add `AppBuilder::icon(bytes_or_path)` that converts to
  `winit::window::Icon` internally.

## 8. No systray / notification API

A desktop launcher typically minimises to tray and shows balloon
notifications ("connection lost", "update available"). Today the
consumer must depend on `tray-icon` directly.

→ First-class `AppBuilder::tray_icon(...)` + `AppBuilder::tray_menu(...)`
  using the `tray-icon` crate under the hood.

## 9. No mention of Windows specifics

Windows-11 DWM border colour exists (`dwm_border_color`), good — but
nothing about `corner_preference` (rounded vs square corners),
`captionless_resize_borders` (8 px invisible resize hit zones around
a chromeless window), `WS_EX_LAYERED` for translucent backgrounds, etc.
For a "draws its own chrome" framework these are table stakes.

---

For the launcher we proceeded with plan B: keep the existing
`winit + softbuffer + uzor::render` stack, add `with_decorations(false)`,
draw our own titlebar / window controls, embed icon via `winres`.
When framework gains items 1, 2, 5, 6, 7 we will reconsider migrating.
