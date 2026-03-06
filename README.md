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

### What UZOR Does NOT Do

- Does NOT enforce visual style (your app decides colors, borders, effects)
- Does NOT contain business logic (your app owns state and handlers)
- Does NOT make rendering decisions (your app uses RenderContext to draw)

## Quick Start

```toml
# Cargo.toml — the `uzor` facade crate re-exports everything
[dependencies]
uzor = "1.0"  # default: vello-gpu backend

# Or pick a specific backend:
uzor = { version = "1.0", default-features = false, features = ["tiny-skia"] }
uzor = { version = "1.0", default-features = false, features = ["vello-cpu"] }
```

## Rendering Backends

UZOR's `RenderContext` trait is a pure Canvas2D-style API (33 methods, zero dependencies). Multiple backends implement it:

| Backend | Crate | GPU? | Platform | Notes |
|---------|-------|------|----------|-------|
| **Vello GPU** | `uzor-backend-vello-gpu` | Yes | Desktop/WASM | Default. vello 0.6 + wgpu |
| **Vello CPU** | `uzor-backend-vello-cpu` | No | Desktop | vello_cpu 0.0.6, pure software |
| **Vello Hybrid** | `uzor-backend-vello-hybrid` | Mixed | Desktop | vello_hybrid 0.0.6 |
| **tiny-skia** | `uzor-backend-tiny-skia` | No | Desktop | tiny-skia 0.11 + fontdue |
| **Canvas2D** | `uzor-backend-canvas2d` | No | WASM | Browser Canvas2D via web-sys |

### Feature Flags

```toml
# Individual backends
uzor = { version = "1.0", features = ["vello-gpu"] }     # default
uzor = { version = "1.0", features = ["vello-cpu"] }
uzor = { version = "1.0", features = ["vello-hybrid"] }
uzor = { version = "1.0", features = ["tiny-skia"] }
uzor = { version = "1.0", features = ["canvas2d"] }

# Convenience groups
uzor = { version = "1.0", features = ["all-cpu"] }       # vello-cpu + tiny-skia
uzor = { version = "1.0", features = ["all-gpu"] }       # vello-gpu + vello-hybrid
uzor = { version = "1.0", features = ["all-wasm"] }      # canvas2d
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Your Application                                   │
│  - Owns all visual rendering                        │
│  - Owns business logic and state                    │
│  - Uses uzor-core for geometry + interaction        │
│  - Uses RenderContext to draw                       │
└─────────────────────────────────────────────────────┘
                         |
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │ uzor-core  │ │uzor-layout │ │uzor-render │
   │ Geometry   │ │ Helpers    │ │ Trait API  │
   │ Interaction│ │ (optional) │ │            │
   └────────────┘ └────────────┘ └─────┬──────┘
                                       │
                    ┌──────────────┬────┴────┬──────────────┐
                    ▼              ▼         ▼              ▼
             ┌───────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
             │ vello-gpu │ │vello-cpu │ │tiny-skia │ │canvas2d  │
             └───────────┘ └──────────┘ └──────────┘ └──────────┘
```

## Example

```rust
use uzor_core::{Context, WidgetId, Rect, WidgetState};

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

## Crate Map

| Crate | Description |
|-------|-------------|
| `uzor` | Facade — re-exports core + render + feature-gated backends |
| `uzor-core` | Core engine: geometry, interaction, input state |
| `uzor-render` | `RenderContext` trait (Canvas2D-style, zero deps) |
| `uzor-layout` | Layout helpers: alignment, stacking, grid |
| `uzor-animation` | Animation utilities |
| `uzor-desktop` | Desktop backend (winit integration) |
| `uzor-web` | Web/WASM backend |
| `uzor-backend-vello-gpu` | GPU rendering via vello + wgpu |
| `uzor-backend-vello-cpu` | CPU rendering via vello_cpu |
| `uzor-backend-vello-hybrid` | Hybrid rendering via vello_hybrid |
| `uzor-backend-vello-common` | Shared utilities for vello backends |
| `uzor-backend-tiny-skia` | CPU rendering via tiny-skia |
| `uzor-backend-canvas2d` | Browser Canvas2D via web-sys |

## Design Principles

1. **Core is rendering-agnostic** — no `draw_*` calls in uzor-core
2. **Applications control visuals** — UZOR never enforces style
3. **Backends are isolated** — core has zero platform dependencies
4. **Pick your level of control** — use core directly, or layout helpers, or both

## `RenderContext` Trait

The rendering trait follows the Canvas2D API pattern:

```rust
pub trait RenderContext {
    // State
    fn save(&mut self);
    fn restore(&mut self);
    fn set_transform(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64);

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
    fn set_fill_color(&mut self, color: &str);    // CSS hex: "#ff0000"
    fn set_stroke_color(&mut self, color: &str);
    fn set_line_width(&mut self, width: f64);
    fn set_global_alpha(&mut self, alpha: f64);

    // Text
    fn fill_text(&mut self, text: &str, x: f64, y: f64);
    fn measure_text(&self, text: &str) -> f64;
    fn set_font(&mut self, font: &str);

    // Clipping
    fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    // ... and more
}
```

All coordinates are `f64`, colors are CSS hex strings. Any backend that implements this trait is plug-and-play.

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
  <a href="https://zen-geldmaschine.net/">
    <img src="assets/author.svg" alt="zengeld" />
  </a>
</p>
