//! Re-export shim — the image registry moved to the shared
//! `uzor-urx-image` crate (URX Wave 4 design §0.4/§10 Commit 1,
//! `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`).
//!
//! Kept at this SAME path (`crate::image_reg`) so `image_draw.rs`'s and
//! `backend.rs`'s existing `crate::image_reg::lookup_image`-shaped call
//! sites (and this crate's own `pub use image_reg::{...}` in `lib.rs`)
//! don't change at all — same "extraction, not a public-API break"
//! discipline as Wave 2's `GlyphKey`/`subpixel_bin_for_x` move into
//! `uzor-urx-glyph`. `image-decode` forwards to
//! `uzor-urx-image/image-decode` (see this crate's `Cargo.toml`) — no
//! local `image` crate dependency needed here anymore.

pub use uzor_urx_image::*;
