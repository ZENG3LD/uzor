//! URX full-GPU compute pipeline backend.
//!
//! # Architecture
//!
//! Six-stage compute pipeline:
//!   encode → tile_assign → tile_sort → coarse → fine → blit
//!
//! This crate ships the first two dispatches: SceneCmd encoding and
//! tile_assign + tile_sort. The remaining stages (coarse / fine / blit)
//! land in subsequent development stages.
//!
//! # Crate layout
//!
//! - [`cmd`]     — flat `SceneCmd` struct (32 bytes, GPU-uploadable)
//! - [`encoder`] — CPU encoder: `Scene` → `Vec<SceneCmd>`
//! - [`tile`]    — `TileBuffers` + `TilePipeline` compute dispatch

pub mod cmd;
pub mod encoder;
pub mod tile;

pub use cmd::{CmdKind, SceneCmd};
pub use encoder::encode_scene;
pub use tile::{DispatchUniforms, TileBuffers, TilePipeline, TILE_CMD_CAP, TILE_SIZE};
