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
    /// Reserved for future variants (gradient/glyph/path) — encoder
    /// emits only Rect for the v1.6.0 first-stage.
    _Reserved = 1,
}

/// One flat scene command. 32 bytes total, repr(C) for stable layout.
///
/// Slots interpretation by `kind`:
///   Rect:  slot[0..4] = bbox xyxy (f32),  slot[4] = packed_rgba u32, slot[5..8] = unused
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

impl SceneCmd {
    /// Create a Rect command from bbox corners and RGBA bytes.
    pub fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [u8; 4]) -> Self {
        let packed = (rgba[0] as u32)
            | ((rgba[1] as u32) << 8)
            | ((rgba[2] as u32) << 16)
            | ((rgba[3] as u32) << 24);
        Self {
            kind: CmdKind::Rect as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: packed, slot5: 0, slot6: 0,
        }
    }

    /// Returns the bounding box `[x0, y0, x1, y1]` if this is a Rect command.
    pub fn bbox(&self) -> Option<[f32; 4]> {
        if self.kind == CmdKind::Rect as u32 {
            Some([self.slot0, self.slot1, self.slot2, self.slot3])
        } else {
            None
        }
    }
}

// Compile-time size assertion.
const _: () = assert!(std::mem::size_of::<SceneCmd>() == 32);
