//! Rare-hue keep and luminance split inside a chromatic fill.
//! Gold stems and coin highlights are small and get donated to cream/mid-gold.

use crate::color::{chroma, dist2, same_flat};

pub fn rescue_rare_hue(rgb: &[u8], idx: &mut [u32], pal: &mut Vec<[u8; 3]>, w: usize, h: usize) {
    let n = w * h;
    if pal.is_empty() || n == 0 {
        return;
    }
    let mut orphan = vec![false; n];
    for p in 0..n {
        let c = [rgb[p * 3], rgb[p * 3 + 1], rgb[p * 3 + 2]];
        if chroma(c) < 40 {
            continue;
        }
        let lab = idx[p] as usize;
        if lab >= pal.len() {
            continue;
        }
        if dist2(c, pal[lab]) >= 40 * 40 {
            orphan[p] = true;
        }
    }
    let mut seen = vec![false; n];
    let mut next = pal.len() as u32;
    for y in 0..h {
        for x in 0..w {
            let start = y * w + x;
            if !orphan[start] || seen[start] {
                continue;
            }
            let seed = [rgb[start * 3], rgb[start * 3 + 1], rgb[start * 3 + 2]];
            let mut stack = vec![start];
            let mut members = Vec::new();
            seen[start] = true;
            while let Some(p) = stack.pop() {
                members.push(p);
                let px = p % w;
                let py = p / w;
                let neigh = [
                    if px > 0 { Some(p - 1) } else { None },
                    if px + 1 < w { Some(p + 1) } else { None },
                    if py > 0 { Some(p - w) } else { None },
                    if py + 1 < h { Some(p + w) } else { None },
                ];
                for q in neigh.into_iter().flatten() {
                    if seen[q] || !orphan[q] {
                        continue;
                    }
                    let c = [rgb[q * 3], rgb[q * 3 + 1], rgb[q * 3 + 2]];
                    if same_flat(seed, c) && dist2(seed, c) <= 30 * 30 {
                        seen[q] = true;
                        stack.push(q);
                    }
                }
            }
            if members.len() < 8 {
                continue;
            }
            let mut acc = [0u64; 3];
            for &p in &members {
                acc[0] += rgb[p * 3] as u64;
                acc[1] += rgb[p * 3 + 1] as u64;
                acc[2] += rgb[p * 3 + 2] as u64;
                idx[p] = next;
            }
            let m = members.len() as u64;
            pal.push([
                (acc[0] / m) as u8,
                (acc[1] / m) as u8,
                (acc[2] / m) as u8,
            ]);
            next += 1;
        }
    }
}

pub fn split_luma(rgb: &[u8], idx: &mut [u32], pal: &mut Vec<[u8; 3]>, w: usize, h: usize) {
    let n = w * h;
    let k = pal.len();
    if k == 0 {
        return;
    }
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); k];
    for p in 0..n {
        let lab = idx[p] as usize;
        if lab < k {
            bins[lab].push(p);
        }
    }
    let mut next = k as u32;
    for lab in 0..k {
        if chroma(pal[lab]) < 32 {
            continue;
        }
        let pix = &bins[lab];
        if pix.len() < 48 {
            continue;
        }
        let mut lmin = 255u8;
        let mut lmax = 0u8;
        for &p in pix {
            let l = luma(rgb, p);
            lmin = lmin.min(l);
            lmax = lmax.max(l);
        }
        if (lmax as i32 - lmin as i32) < 48 {
            continue;
        }
        let cut = (lmin as u16 + lmax as u16) / 2;
        let mut lo = Vec::new();
        let mut hi = Vec::new();
        for &p in pix {
            if luma(rgb, p) as u16 <= cut {
                lo.push(p);
            } else {
                hi.push(p);
            }
        }
        if lo.len() < 24 || hi.len() < 24 {
            continue;
        }
        let a = mean_rgb(rgb, &lo);
        let b = mean_rgb(rgb, &hi);
        if dist2(a, b) < 28 * 28 {
            continue;
        }
        for &p in &hi {
            idx[p] = next;
        }
        pal.push(b);
        next += 1;
    }
}

fn luma(rgb: &[u8], p: usize) -> u8 {
    ((rgb[p * 3] as u16 + rgb[p * 3 + 1] as u16 + rgb[p * 3 + 2] as u16) / 3) as u8
}

fn mean_rgb(rgb: &[u8], pix: &[usize]) -> [u8; 3] {
    let mut acc = [0u64; 3];
    for &p in pix {
        acc[0] += rgb[p * 3] as u64;
        acc[1] += rgb[p * 3 + 1] as u64;
        acc[2] += rgb[p * 3 + 2] as u64;
    }
    let n = pix.len().max(1) as u64;
    [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
}
