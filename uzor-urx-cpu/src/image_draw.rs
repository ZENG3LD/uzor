//! Image draw — bilinear-filtered blit of a registered `ImageData`
//! into a destination rect with optional source crop.
//!
//! Premul source-over, honours clip stack (including rounded clips
//! via `ClipStack::pixel_coverage`). Sampling: bilinear when scaled,
//! 1:1 fast-path when dest size matches src size and transform is
//! identity.

use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::scene::ImageId;

use crate::clip::{transform_axis_aligned, ClipStack};
use crate::image_reg::{lookup_image, ImageDataArc};
use crate::pixmap::Pixmap;

pub(crate) fn draw_image_aa(
    pixmap:   &mut Pixmap,
    clip:     &ClipStack,
    src_id:   ImageId,
    src_rect: Option<Rect>,
    dest:     Rect,
    transform: &Affine,
) -> bool {
    let img = match lookup_image(src_id) {
        Some(d) => d,
        None => {
            metrics::counter!(
                uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                "kind" => "image_id_unknown",
            ).increment(1);
            return false;
        }
    };

    let dst_screen = transform_axis_aligned(*transform, dest);
    let cur_clip = clip.current();
    let visible = dst_screen.intersect(cur_clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return true; }

    let src_box = src_rect.unwrap_or_else(|| Rect::new(0.0, 0.0, img.width as f64, img.height as f64));
    if src_box.width() <= 0.0 || src_box.height() <= 0.0 { return true; }

    blit_with_filter(pixmap, clip, &img, src_box, dst_screen, visible);
    true
}

fn blit_with_filter(
    pixmap:   &mut Pixmap,
    clip:     &ClipStack,
    img:      &ImageDataArc,
    src_box:  Rect,
    dst_box:  Rect,
    visible:  Rect,
) {
    let w  = pixmap.width()  as i64;
    let h  = pixmap.height() as i64;
    let ix0 = (visible.x0.floor() as i64).max(0);
    let iy0 = (visible.y0.floor() as i64).max(0);
    let ix1 = (visible.x1.ceil()  as i64).min(w);
    let iy1 = (visible.y1.ceil()  as i64).min(h);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let dst_w = (dst_box.x1 - dst_box.x0).max(1e-9);
    let dst_h = (dst_box.y1 - dst_box.y0).max(1e-9);
    let scale_x = src_box.width()  / dst_w;
    let scale_y = src_box.height() / dst_h;
    let unscaled = (scale_x - 1.0).abs() < 1e-4 && (scale_y - 1.0).abs() < 1e-4;
    let use_mask = !clip.all_rect();

    let img_w = img.width as i64;
    let img_h = img.height as i64;

    for py in iy0 .. iy1 {
        let cy = py as f64 + 0.5;
        let src_y = src_box.y0 + (cy - dst_box.y0) * scale_y;
        for px in ix0 .. ix1 {
            let cx = px as f64 + 0.5;
            let src_x = src_box.x0 + (cx - dst_box.x0) * scale_x;
            let sample = if unscaled {
                let sx = src_x.floor() as i64;
                let sy = src_y.floor() as i64;
                if sx < 0 || sx >= img_w || sy < 0 || sy >= img_h { continue; }
                let idx = ((sy * img_w + sx) * 4) as usize;
                [img.bytes[idx], img.bytes[idx+1], img.bytes[idx+2], img.bytes[idx+3]]
            } else {
                sample_bilinear(img, src_x, src_y)
            };
            if sample[3] == 0 { continue; }
            let mut src = sample;
            if use_mask {
                let m = clip.pixel_coverage(px, py);
                if m == 0 { continue; }
                if m != 255 {
                    src = [
                        ((src[0] as u32 * m as u32 + 127) / 255) as u8,
                        ((src[1] as u32 * m as u32 + 127) / 255) as u8,
                        ((src[2] as u32 * m as u32 + 127) / 255) as u8,
                        ((src[3] as u32 * m as u32 + 127) / 255) as u8,
                    ];
                }
            }
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}

#[inline]
fn sample_bilinear(img: &ImageDataArc, x: f64, y: f64) -> [u8; 4] {
    // Pixel-centre alignment: sample at (x - 0.5, y - 0.5) so integer
    // x lands between texels not on a texel centre.
    let fx = x - 0.5;
    let fy = y - 0.5;
    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let tx = fx - x0 as f64;
    let ty = fy - y0 as f64;

    let img_w = img.width as i64;
    let img_h = img.height as i64;
    let s = |sx: i64, sy: i64| -> [f64; 4] {
        let cx = sx.clamp(0, img_w - 1) as usize;
        let cy = sy.clamp(0, img_h - 1) as usize;
        let idx = (cy * img.width as usize + cx) * 4;
        [
            img.bytes[idx]   as f64,
            img.bytes[idx+1] as f64,
            img.bytes[idx+2] as f64,
            img.bytes[idx+3] as f64,
        ]
    };
    let p00 = s(x0,     y0    );
    let p10 = s(x0 + 1, y0    );
    let p01 = s(x0,     y0 + 1);
    let p11 = s(x0 + 1, y0 + 1);

    let w00 = (1.0 - tx) * (1.0 - ty);
    let w10 = tx         * (1.0 - ty);
    let w01 = (1.0 - tx) * ty;
    let w11 = tx         * ty;
    let mut out = [0.0f64; 4];
    for i in 0..4 {
        out[i] = p00[i] * w00 + p10[i] * w10 + p01[i] * w01 + p11[i] * w11;
    }
    [
        out[0].round().clamp(0.0, 255.0) as u8,
        out[1].round().clamp(0.0, 255.0) as u8,
        out[2].round().clamp(0.0, 255.0) as u8,
        out[3].round().clamp(0.0, 255.0) as u8,
    ]
}
