//! URX full-GPU compute pipeline backend.
//!
//! # Architecture
//!
//! Six-stage compute pipeline:
//!   encode → tile_assign → tile_sort → (coarse folded) → fine → blit
//!
//! Coarse pass note (v1.6.0 rect-only): no separate coarse stage.
//! The sorted `tile_lists` buffer IS the PTCL — `fine.wgsl` reads it
//! directly. Add `coarse.wgsl` when implementing gradient/glyph variants.
//!
//! # Crate layout
//!
//! - [`cmd`]     — flat `SceneCmd` struct (32 bytes, GPU-uploadable)
//! - [`encoder`] — CPU encoder: `Scene` → `Vec<SceneCmd>`
//! - [`tile`]    — `TileBuffers` + `TilePipeline` (assign + sort + fine)

pub mod cmd;
pub mod encoder;
pub mod tile;

pub use cmd::{CmdKind, SceneCmd};
pub use encoder::encode_scene;
pub use tile::{DispatchUniforms, TileBuffers, TilePipeline, TILE_CMD_CAP, TILE_SIZE};
