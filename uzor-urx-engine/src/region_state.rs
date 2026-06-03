//! Per-region state held by the engine. One entry per RegionId.

use uzor_urx_core::dirty::DirtyState;
use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::scene::Scene;

use crate::cadence::RenderCadence;

#[derive(Debug, Clone)]
pub struct RegionState {
    /// The scene the consumer last upserted for this region.
    pub scene:     Scene,
    /// Logical bounds in window-space pixels. Used for the dirty
    /// rect union + clip during render.
    pub bounds:    Rect,
    /// Current Affine — separate from scene so transform-only
    /// changes never trigger re-raster.
    pub transform: Affine,
    /// Current dirty state.
    pub dirty:     DirtyState,
    /// Caller-declared cadence intent.
    pub cadence:   RenderCadence,
}

impl RegionState {
    pub fn new(scene: Scene, bounds: Rect, cadence: RenderCadence) -> Self {
        Self {
            scene,
            bounds,
            transform: Affine::IDENTITY,
            // First time we see a region it must paint at least once.
            dirty: DirtyState::Content,
            cadence,
        }
    }
}
