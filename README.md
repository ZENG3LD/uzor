# UZOR

> **Universal Headless UI Framework for Rust**
> Geometry + Interaction + Multi-Backend Rendering

[![Crates.io](https://img.shields.io/crates/v/uzor.svg)](https://crates.io/crates/uzor)
[![docs.rs](https://docs.rs/uzor/badge.svg)](https://docs.rs/uzor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/zeng3ld/uzor#license)

## What is UZOR?

UZOR is a **low-level UI framework** that handles geometry, interaction detection, and provides a Canvas2D-style rendering trait — while giving applications **full control** over visuals and business logic.

**Think of UZOR like a physics engine for UI:** it calculates geometry and detects interactions, your app renders whatever it wants.

### What UZOR Does

1. **Rect Management** — Creates and tracks widget geometry
2. **Interaction Detection** — Hover, click, drag, scroll on rects
3. **RenderContext Trait** — Canvas2D-style API with multiple backend implementations
4. **Layout Helpers** — Alignment, stacking, grid layout utilities
5. **Panel System** — Dockable, resizable panel management
6. **Animation Engine** — Spring physics, easing, coordinated animations
7. **UI Widgets** — Buttons, dropdowns, sliders, text inputs, toasts, and more

### What UZOR Does NOT Do

- Does NOT enforce visual style (your app decides colors, borders, effects)
- Does NOT contain business logic (your app owns state and handlers)
- Does NOT make rendering decisions (your app uses RenderContext to draw)

## Documentation

Detailed technical reference is in [`uzor/docs/`](uzor/docs/):

| Guide | Contents |
|-------|----------|
| [01-core.md](uzor/docs/01-core.md) | Context, Types (Rect, WidgetId), Input System (InputCoordinator, z-order layers, Sense, hit testing), Layout engine |
| [02-render-widgets.md](uzor/docs/02-render-widgets.md) | RenderContext trait (all methods), SVG rendering, Icon system, all Widgets (button, dropdown, slider, text input, toast, etc.) |
| [03-panels-animation.md](uzor/docs/03-panels-animation.md) | Docking panel system (DockingManager, tabs, drag-and-drop, floating windows), Animation engine (spring, easing, decay, timeline, recipes) |
| [04-fx-themes-platform.md](uzor/docs/04-fx-themes-platform.md) | macOS theme system, Interactive/Text/Cursor/Scroll effects, Number animation, Platform abstraction, State registry |

## Quick Start

```toml
[dependencies]
uzor = "1.0"                    # core library (zero platform deps)
uzor-backend-vello-gpu = "1.0"  # pick a rendering backend
uzor-desktop = "1.0"            # pick a platform handler
```

## Architecture

UZOR is organized into three layers:

```
┌─────────────────────────────────────────────────────┐
│  Your Application                                   │
│  - Owns all visual rendering                        │
│  - Owns business logic and state                    │
│  - Uses uzor for geometry + interaction + widgets   │
│  - Uses RenderContext to draw                       │
└─────────────────────────────────────────────────────┘
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │   uzor     │ │  Backends  │ │ Platforms  │
   │ (core lib) │ │ (renderers)│ │ (handlers) │
   └────────────┘ └────────────┘ └────────────┘
```

### Core: `uzor`

Single crate, zero platform dependencies. Contains everything:

| Module | Description |
|--------|-------------|
| `uzor::input` | Input state, event processing, interaction detection |
| `uzor::render` | `RenderContext` trait (Canvas2D-style API) |
| `uzor::widgets` | Buttons, dropdowns, sliders, text inputs, toasts |
| `uzor::panels` | Dockable panel system with drag, resize, tabs |
| `uzor::animation` | Spring physics, easing, timeline, coordinated recipes |
| `uzor::layout_helpers` | Alignment, stacking, sizing utilities |
| `uzor::macos` | macOS-style widget themes and colors |
| `uzor::interactive` | Elastic sliders, spotlights, animated lists |
| `uzor::text_fx` | Text effects: decrypt, fuzzy, gradient, shiny |
| `uzor::cursor` | Cursor effects: blob, click spark, glare, magnet |
| `uzor::numbers` | Animated number counters |
| `uzor::scroll_fx` | Scroll effects: float, reveal, velocity |

### Backends (renderers)

Each backend implements the `RenderContext` trait:

| Crate | GPU? | Platform | Notes |
|-------|------|----------|-------|
| `uzor-backend-vello-gpu` | Yes | Desktop/WASM | vello 0.6 + wgpu |
| `uzor-backend-vello-cpu` | No | Desktop | vello_cpu, pure software |
| `uzor-backend-vello-hybrid` | Mixed | Desktop | vello_hybrid |
| `uzor-backend-tiny-skia` | No | Desktop | tiny-skia + fontdue |
| `uzor-backend-canvas2d` | No | WASM | Browser Canvas2D via web-sys |
| `uzor-backend-wgpu-instanced` | Yes | Desktop | Instanced quads/lines/text, 3 draw calls |

Shared vello utilities live in `uzor-backend-vello-common`.

### Platforms (handlers)

Window management and event loop integration:

| Crate | Platform | Dependencies |
|-------|----------|--------------|
| `uzor-desktop` | Desktop | winit |
| `uzor-web` | Browser/WASM | web-sys |
| `uzor-mobile` | iOS/Android | winit (mobile) |
| `uzor-tui` | Terminal | crossterm |

## Example

```rust
use uzor::{Context, WidgetId, Rect, WidgetState};

// 1. Register a rect with UZOR
let button_id = WidgetId::new("my_button");
let btn_rect = Rect::new(100.0, 50.0, 200.0, 40.0);
ctx.layout.computed.insert(button_id.clone(), btn_rect);

// 2. Query interaction state (UZOR does NOT render)
let response = ctx.icon_button(button_id);

// 3. YOUR app decides how to render based on state
let color = match response.state {
    WidgetState::Pressed => "#2563eb",
    WidgetState::Hovered => "#3b82f6",
    WidgetState::Normal  => "#60a5fa",
};

// 4. Draw using any RenderContext backend
render_ctx.set_fill_color(color);
render_ctx.fill_rect(response.rect.x, response.rect.y, 200.0, 40.0);
```

## `RenderContext` Trait

The rendering trait follows the Canvas2D API pattern:

```rust
pub trait RenderContext {
    // State
    fn save(&mut self);
    fn restore(&mut self);

    // Shapes
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64);
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64);
    fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64);

    // Paths
    fn begin_path(&mut self);
    fn move_to(&mut self, x: f64, y: f64);
    fn line_to(&mut self, x: f64, y: f64);
    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64);
    fn fill(&mut self);
    fn stroke(&mut self);

    // Style
    fn set_fill_color(&mut self, color: &str);
    fn set_stroke_color(&mut self, color: &str);
    fn set_stroke_width(&mut self, width: f64);
    fn set_global_alpha(&mut self, alpha: f64);

    // Text
    fn fill_text(&mut self, text: &str, x: f64, y: f64);
    fn measure_text(&self, text: &str) -> f64;
    fn set_font(&mut self, font: &str);

    // ... and more
}
```

All coordinates are `f64`, colors are CSS hex strings. Any backend that implements this trait is plug-and-play.

## Design Principles

1. **Core is rendering-agnostic** — no platform dependencies in `uzor`
2. **Applications control visuals** — UZOR never enforces style
3. **Backends are isolated** — each is a separate crate with its own deps
4. **Pick your level of control** — use core directly, or widgets, or both

## Support the Project

If you find this library useful, consider supporting development:

| Currency | Network | Address |
|----------|---------|---------|
| USDT | TRC20 | `TNxMKsvVLYViQ5X5sgCYmkzH4qjhhh5U7X` |
| USDC | Arbitrum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| ETH | Ethereum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| BTC | Bitcoin | `bc1qjgzthxja8umt5tvrp5tfcf9zeepmhn0f6mnt40` |
| SOL | Solana | `DZJjmH8Cs5wEafz5Ua86wBBkurSA4xdWXa3LWnBUR94c` |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

---

<p align="center">
  <img src="assets/author.svg" alt="zengeld" />
</p>
