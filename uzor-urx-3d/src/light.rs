//! Light sources for the Wave 4 Phong / Blinn-Phong pipeline.
//!
//! Three kinds:
//!   - `Directional` — infinite-distance light, single direction (sun)
//!   - `Point`       — positioned light, smooth quadratic falloff
//!   - `Spot`        — positioned + directional cone with inner/outer
//!                     cutoff cosines (Wave 4b)
//!
//! GPU layout: packed 80-byte uniform record per slot. The full
//! `LightArray` is one uniform block with a count + fixed-size array.
//! Wave 7 added per-light shadow params (only first directional casts).

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
        /// Quadratic attenuation falls smoothly from 1 to 0 in `[0,range]`.
        range: f32,
    },
    /// Cone light: positioned + direction + inner/outer cutoff angles.
    /// Inside `inner_cone_rad` — full brightness. Between inner and
    /// outer — smoothstep fade. Beyond outer — dark.
    Spot {
        position: Vec3,
        /// Direction the cone POINTS (axis from emitter outward).
        direction: Vec3,
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_cone_rad: f32,
        outer_cone_rad: f32,
    },
}

impl Light {
    pub fn directional(direction: Vec3, color: [f32; 3], intensity: f32) -> Self {
        Self::Directional { direction, color, intensity }
    }

    pub fn point(position: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self::Point { position, color, intensity, range }
    }

    pub fn spot(
        position: Vec3,
        direction: Vec3,
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_cone_rad: f32,
        outer_cone_rad: f32,
    ) -> Self {
        Self::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_cone_rad: inner_cone_rad.max(0.0),
            outer_cone_rad: outer_cone_rad.max(inner_cone_rad.max(0.0) + 1e-4),
        }
    }
}

/// One light slot — repr(C), 80 bytes, matches WGSL `LightSlot` after
/// std140-style padding.
///
/// Layout (offsets):
///   00  kind:u32                   (0=dir, 1=point, 2=spot)
///   04  _pad0a:u32  ── kind block padded to 16B
///   08  _pad0b:u32
///   12  _pad0c:u32
///   16  vec:[f32;3]                (dir vector OR position depending on kind)
///   28  _pad1:f32
///   32  color:[f32;3]
///   44  intensity:f32
///   48  range:f32
///   52  cos_inner:f32              (Wave 4b — spot only; 0 for non-spot)
///   56  cos_outer:f32              (Wave 4b — spot only; 0 for non-spot)
///   60  _pad2c:f32
///   64  dir:[f32;3]                (Wave 4b — spot only; cone axis)
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
    pub cos_inner: f32,
    pub cos_outer: f32,
    pub _pad2c: f32,
    pub dir: [f32; 3],
    pub _trailing_d: f32,
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
        cos_inner: 0.0,
        cos_outer: 0.0,
        _pad2c: 0.0,
        dir: [0.0; 3],
        _trailing_d: 0.0,
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
                cos_inner: 0.0,
                cos_outer: 0.0,
                _pad2c: 0.0,
                dir: [0.0; 3],
                _trailing_d: 0.0,
            },
            Light::Point { position, color, intensity, range } => Self {
                kind: 1,
                _pad0: [0; 3],
                vec: position.to_array(),
                _pad1: 0.0,
                color,
                intensity,
                range,
                cos_inner: 0.0,
                cos_outer: 0.0,
                _pad2c: 0.0,
                dir: [0.0; 3],
                _trailing_d: 0.0,
            },
            Light::Spot {
                position,
                direction,
                color,
                intensity,
                range,
                inner_cone_rad,
                outer_cone_rad,
            } => Self {
                kind: 2,
                _pad0: [0; 3],
                vec: position.to_array(),
                _pad1: 0.0,
                color,
                intensity,
                range,
                cos_inner: inner_cone_rad.cos(),
                cos_outer: outer_cone_rad.cos(),
                _pad2c: 0.0,
                dir: direction.normalize_or_zero().to_array(),
                _trailing_d: 0.0,
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
