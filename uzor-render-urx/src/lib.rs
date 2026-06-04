//! URX render adapter — `uzor::RenderContext` implementation that emits
//! `urx_core::Scene` draw commands.
//!
//! Phase A bridge: consumers that speak the existing imperative
//! `RenderContext` API (begin_path / move_to / fill / stroke / set_font /
//! fill_text / …) write into `UrxRenderContext`; on `take_scene()` they
//! get a flat `urx_core::Scene` ready to be rasterised by any of the four
//! URX 2D backends (`urx-cpu` / `urx-wgpu` / `urx-hybrid` / `urx-wgpu-full`).
//!
//! Phase B (future): tessera-canvas / consumer code learns to emit
//! `urx_core::DrawCommand` directly, skipping this converter entirely.
//! At that point the converter stays as the back-compat path for legacy
//! `RenderContext` consumers; both paint into the same `Scene`.
//!
//! Why one adapter, not four: `urx_core::Scene` was designed as the
//! universal middle wire — every URX backend consumes the same
//! `DrawCommand` vocabulary unchanged. Backend choice happens at submit
//! time in `uzor-render-hub`; this crate stays backend-agnostic.

mod context;

pub use context::UrxRenderContext;
