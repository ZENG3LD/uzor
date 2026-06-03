//! URX Hybrid backend — CPU rasterise + GPU composite.
//!
//! ## Architecture
//!
//! Hybrid sits between pure CPU and pure GPU. For each region:
//!   - **Cacheable + small** → CPU rasterise to pixmap, upload as
//!     wgpu texture once, GPU composite (textured quad) every frame.
//!     Transform animation = trivial uniform update, NO CPU work.
//!   - **HighHz or large** → GPU direct via uzor-urx-wgpu adapter.
//!
//! ## What's in this Phase 7 slice
//!
//! - `HybridBackend` shell — owns region-texture cache (wgpu side)
//! - `RegionTexture` — uploaded `wgpu::Texture` + bind group per region
//! - `composite_pass` — single render pass that draws N textured quads
//!   over the swap chain target
//!
//! ## What's deferred to Phase 7.5
//!
//! - Glyph atlas (CPU-side fontdue/skrifa raster → shared atlas
//!   texture). Currently text on Hybrid path = same as on WGPU path
//!   (cosmic-text via uzor-render-wgpu-instanced).
//! - Auto-promotion (3-frames-of-transform-only → promote to layer).
//! - Tile sub-division for very-large regions (>1024×1024) where one
//!   atlas slot is wasteful.

pub mod atlas;
pub mod backend;
pub mod composite;
pub mod region_tex;

pub use atlas::{AtlasSlot, AtlasUpsertResult, RegionAtlas};
pub use backend::HybridBackend;
pub use composite::{QuadInstance, ScreenUniform};
pub use region_tex::RegionTexture;
