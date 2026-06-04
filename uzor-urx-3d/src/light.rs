//! Light sources for the Wave 4 Phong / Blinn-Phong pipeline.
//!
//! Two kinds for now:
//!   - `Directional` — infinite-distance light, single direction (sun)
//!   - `Point` — positioned light, inverse-square-ish falloff
//!
//! GPU layout: packed 80-byte uniform record per slot. The full
//! `LightArray` is one uniform block with a count + fixed-size array.
//! Wave 7 will add per-light shadow params (depth map slice index).

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const MAX_LIGHTS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub enum Light {
    Directional {
        /// Direction the light TRAVELS (from sun to scene). Shader flips
        /// it to point from surface→light.
        direction: Vec3,
        color: [f32; 3],
        intensity: f32,
    },
    Point {
        position: Vec3,
        color: [f32; 3],
        intensity: f32,
        /// Quadratic attenuation = 1 / (1 + linear·d + quad·d²).
        /// `range` derives both: linear = 4.5/range, quad = 75/range².
        range: f32,
    },
}

impl Light {
    pub fn directional(direction: Vec3, color: [f32; 3], intensity: f32) -> Self {
        Self::Directional { direction, color, intensity }
    }

    pub fn point(position: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self::Point { position, color, intensity, range }
    }
}

/// One light slot — repr(C), 80 bytes, matches WGSL `LightSlot` after
/// std140-style padding.
///
/// Layout (offsets):
///   00  kind:u32
///   04  _pad0a:u32  ── kind block padded to 16B
///   08  _pad0b:u32
///   12  _pad0c:u32
///   16  vec:[f32;3]
///   28  _pad1:f32
///   32  color:[f32;3]
///   44  intensity:f32
///   48  range:f32
///   52  _pad2a:f32
///   56  _pad2b:f32
///   60  _pad2c:f32
///   64  _trailing_a:f32        ── struct stride rounded up to 80
///   68  _trailing_b:f32
///   72  _trailing_c:f32
///   76  _trailing_d:f32
///   80  end
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct LightRaw {
    pub kind: u32,
    pub _pad0: [u32; 3],
    pub vec: [f32; 3],
    pub _pad1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub _pad2: [f32; 3],
    pub _trailing: [f32; 4],
}

impl LightRaw {
    pub const UNUSED: Self = Self {
        kind: 0xFFFF_FFFF,
        _pad0: [0; 3],
        vec: [0.0; 3],
        _pad1: 0.0,
        color: [0.0; 3],
        intensity: 0.0,
        range: 0.0,
        _pad2: [0.0; 3],
        _trailing: [0.0; 4],
    };

    pub fn from_light(l: &Light) -> Self {
        match *l {
            Light::Directional { direction, color, intensity } => Self {
                kind: 0,
                _pad0: [0; 3],
                vec: direction.normalize_or_zero().to_array(),
                _pad1: 0.0,
                color,
                intensity,
                range: 0.0,
                _pad2: [0.0; 3],
                _trailing: [0.0; 4],
            },
            Light::Point { position, color, intensity, range } => Self {
                kind: 1,
                _pad0: [0; 3],
                vec: position.to_array(),
                _pad1: 0.0,
                color,
                intensity,
                range,
                _pad2: [0.0; 3],
                _trailing: [0.0; 4],
            },
        }
    }
}

const _: () = assert!(std::mem::size_of::<LightRaw>() == 80, "LightRaw must be 80 bytes");

/// LightArray uniform layout — matches WGSL `LightArrayU`.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct LightArrayRaw {
    pub count: u32,
    pub _pad: [u32; 3],
    pub ambient: [f32; 3],
    pub _pad_a: f32,
    pub lights: [LightRaw; MAX_LIGHTS],
}

impl LightArrayRaw {
    pub fn from_lights(lights: &[Light], ambient: [f32; 3]) -> Self {
        let mut slots = [LightRaw::UNUSED; MAX_LIGHTS];
        let n = lights.len().min(MAX_LIGHTS);
        for (i, l) in lights.iter().take(n).enumerate() {
            slots[i] = LightRaw::from_light(l);
        }
        Self {
            count: n as u32,
            _pad: [0; 3],
            ambient,
            _pad_a: 0.0,
            lights: slots,
        }
    }

    pub fn default_unlit() -> Self {
        Self {
            count: 0,
            _pad: [0; 3],
            ambient: [1.0, 1.0, 1.0],
            _pad_a: 0.0,
            lights: [LightRaw::UNUSED; MAX_LIGHTS],
        }
    }
}
