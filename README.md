# UZOR

> Geometry, interaction, and multi-backend rendering for Rust.

[![Crates.io](https://img.shields.io/crates/v/uzor.svg)](https://crates.io/crates/uzor)
[![docs.rs](https://docs.rs/uzor/badge.svg)](https://docs.rs/uzor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/zeng3ld/uzor#license)

UZOR is a low-level UI stack: it owns widget geometry, hit-testing, and a Canvas2D-style `RenderContext`. The app owns visuals and business logic.

The workspace is one version (**1.5.1**). Every crate below publishes at that number.

## Crate map

**Core**

| Crate | Role |
|-------|------|
| `uzor` | Engine: input, layout, panels, widgets, animation, `RenderContext` |
| `uzor-fonts` | Bundled fonts |
| `uzor-icon` | SVG → PNG/ICO helpers |
| `uzor-framework-macros` | `view!` DSL |
| `uzor-agent-api` | Local HTTP control plane for live apps |

**Windows**

| Crate | Role |
|-------|------|
| `uzor-window-desktop` | winit desktop |
| `uzor-window-web` | WASM / browser |
| `uzor-window-mobile` | iOS / Android |
| `uzor-desktop` | Desktop runtime (`AppBuilder::run`) |
| `uzor-tui` | Cell-buffer TUI (crossterm). Feature `ascii` pulls `uzor-text` |

**Render backends** (`RenderContext` implementations)

| Crate | Role |
|-------|------|
| `uzor-render-vello-gpu` | vello + wgpu |
| `uzor-render-vello-cpu` | software vello |
| `uzor-render-vello-hybrid` | hybrid vello |
| `uzor-render-tiny-skia` | tiny-skia CPU |
| `uzor-render-wgpu-instanced` | instanced wgpu quads/lines/text |
| `uzor-render-canvas2d` | browser Canvas2D |
| `uzor-render-svg` | serialize draws to standalone SVG |
| `uzor-render-hub` | pick a backend, submit frames |
| `uzor-render-urx` | `RenderContext` → URX `Scene` adapter |

**URX** (own scene + raster family, not the old vello path)

| Crate | Role |
|-------|------|
| `uzor-urx-core` | Scene, dirty state, metrics |
| `uzor-urx-glyph` | Glyph atlas / raster |
| `uzor-urx-image` | Shared `ImageId` → RGBA8 store |
| `uzor-urx-cpu` | CPU scanline raster |
| `uzor-urx-wgpu` | Instanced wgpu consumer |
| `uzor-urx-hybrid` | CPU tiles + GPU composite |
| `uzor-urx-wgpu-full` | Full-GPU compute pipeline |
| `uzor-urx-engine` | Dirty regions, cadence, dispatch |
| `uzor-urx-region-mixer` | Per-region CPU/GPU split |
| `uzor-urx-3d` | Native 3D (camera, depth, glTF) |
| `uzor-urx-physics` | AABB/sphere + simple dynamics |

**Content engines**

| Crate | Role |
|-------|------|
| `uzor-text` | Paragraph layout, hyphenation, ASCII cell shaders |
| `uzor-figures` | Charts: scales, marks, bar/curve/heatmap/dag/… |
| `uzor-figures-registry` | `.figure.json` → typed spec → render |
| `uzor-typeset` | Pages / slides / frames over text + figures |
| `uzor-export` | Headless PNG / SVG / PDF |
| `uzor-graph` | Force-directed graphs (2D + 3D) |
| `uzor-proof-harness` | Same draw closure through every rasterizer; pixel-diff |

**Demos:** `uzor-examples`.

## Quick start

```toml
[dependencies]
uzor = "1.5"
uzor-render-vello-gpu = "1.5"
uzor-desktop = "1.5"
```

Core feature flags: `serde`, `animation`, `shaper` (cosmic-text).

Technical notes for the kernel live in [`uzor/docs/`](uzor/docs/).

## Support

| Currency | Network | Address |
|----------|---------|---------|
| USDT | TRC20 | `TNxMKsvVLYViQ5X5sgCYmkzH4qjhhh5U7X` |
| USDC | Arbitrum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| ETH | Ethereum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| BTC | Bitcoin | `bc1qjgzthxja8umt5tvrp5tfcf9zeepmhn0f6mnt40` |
| SOL | Solana | `DZJjmH8Cs5wEafz5Ua86wBBkurSA4xdWXa3LWnBUR94c` |

## License

MIT OR Apache-2.0. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).

---

<p align="center">
  <img src="assets/author.svg" alt="zengeld" />
</p>
