//! GPU-side encoded scene commands. Flat 32-byte structs uploaded as a
//! storage buffer; the compute pipeline reads this directly without
//! type-tagged unions.
//!
//! Layout matches research-16 §3 "Scene encoding" — kind discriminator
//! in the first u32, geometry payload follows.

use bytemuck::{Pod, Zeroable};

/// Discriminator for `SceneCmd.kind`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdKind {
    /// Filled axis-aligned rect. payload = [x0, y0, x1, y1], packed_rgba in slot[4].
    Rect = 0,
    /// Reserved.
    _Reserved = 1,
    /// Two-color linear gradient over a rect.
    /// slot[0..4] = bbox xyxy, slot[4] = start_color packed rgba8, slot[5] = end_color packed rgba8,
    /// slot[6] = direction enum: 0=L→R, 1=T→B, 2=TL→BR diagonal, 3=BL→TR diagonal.
    LinGradient = 2,
    /// Radial gradient over a rect: inner color at bbox center, outer color at bbox edge.
    /// slot[0..4] = bbox xyxy, slot[4] = inner_color packed rgba8, slot[5] = outer_color packed rgba8,
    /// slot[6] = reserved (center + radius derived from bbox at shader time).
    RadGradient = 3,
}

/// One flat scene command. 32 bytes total, repr(C) for stable layout.
///
/// Slots interpretation by `kind`:
///   Rect:        slot[0..4] = bbox xyxy (f32), slot[4] = packed_rgba u32, slot[5..7] = unused
///   LinGradient: slot[0..4] = bbox xyxy (f32), slot[4] = start_color, slot[5] = end_color,
///                slot[6] = direction (0=L→R, 1=T→B, 2=TL→BR, 3=BL→TR)
///   RadGradient: slot[0..4] = bbox xyxy (f32), slot[4] = inner_color, slot[5] = outer_color,
///                slot[6] = reserved
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SceneCmd {
    pub kind:  u32,
    pub slot0: f32,
    pub slot1: f32,
    pub slot2: f32,
    pub slot3: f32,
    pub slot4: u32,
    pub slot5: u32,
    pub slot6: u32,
}

/// Direction encoding for `CmdKind::LinGradient`.
pub mod lin_dir {
    /// Gradient flows left → right (along +X).
    pub const HORIZONTAL: u32 = 0;
    /// Gradient flows top → bottom (along +Y).
    pub const VERTICAL: u32 = 1;
    /// Gradient flows top-left → bottom-right.
    pub const DIAGONAL_TLBR: u32 = 2;
    /// Gradient flows bottom-left → top-right.
    pub const DIAGONAL_BLTR: u32 = 3;
}

fn pack_rgba(rgba: [u8; 4]) -> u32 {
    (rgba[0] as u32)
        | ((rgba[1] as u32) << 8)
        | ((rgba[2] as u32) << 16)
        | ((rgba[3] as u32) << 24)
}

impl SceneCmd {
    /// Create a Rect command from bbox corners and RGBA bytes.
    pub fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [u8; 4]) -> Self {
        Self {
            kind: CmdKind::Rect as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(rgba), slot5: 0, slot6: 0,
        }
    }

    /// Create a two-color linear gradient command.
    ///
    /// `direction` is one of the `lin_dir::*` constants:
    ///   - `HORIZONTAL`    (0): start_rgba at left, end_rgba at right
    ///   - `VERTICAL`      (1): start_rgba at top,  end_rgba at bottom
    ///   - `DIAGONAL_TLBR` (2): start_rgba at top-left,    end_rgba at bottom-right
    ///   - `DIAGONAL_BLTR` (3): start_rgba at bottom-left, end_rgba at top-right
    pub fn lin_gradient(
        x0: f32, y0: f32, x1: f32, y1: f32,
        start_rgba: [u8; 4],
        end_rgba: [u8; 4],
        direction: u32,
    ) -> Self {
        Self {
            kind: CmdKind::LinGradient as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(start_rgba),
            slot5: pack_rgba(end_rgba),
            slot6: direction,
        }
    }

    /// Create a radial gradient command.
    ///
    /// Center and radius are derived from the bbox at shader time:
    ///   center = bbox midpoint, max_r = max(half-width, half-height).
    /// `inner_rgba` is the color at the center; `outer_rgba` at the perimeter.
    pub fn rad_gradient(
        x0: f32, y0: f32, x1: f32, y1: f32,
        inner_rgba: [u8; 4],
        outer_rgba: [u8; 4],
    ) -> Self {
        Self {
            kind: CmdKind::RadGradient as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(inner_rgba),
            slot5: pack_rgba(outer_rgba),
            slot6: 0,
        }
    }

    /// Returns the bounding box `[x0, y0, x1, y1]` for any cmd kind.
    pub fn bbox(&self) -> [f32; 4] {
        [self.slot0, self.slot1, self.slot2, self.slot3]
    }
}

// Compile-time size assertion.
const _: () = assert!(std::mem::size_of::<SceneCmd>() == 32);
