//! PNG/JPEG → SVG tracer for flat-color graphics.
//!
//! Inverse of [`uzor-icon`](https://docs.rs/uzor-icon): that crate rasterizes
//! SVG, this one vectorizes a raster. Pipeline: median-cut + k-means palette,
//! speckle absorb, pixel-boundary contours, SVG paths.

mod contour;
mod quantize;
mod region;
mod svg;

use std::path::Path;

use image::RgbImage;
use resvg::usvg::{Options as UsvgOptions, Tree};
use tiny_skia::{Pixmap, Transform};

pub use contour::Pt;

#[derive(Clone, Debug)]
pub struct VectorizeOptions {
    pub colors: u32,
    pub merge: f32,
    pub min_area: u32,
    pub epsilon: f32,
    pub absorb_dist: f32,
    pub kmeans_iters: u32,
}

impl Default for VectorizeOptions {
    fn default() -> Self {
        Self {
            colors: 48,
            merge: 14.0,
            min_area: 32,
            epsilon: 0.4,
            absorb_dist: 48.0,
            kmeans_iters: 10,
        }
    }
}

#[derive(Debug)]
pub struct SvgDocument {
    pub svg: String,
    pub width: u32,
    pub height: u32,
    pub colors_kept: u32,
    pub contours: u32,
    /// Packed RGB of the quantized source (length = width*height*3).
    pub quantized: Vec<u8>,
}

#[derive(Debug)]
pub enum VectorizeError {
    Image(String),
    Empty,
    Raster(String),
}

impl std::fmt::Display for VectorizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorizeError::Image(s) => write!(f, "image: {s}"),
            VectorizeError::Empty => write!(f, "empty image"),
            VectorizeError::Raster(s) => write!(f, "raster: {s}"),
        }
    }
}

impl std::error::Error for VectorizeError {}

pub fn vectorize_path(
    path: impl AsRef<Path>,
    opt: &VectorizeOptions,
) -> Result<SvgDocument, VectorizeError> {
    let img = image::open(path.as_ref())
        .map_err(|e| VectorizeError::Image(e.to_string()))?
        .to_rgb8();
    vectorize_rgb(&img, opt)
}

pub fn vectorize_rgb(img: &RgbImage, opt: &VectorizeOptions) -> Result<SvgDocument, VectorizeError> {
    let w = img.width() as usize;
    let h = img.height() as usize;
    if w == 0 || h == 0 {
        return Err(VectorizeError::Empty);
    }
    let mut rgb = Vec::with_capacity(w * h * 3);
    for p in img.pixels() {
        rgb.extend_from_slice(&p.0);
    }

    let k = opt.colors.max(2) as usize;
    let (mut idx, mut pal) = quantize::median_cut(&rgb, k);
    quantize::kmeans_refine(&rgb, &mut idx, &mut pal, opt.kmeans_iters);
    quantize::merge_similar(&mut idx, &mut pal, opt.merge);
    region::majority_snap(&mut idx, w, h);
    region::absorb_speckles(
        &mut idx,
        &pal,
        w,
        h,
        opt.min_area,
        opt.absorb_dist,
    );
    quantize::snap_palette_median(&rgb, &idx, &mut pal);

    let mut quantized = vec![0u8; w * h * 3];
    for i in 0..w * h {
        let c = pal[idx[i] as usize];
        quantized[i * 3] = c[0];
        quantized[i * 3 + 1] = c[1];
        quantized[i * 3 + 2] = c[2];
    }

    let (labels, nlab) = region::label_same_color(&idx, w, h);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); nlab as usize + 1];
    for (i, &lab) in labels.iter().enumerate() {
        members[lab as usize].push(i);
    }
    let mut layers: Vec<(u32, [u8; 3], String)> = Vec::new();
    let mut contour_count = 0u32;
    for blob_id in 1..=nlab {
        let pix = &members[blob_id as usize];
        let area = pix.len() as u32;
        if area < opt.min_area {
            continue;
        }
        let mut mask = vec![false; w * h];
        for &i in pix {
            mask[i] = true;
        }
        let color_i = idx[pix[0]];
        let loops_raw = contour::blob_loops(&mask, w, h);
        let mut loops = Vec::new();
        for lp in loops_raw {
            if let Some(s) = contour::simplify_loop(&lp, opt.epsilon) {
                loops.push(s);
            }
        }
        if loops.is_empty() {
            continue;
        }
        contour_count += loops.len() as u32;
        let d = svg::path_d(&loops);
        let fill = svg::hex_color(pal[color_i as usize]);
        let path = format!("  <path fill=\"{fill}\" fill-rule=\"evenodd\" d=\"{d}\"/>");
        layers.push((area, pal[color_i as usize], path));
    }
    layers.sort_by(|a, b| b.0.cmp(&a.0));
    let paths: Vec<String> = layers.into_iter().map(|l| l.2).collect();
    let colors_kept = paths.len() as u32;
    let svg = svg::document(w as u32, h as u32, &paths);
    Ok(SvgDocument {
        svg,
        width: w as u32,
        height: h as u32,
        colors_kept,
        contours: contour_count,
        quantized,
    })
}

pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Result<RgbImage, VectorizeError> {
    let tree = Tree::from_data(svg.as_bytes(), &UsvgOptions::default())
        .map_err(|e| VectorizeError::Raster(e.to_string()))?;
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| VectorizeError::Raster("pixmap".into()))?;
    let sz = tree.size();
    let transform = Transform::from_scale(width as f32 / sz.width(), height as f32 / sz.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let data = pixmap.data();
    let mut img = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            // resvg writes RGBA; transparent is 0,0,0,0 — treat as black
            img.put_pixel(x, y, image::Rgb([data[i], data[i + 1], data[i + 2]]));
        }
    }
    Ok(img)
}

#[derive(Debug, Clone)]
pub struct ParityStats {
    pub mae: f32,
    pub close_pct: f32,
    pub visible_pct: f32,
}

pub fn parity_rgb(a: &RgbImage, b: &RgbImage) -> ParityStats {
    let w = a.width().min(b.width());
    let h = a.height().min(b.height());
    let n = (w * h) as f32;
    let mut sum = 0.0f32;
    let mut close = 0u32;
    let mut vis = 0u32;
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            let mut maxc = 0i32;
            for c in 0..3 {
                let d = (pa[c] as i32 - pb[c] as i32).abs();
                sum += d as f32;
                if d > maxc {
                    maxc = d;
                }
            }
            if maxc <= 8 {
                close += 1;
            }
            if maxc > 18 {
                vis += 1;
            }
        }
    }
    ParityStats {
        mae: sum / (n * 3.0),
        close_pct: close as f32 / n * 100.0,
        visible_pct: vis as f32 / n * 100.0,
    }
}

pub fn quantized_image(doc: &SvgDocument) -> RgbImage {
    let mut img = RgbImage::new(doc.width, doc.height);
    for y in 0..doc.height {
        for x in 0..doc.width {
            let i = ((y * doc.width + x) * 3) as usize;
            img.put_pixel(
                x,
                y,
                image::Rgb([
                    doc.quantized[i],
                    doc.quantized[i + 1],
                    doc.quantized[i + 2],
                ]),
            );
        }
    }
    img
}

pub fn diff_heatmap(src: &RgbImage, svg: &RgbImage) -> RgbImage {
    let w = src.width().min(svg.width());
    let h = src.height().min(svg.height());
    let mut out = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let a = src.get_pixel(x, y).0;
            let b = svg.get_pixel(x, y).0;
            let mut maxc = 0i32;
            for c in 0..3 {
                maxc = maxc.max((a[c] as i32 - b[c] as i32).abs());
            }
            let heat = (maxc * 3).clamp(0, 255) as u8;
            out.put_pixel(
                x,
                y,
                image::Rgb([
                    ((a[0] as u16 * 45 / 100) as u8).saturating_add(heat),
                    a[1] * 45 / 100,
                    a[2] * 45 / 100,
                ]),
            );
        }
    }
    out
}
