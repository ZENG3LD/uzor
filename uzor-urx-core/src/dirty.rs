//! Dirty-state classification — the three-state contract from research.
//!
//! Browser cc, Flutter, GTK4, SwiftUI all converge on the same model:
//!
//!   - `Clean`         — nothing changed, blit cached pixels
//!   - `TransformOnly` — only Affine changed → composite cached texture
//!                       at new transform, NO re-rasterisation
//!   - `Content`       — pixels changed → invalidate cache, re-raster
//!
//! Without this split, every transform change triggers a re-raster
//! pass and the retained-mode benefit collapses.

use crate::math::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirtyState {
    #[default]
    Clean,
    TransformOnly,
    Content,
}

impl DirtyState {
    /// Returns true when re-rasterisation is required (not just composite).
    pub fn needs_raster(self) -> bool {
        matches!(self, Self::Content)
    }

    /// Returns true when composition must run (transform OR content
    /// dirty — but not when fully clean).
    pub fn needs_compose(self) -> bool {
        !matches!(self, Self::Clean)
    }

    /// Promote `Clean` → `TransformOnly` or `TransformOnly` → `Content`.
    /// Idempotent at `Content` (the strongest state).
    pub fn promote_to_transform(&mut self) {
        if *self == Self::Clean {
            *self = Self::TransformOnly;
        }
    }

    pub fn promote_to_content(&mut self) {
        *self = Self::Content;
    }
}

/// A union of dirty rectangles since last clear. Backends scissor to
/// this when present, skip instances outside it.
///
/// Initial impl: union bbox (simplest correct). If profiling shows
/// large dirty unions with huge clean interior areas we promote to
/// a tile-grid representation (WebRender-style, research-05 §7.3).
#[derive(Debug, Default, Clone, Copy)]
pub struct DirtyRect(Option<Rect>);

impl DirtyRect {
    pub const EMPTY: Self = Self(None);

    pub fn add(&mut self, r: Rect) {
        match self.0 {
            None    => self.0 = Some(r),
            Some(u) => self.0 = Some(u.union(r)),
        }
    }

    pub fn bbox(&self) -> Option<Rect> { self.0 }

    pub fn is_empty(&self) -> bool { self.0.is_none() }

    pub fn reset(&mut self) { self.0 = None; }
}
