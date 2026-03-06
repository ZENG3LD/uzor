//! uzor-cursor — Cursor interaction effects for the uzor UI framework
//!
//! This crate provides rendering-agnostic cursor interaction effects:
//! - Magnet: Elements attracted to cursor within a radius
//! - ClickSpark: Particle bursts on click
//! - BlobCursor: Trailing blob cursor with gooey merge effect
//! - GlareHover: Shiny glare sweep across elements on hover
//!
//! All effects compute animation state only — rendering is left to the UI layer.

mod magnet;
mod click_spark;
mod blob_cursor;
mod glare_hover;

pub use magnet::{Magnet, MagnetState};
pub use click_spark::{ClickSpark, ClickSparkState, Particle, Easing};
pub use blob_cursor::{BlobCursor, BlobCursorState, BlobState};
pub use glare_hover::{GlareHover, GlareHoverState};
