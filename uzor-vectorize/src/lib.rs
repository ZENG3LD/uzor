//! PNG/JPEG → SVG tracer for flat-color graphics.
//!
//! Inverse of [`uzor-icon`](https://docs.rs/uzor-icon): that crate rasterizes
//! SVG, this one vectorizes a raster. Labels come from `uzor-vision`.

mod contour;
mod quantize;
mod region;
mod segment;
mod svg;

use std::path::Path;

use image::RgbImage;
use resvg::usvg::{Options as UsvgOptions, Tree};
use tiny_skia::{Pixmap, Transform};

pub use contour::Pt;
pub use uzor_vision::GpuMode;

#[derive(Clone, Debug)]
pub struct VectorizeOptions {
    pub colors: u32,
    pub merge: f32,
    pub min_area: u32,
    pub epsilon: f32,
    pub absorb_dist: f32,
    pub kmeans_iters: u32,
    /// 3×3 majority on palette labels. Off: it ate thin black outlines.
    pub majority: bool,
    /// Felzenszwalb `k` / size. Higher = coarser regions. 0 = median-cut.
    pub tau: f32,
    pub gpu: GpuMode,
    /// Superpixels. 0 = Felzenszwalb (`tau`) or median-cut.
    pub slic: u32,
}

impl Default for VectorizeOptions {
    fn default() -> Self {
        Self {
            colors: 48,
            merge: 14.0,
            min_area: 32,
            epsilon: 0.4,
            absorb_dist: 48.0,
            kmeans_iters: 0,
            majority: false,
            tau: 80.0,
            gpu: GpuMode::Hybrid,
            slic: 512,
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
    pub denoise_device: String,
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

fn loop_len(lp: &[contour::Pt]) -> f32 {
    let mut s = 0.0f32;
    for i in 1..lp.len() {
        let dx = (lp[i].0 - lp[i - 1].0) as f32;
        let dy = (lp[i].1 - lp[i - 1].1) as f32;
        s += (dx * dx + dy * dy).sqrt();
    }
    s
}

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
    let orig = rgb.clone();

    let vcfg = uzor_vision::Config {
        gpu: opt.gpu,
        slic_k: opt.slic,
        merge: opt.merge,
    };
    let frame = uzor_vision::process(&rgb, w, h, &vcfg);
    let rgb = frame.rgb;
    let denoise_device = frame.device;
    let (mut idx, mut pal) = if opt.slic > 0 && !frame.idx.is_empty() {
        (frame.idx, frame.pal)
    } else if opt.tau > 0.0 {
        let sp = segment::felzenszwalb(&rgb, w, h, opt.tau, opt.min_area);
        let mut idx = sp.0;
        let mut pal = sp.1;
        region::merge_adjacent_similar(&mut idx, &mut pal, w, h, opt.merge);
        (idx, pal)
    } else {
        let k = opt.colors.max(2) as usize;
        quantize::median_cut(&rgb, k)
    };
    if opt.kmeans_iters > 0 {
        quantize::kmeans_refine(&rgb, &mut idx, &mut pal, opt.kmeans_iters);
    }
    if opt.slic == 0 && opt.tau <= 0.0 {
        quantize::merge_similar(&mut idx, &mut pal, opt.merge);
    }
    if opt.majority {
        region::majority_snap(&mut idx, w, h);
    }
    region::absorb_speckles(
        &mut idx,
        &pal,
        w,
        h,
        opt.min_area,
        opt.absorb_dist,
    );
    region::despeckle_labels(&mut idx, w, h);
    region::fill_small_holes(&mut idx, w, h, 16);
    quantize::snap_palette_median(&orig, &idx, &mut pal, w, h);

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
    let img_area = (w * h) as u32;
    let mut fills: Vec<(u32, [u8; 3], Vec<Vec<contour::Pt>>)> = Vec::new();
    let mut strokes: Vec<(u32, [u8; 3], f32, Vec<Vec<contour::Pt>>)> = Vec::new();
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
        let color = pal[idx[pix[0]] as usize];
        let loops_raw = contour::blob_loops(&mask, w, h);
        let mut loops = Vec::new();
        let mut peri = 0.0f32;
        for lp in loops_raw {
            peri += loop_len(&lp);
            if let Some(s) = contour::simplify_loop(&lp, opt.epsilon) {
                loops.push(s);
            }
        }
        if loops.is_empty() {
            continue;
        }
        contour_count += loops.len() as u32;
        let width = 2.0 * area as f32 / peri.max(1.0);
        let luma = (color[0] as u32 + color[1] as u32 + color[2] as u32) / 3;
        let thin_dark = luma < 40 && width < 2.2 && area < img_area / 20;
        if thin_dark {
            strokes.push((area, color, width.clamp(0.8, 5.0), loops));
        } else {
            fills.push((area, color, loops));
        }
    }
    fills.sort_by(|a, b| b.0.cmp(&a.0));
    let mut fill_by_color: Vec<(u32, [u8; 3], Vec<Vec<contour::Pt>>)> = Vec::new();
    for (area, color, loops) in fills {
        if let Some(slot) = fill_by_color.iter_mut().find(|s| s.1 == color) {
            slot.0 += area;
            slot.2.extend(loops);
        } else {
            fill_by_color.push((area, color, loops));
        }
    }
    fill_by_color.sort_by(|a, b| b.0.cmp(&a.0));
    let mut paths: Vec<String> = Vec::new();
    for (_, color, loops) in fill_by_color {
        let hex = svg::hex_color(color);
        paths.push(svg::fill_path(&hex, &svg::path_d(&loops)));
    }
    let mut stroke_by_color: Vec<(u32, [u8; 3], f32, u32, Vec<Vec<contour::Pt>>)> = Vec::new();
    for (area, color, width, loops) in strokes {
        if let Some(slot) = stroke_by_color.iter_mut().find(|s| s.1 == color) {
            slot.0 += area;
            slot.2 += width * area as f32;
            slot.3 += area;
            slot.4.extend(loops);
        } else {
            stroke_by_color.push((area, color, width * area as f32, area, loops));
        }
    }
    for (_, color, wsum, warea, loops) in stroke_by_color {
        let hex = svg::hex_color(color);
        let sw = (wsum / warea.max(1) as f32).clamp(0.8, 5.0);
        paths.push(svg::stroke_path(&hex, sw, &svg::path_d(&loops)));
    }
    let colors_kept = paths.len() as u32;
    let svg = svg::document(w as u32, h as u32, &paths);
    Ok(SvgDocument {
        svg,
        width: w as u32,
        height: h as u32,
        colors_kept,
        contours: contour_count,
        quantized,
        denoise_device,
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

/// 2×2 contact: source | svg / quant | heatmap. Debug loop reads this.
pub fn parity_sheet(src: &RgbImage, svg: &RgbImage, quant: &RgbImage, diff: &RgbImage) -> RgbImage {
    let w = src.width();
    let h = src.height();
    let mut sheet = RgbImage::new(w * 2, h * 2);
    blit(&mut sheet, src, 0, 0);
    blit(&mut sheet, svg, w, 0);
    blit(&mut sheet, quant, 0, h);
    blit(&mut sheet, diff, w, h);
    sheet
}

fn blit(dst: &mut RgbImage, src: &RgbImage, ox: u32, oy: u32) {
    let iw = src.width();
    let ih = src.height();
    for y in 0..ih {
        for x in 0..iw {
            dst.put_pixel(ox + x, oy + y, *src.get_pixel(x, y));
        }
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
            let luma = (((a[0] as u16 + a[1] as u16 + a[2] as u16) / 3) * 35 / 100) as u8;
            let heat = if maxc <= 8 {
                0u8
            } else {
                ((maxc - 8) * 4).clamp(0, 255) as u8
            };
            out.put_pixel(
                x,
                y,
                image::Rgb([luma.saturating_add(heat), luma, luma]),
            );
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct Hotspot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub area: u32,
    pub mean_err: f32,
}

/// Largest visible-error blobs (max-channel > 18). Agent control plane:
/// one crop per hotspot, not one sheet of the whole image.
pub fn vis_hotspots(src: &RgbImage, svg: &RgbImage, n: usize) -> Vec<Hotspot> {
    let w = src.width().min(svg.width()) as usize;
    let h = src.height().min(svg.height()) as usize;
    let mut vis = vec![false; w * h];
    let mut err = vec![0u16; w * h];
    for y in 0..h {
        for x in 0..w {
            let a = src.get_pixel(x as u32, y as u32).0;
            let b = svg.get_pixel(x as u32, y as u32).0;
            let mut maxc = 0i32;
            for c in 0..3 {
                maxc = maxc.max((a[c] as i32 - b[c] as i32).abs());
            }
            if maxc > 18 {
                vis[y * w + x] = true;
                err[y * w + x] = maxc as u16;
            }
        }
    }
    let mut parent: Vec<u32> = (0..(w * h) as u32).collect();
    let find = |p: &mut [u32], mut a: u32| -> u32 {
        while p[a as usize] != a {
            let n = p[a as usize];
            p[a as usize] = p[n as usize];
            a = n;
        }
        a
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !vis[i] {
                continue;
            }
            if x + 1 < w && vis[i + 1] {
                let ra = find(&mut parent, i as u32);
                let rb = find(&mut parent, (i + 1) as u32);
                if ra != rb {
                    parent[rb as usize] = ra;
                }
            }
            if y + 1 < h && vis[i + w] {
                let ra = find(&mut parent, i as u32);
                let rb = find(&mut parent, (i + w) as u32);
                if ra != rb {
                    parent[rb as usize] = ra;
                }
            }
        }
    }
    use std::collections::HashMap;
    let mut acc: HashMap<u32, (u32, u32, u32, u32, u32, u32)> = HashMap::new();
    // root -> minx miny maxx maxy area errsum
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !vis[i] {
                continue;
            }
            let r = find(&mut parent, i as u32);
            let e = acc.entry(r).or_insert((x as u32, y as u32, x as u32, y as u32, 0, 0));
            e.0 = e.0.min(x as u32);
            e.1 = e.1.min(y as u32);
            e.2 = e.2.max(x as u32);
            e.3 = e.3.max(y as u32);
            e.4 += 1;
            e.5 += err[i] as u32;
        }
    }
    let mut spots: Vec<Hotspot> = acc
        .into_values()
        .filter(|e| e.4 >= 40)
        .map(|e| Hotspot {
            x: e.0,
            y: e.1,
            w: e.2 - e.0 + 1,
            h: e.3 - e.1 + 1,
            area: e.4,
            mean_err: e.5 as f32 / e.4 as f32,
        })
        .collect();
    spots.sort_by(|a, b| b.area.cmp(&a.area));
    spots.truncate(n.max(1));
    let pad = 24u32;
    let ww = src.width();
    let hh = src.height();
    for s in spots.iter_mut() {
        let x0 = s.x.saturating_sub(pad);
        let y0 = s.y.saturating_sub(pad);
        let x1 = (s.x + s.w + pad).min(ww);
        let y1 = (s.y + s.h + pad).min(hh);
        s.x = x0;
        s.y = y0;
        s.w = x1 - x0;
        s.h = y1 - y0;
    }
    spots
}

pub fn hotspot_strip(src: &RgbImage, svg: &RgbImage, heat: &RgbImage, hs: &Hotspot) -> RgbImage {
    let mut strip = RgbImage::new(hs.w * 3, hs.h);
    for y in 0..hs.h {
        for x in 0..hs.w {
            let sx = hs.x + x;
            let sy = hs.y + y;
            strip.put_pixel(x, y, *src.get_pixel(sx, sy));
            strip.put_pixel(hs.w + x, y, *svg.get_pixel(sx, sy));
            strip.put_pixel(hs.w * 2 + x, y, *heat.get_pixel(sx, sy));
        }
    }
    strip
}
