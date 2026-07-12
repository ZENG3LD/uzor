//! Offscreen render-target types — capability-gated `RenderContext` extension.
//!
//! See `docs/uzor-tessera/plans/offscreen-target-2026-07-12.md` §2 for the
//! full design rationale. These types are backend-agnostic handles; the
//! actual texture/pixmap storage lives inside whichever concrete
//! `RenderContext` impl created it — never exposed past this module.

/// Opaque handle to a backend-owned offscreen render target.
///
/// Backends assign the id (monotonic counter or texture-pool index);
/// the kernel only stores and replays it — never inspects internals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OffscreenTargetId(pub u64);

/// Device-pixel dimensions requested for a target. Kernel computes
/// this from the boundary node's resolved `Rect` × `ctx.dpr()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffscreenTargetDesc {
    pub width_px: u32,
    pub height_px: u32,
    pub dpr: f64,
}

/// Result of requesting a target — `None` means "backend does not
/// support offscreen targets" (default no-op case), the caller must
/// fall back to inline paint for this frame and every frame after
/// (the walker should not retry per-frame; cache the `false` capability
/// result per backend instance, not per node).
pub type OffscreenTarget = Option<OffscreenTargetId>;
