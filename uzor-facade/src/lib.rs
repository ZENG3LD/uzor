//! `uzor` — feature-gated facade over the uzor rendering ecosystem.
//!
//! This crate always re-exports the core rendering traits and utilities.
//! Each rendering backend is compiled only when the corresponding feature flag
//! is enabled, so downstream crates pull in exactly the backends they need.
//!
//! # Features
//!
//! | Feature | Backend crate | Context type |
//! |---------|---------------|--------------|
//! | `vello-gpu` *(default)* | `uzor-backend-vello-gpu` | [`VelloGpuRenderContext`] |
//! | `vello-cpu` | `uzor-backend-vello-cpu` | [`VelloCpuRenderContext`] |
//! | `vello-hybrid` | `uzor-backend-vello-hybrid` | [`VelloHybridRenderContext`] |
//! | `tiny-skia` | `uzor-backend-tiny-skia` | [`TinySkiaCpuRenderContext`] |
//! | `canvas2d` | `uzor-backend-canvas2d` | [`Canvas2dRenderContext`] |
//!
//! # Convenience groups
//!
//! - `all-cpu`  — `vello-cpu` + `tiny-skia`
//! - `all-gpu`  — `vello-gpu` + `vello-hybrid`
//! - `all-wasm` — `canvas2d`
//!
//! # Example
//!
//! ```toml
//! # Cargo.toml — only the GPU backend
//! [dependencies]
//! uzor = { path = "...", features = ["vello-gpu"] }
//!
//! # CPU-only (e.g. for headless rendering / tests)
//! uzor = { path = "...", features = ["vello-cpu"], default-features = false }
//! ```

// --- Always-available re-exports -------------------------------------------------

pub use uzor_core::render::{
    RenderContext, RenderContextExt, RenderOp, RenderOps, TextAlign, TextBaseline,
    crisp, crisp_rect, draw_svg_icon, draw_svg_icon_rotated, execute_ops, icons,
};

pub use uzor_core as core;

// --- Backend re-exports (feature-gated) ------------------------------------------

/// GPU backend — vello 0.6 + wgpu 0.20 full compute pipeline.
///
/// Enabled by the `vello-gpu` feature (on by default).
#[cfg(feature = "vello-gpu")]
pub use uzor_backend_vello_gpu::VelloGpuRenderContext;


/// CPU-only backend — vello_cpu 0.0.6, no GPU required.
///
/// Enabled by the `vello-cpu` feature.
#[cfg(feature = "vello-cpu")]
pub use uzor_backend_vello_cpu::VelloCpuRenderContext;

/// Hybrid CPU/GPU backend — CPU strip encoding + GPU fragment rasterization.
///
/// Uses `wgpu 27.x` which **conflicts** with `vello-gpu` (`wgpu 0.20`).
/// Do not enable `vello-hybrid` and `vello-gpu` together in the same binary.
///
/// Enabled by the `vello-hybrid` feature.
#[cfg(feature = "vello-hybrid")]
pub use uzor_backend_vello_hybrid::VelloHybridRenderContext;

/// Pure-Rust CPU software rasterizer using tiny-skia + fontdue.
///
/// Enabled by the `tiny-skia` feature.
#[cfg(feature = "tiny-skia")]
pub use uzor_backend_tiny_skia::TinySkiaCpuRenderContext;

/// Canvas 2D backend for WebAssembly targets.
///
/// This backend is only meaningful on `wasm32` targets.
/// Enabled by the `canvas2d` feature.
#[cfg(feature = "canvas2d")]
#[cfg(target_arch = "wasm32")]
pub use uzor_backend_canvas2d::Canvas2dRenderContext;
