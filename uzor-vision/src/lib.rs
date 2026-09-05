//! Computer vision frontend: CPU / Hybrid / GPU.
//!
//! Downstream crates consume the label map. Not an SVG crate.

mod color;
mod cpu;
mod gpu;
mod merge;
mod rescue;
mod slic;

pub use color::{chroma, dist2, same_flat};
pub use gpu::GpuMode;

#[derive(Clone, Debug)]
pub struct Config {
    pub gpu: GpuMode,
    /// Superpixel count. 0 = skip SLIC (caller segments).
    pub slic_k: u32,
    pub merge: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gpu: GpuMode::Hybrid,
            slic_k: 512,
            merge: 14.0,
        }
    }
}

#[derive(Debug)]
pub struct Frame {
    pub rgb: Vec<u8>,
    pub idx: Vec<u32>,
    pub pal: Vec<[u8; 3]>,
    pub device: String,
}

pub fn process(rgb: &[u8], w: usize, h: usize, cfg: &Config) -> Frame {
    let orig = rgb.to_vec();
    let (rgb, denoise_dev) = gpu::denoise(rgb, w, h, cfg.gpu);
    if cfg.slic_k == 0 {
        return Frame {
            rgb,
            idx: Vec::new(),
            pal: Vec::new(),
            device: denoise_dev,
        };
    }
    let (mut idx, mut pal, slic_dev) = slic::slic(&rgb, w, h, cfg.slic_k, cfg.gpu);
    merge::merge_adjacent_similar(&mut idx, &mut pal, w, h, cfg.merge);
    pal = merge::pal_from_idx(&rgb, &idx, pal.len());
    // Rescue against the undenoised raster: bilateral smears thin gold
    // into cream, then the pixel is no longer an orphan.
    rescue::rescue_rare_hue(&orig, &mut idx, &mut pal, w, h);
    rescue::split_luma(&orig, &mut idx, &mut pal, w, h);
    pal = merge::pal_from_idx(&orig, &idx, pal.len());
    let device = slic_dev;
    Frame {
        rgb,
        idx,
        pal,
        device,
    }
}
