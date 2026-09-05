//! Median-cut palette + optional k-means + similar-color merge.
//!
//! Default path: 3×3 median denoise, Heckbert median-cut on pixels
//! (largest luminance-weighted range, box membership kept). Population
//! unique-color cuts spent the palette on cream JPEG noise and turned
//! gold coins red. Nearest-centroid / k-means steal rare hues.

/// PIL `MedianFilter(3)` equivalent: per-channel 3×3 median.
pub fn median3(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut v = [0u8; 9];
                let mut n = 0usize;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let ny = y as i32 + dy;
                        let nx = x as i32 + dx;
                        if ny < 0 || nx < 0 || ny >= h as i32 || nx >= w as i32 {
                            continue;
                        }
                        v[n] = rgb[(ny as usize * w + nx as usize) * 3 + c];
                        n += 1;
                    }
                }
                let sl = &mut v[..n];
                sl.sort_unstable();
                out[(y * w + x) * 3 + c] = sl[n / 2];
            }
        }
    }
    out
}

/// Edge-preserving smooth. JPEG AA is a 1px ramp that would otherwise
/// become a Felzenszwalb bridge between two flats. Range sigma is in
/// max-channel units.
pub fn bilateral(rgb: &[u8], w: usize, h: usize, radius: i32, sigma_s: f32, sigma_r: f32) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 3];
    if w == 0 || h == 0 {
        return out;
    }
    let inv_2s = 0.5 / (sigma_s * sigma_s).max(1e-6);
    let inv_2r = 0.5 / (sigma_r * sigma_r).max(1e-6);
    let r = radius.max(1);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let cr = rgb[i] as f32;
            let cg = rgb[i + 1] as f32;
            let cb = rgb[i + 2] as f32;
            let mut ar = 0.0f32;
            let mut ag = 0.0f32;
            let mut ab = 0.0f32;
            let mut ws = 0.0f32;
            for dy in -r..=r {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 {
                    continue;
                }
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    if nx < 0 || nx >= w as i32 {
                        continue;
                    }
                    let j = (ny as usize * w + nx as usize) * 3;
                    let dr = rgb[j] as f32 - cr;
                    let dg = rgb[j + 1] as f32 - cg;
                    let db = rgb[j + 2] as f32 - cb;
                    let ds2 = (dx * dx + dy * dy) as f32;
                    let dc2 = dr * dr + dg * dg + db * db;
                    let wt = (-ds2 * inv_2s - dc2 * inv_2r).exp();
                    ar += rgb[j] as f32 * wt;
                    ag += rgb[j + 1] as f32 * wt;
                    ab += rgb[j + 2] as f32 * wt;
                    ws += wt;
                }
            }
            let den = ws.max(1e-6);
            out[i] = (ar / den).round().clamp(0.0, 255.0) as u8;
            out[i + 1] = (ag / den).round().clamp(0.0, 255.0) as u8;
            out[i + 2] = (ab / den).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Heckbert median-cut on pixels. Split the box with the largest
/// luminance-weighted range; keep box membership (no nearest reassignment).
pub fn median_cut(rgb: &[u8], k: usize) -> (Vec<u32>, Vec<[u8; 3]>) {
    let n = rgb.len() / 3;
    let k = k.max(1).min(n.max(1));
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut indices: Vec<u32> = (0..n as u32).collect();
    let mut boxes: Vec<(usize, usize)> = vec![(0, n)];

    while boxes.len() < k {
        let mut best = 0usize;
        let mut best_w = 0u32;
        for (i, &(lo, hi)) in boxes.iter().enumerate() {
            if hi - lo < 2 {
                continue;
            }
            let w = weighted_range(rgb, &indices[lo..hi]);
            if w >= best_w {
                best_w = w;
                best = i;
            }
        }
        if best_w == 0 {
            break;
        }
        let (lo, hi) = boxes[best];
        let axis = lum_axis(rgb, &indices[lo..hi]);
        indices[lo..hi].sort_by_key(|&p| rgb[p as usize * 3 + axis]);
        let mid = lo + (hi - lo) / 2;
        boxes[best] = (lo, mid);
        boxes.push((mid, hi));
    }

    let mut pal = vec![[0u8; 3]; boxes.len()];
    let mut idx = vec![0u32; n];
    for (bi, &(lo, hi)) in boxes.iter().enumerate() {
        pal[bi] = mean_color(rgb, &indices[lo..hi]);
        for &p in &indices[lo..hi] {
            idx[p as usize] = bi as u32;
        }
    }
    (idx, pal)
}

fn box_minmax(rgb: &[u8], pix: &[u32]) -> ([u8; 3], [u8; 3]) {
    let mut min = [255u8; 3];
    let mut max = [0u8; 3];
    for &p in pix {
        let o = p as usize * 3;
        for c in 0..3 {
            min[c] = min[c].min(rgb[o + c]);
            max[c] = max[c].max(rgb[o + c]);
        }
    }
    (min, max)
}

fn weighted_range(rgb: &[u8], pix: &[u32]) -> u32 {
    let (min, max) = box_minmax(rgb, pix);
    (max[0] as u32 - min[0] as u32) * 77
        + (max[1] as u32 - min[1] as u32) * 150
        + (max[2] as u32 - min[2] as u32) * 29
}

fn lum_axis(rgb: &[u8], pix: &[u32]) -> usize {
    let (min, max) = box_minmax(rgb, pix);
    let f = [
        (max[0] as u32 - min[0] as u32) * 77,
        (max[1] as u32 - min[1] as u32) * 150,
        (max[2] as u32 - min[2] as u32) * 29,
    ];
    if f[0] >= f[1] && f[0] >= f[2] {
        0
    } else if f[1] >= f[2] {
        1
    } else {
        2
    }
}

fn mean_color(rgb: &[u8], pix: &[u32]) -> [u8; 3] {
    if pix.is_empty() {
        return [0; 3];
    }
    let mut s = [0u64; 3];
    for &p in pix {
        let o = p as usize * 3;
        s[0] += rgb[o] as u64;
        s[1] += rgb[o + 1] as u64;
        s[2] += rgb[o + 2] as u64;
    }
    let n = pix.len() as u64;
    [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
}

pub fn kmeans_refine(rgb: &[u8], idx: &mut [u32], pal: &mut Vec<[u8; 3]>, iters: u32) {
    let n = rgb.len() / 3;
    let k = pal.len();
    if k == 0 || n == 0 {
        return;
    }
    for _ in 0..iters {
        let mut acc = vec![0u64; k * 3];
        let mut cnt = vec![0u64; k];
        for p in 0..n {
            let lab = idx[p] as usize;
            if lab >= k {
                continue;
            }
            acc[lab * 3] += rgb[p * 3] as u64;
            acc[lab * 3 + 1] += rgb[p * 3 + 1] as u64;
            acc[lab * 3 + 2] += rgb[p * 3 + 2] as u64;
            cnt[lab] += 1;
        }
        // Mean is pulled by JPEG fringe. Rare saturated bins (coins,
        // flowers) stay on their previous centroid if they are small.
        let protect = (n as u64 / 80).max(256);
        for lab in 0..k {
            if cnt[lab] == 0 {
                continue;
            }
            if cnt[lab] < protect {
                continue;
            }
            pal[lab] = [
                (acc[lab * 3] / cnt[lab]) as u8,
                (acc[lab * 3 + 1] / cnt[lab]) as u8,
                (acc[lab * 3 + 2] / cnt[lab]) as u8,
            ];
        }
        let new_idx = assign_nearest(rgb, n, pal);
        if new_idx.as_slice() == idx {
            break;
        }
        idx.copy_from_slice(&new_idx);
    }
}

pub fn merge_similar(idx: &mut [u32], pal: &mut Vec<[u8; 3]>, thresh: f32) {
    let k = pal.len();
    if k == 0 {
        return;
    }
    let mut parent: Vec<usize> = (0..k).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    let t2 = (thresh * thresh) as i32;
    let mut counts = vec![0u64; k];
    for &i in idx.iter() {
        counts[i as usize] += 1;
    }
    let live: Vec<usize> = (0..k).filter(|&i| counts[i] > 0).collect();
    for a in 0..live.len() {
        let i = live[a];
        for &j in live.iter().skip(a + 1) {
            if !merge_ok(pal[i], pal[j], t2) {
                continue;
            }
            let ri = find(&mut parent, i);
            let rj = find(&mut parent, j);
            if ri != rj {
                parent[rj] = ri;
            }
        }
    }
    let mut remap = vec![0u32; k];
    let mut new_pal: Vec<[u8; 3]> = Vec::new();
    let mut acc = Vec::new();
    let mut acc_n = Vec::new();
    let mut seen = vec![None; k];
    for i in 0..k {
        let r = find(&mut parent, i);
        if seen[r].is_none() {
            seen[r] = Some(new_pal.len() as u32);
            new_pal.push([0; 3]);
            acc.push([0u64; 3]);
            acc_n.push(0u64);
        }
        remap[i] = seen[r].unwrap();
        let ni = remap[i] as usize;
        acc[ni][0] += pal[i][0] as u64 * counts[i];
        acc[ni][1] += pal[i][1] as u64 * counts[i];
        acc[ni][2] += pal[i][2] as u64 * counts[i];
        acc_n[ni] += counts[i];
    }
    for i in 0..new_pal.len() {
        let n = acc_n[i].max(1);
        new_pal[i] = [
            (acc[i][0] / n) as u8,
            (acc[i][1] / n) as u8,
            (acc[i][2] / n) as u8,
        ];
    }
    for v in idx.iter_mut() {
        *v = remap[*v as usize];
    }
    *pal = new_pal;
}

/// Per-bin median of **interior** pixels (4-neighbours same label).
/// Fringe / AA does not pull the fill.
pub fn snap_palette_median(rgb: &[u8], idx: &[u32], pal: &mut Vec<[u8; 3]>, w: usize, h: usize) {
    let k = pal.len();
    if k == 0 {
        return;
    }
    let mut buckets: Vec<[Vec<u8>; 3]> = (0..k)
        .map(|_| [Vec::new(), Vec::new(), Vec::new()])
        .collect();
    let n = rgb.len() / 3;
    if w == 0 || h == 0 || w * h != n {
        for p in 0..n {
            let lab = idx[p] as usize;
            if lab >= k {
                continue;
            }
            buckets[lab][0].push(rgb[p * 3]);
            buckets[lab][1].push(rgb[p * 3 + 1]);
            buckets[lab][2].push(rgb[p * 3 + 2]);
        }
    } else {
        let mut any = vec![false; k];
        for y in 0..h {
            for x in 0..w {
                let p = y * w + x;
                let lab = idx[p] as usize;
                if lab >= k {
                    continue;
                }
                let mut interior = true;
                if x == 0 || idx[p - 1] != idx[p] {
                    interior = false;
                }
                if x + 1 == w || idx[p + 1] != idx[p] {
                    interior = false;
                }
                if y == 0 || idx[p - w] != idx[p] {
                    interior = false;
                }
                if y + 1 == h || idx[p + w] != idx[p] {
                    interior = false;
                }
                if interior {
                    buckets[lab][0].push(rgb[p * 3]);
                    buckets[lab][1].push(rgb[p * 3 + 1]);
                    buckets[lab][2].push(rgb[p * 3 + 2]);
                    any[lab] = true;
                }
            }
        }
        for p in 0..n {
            let lab = idx[p] as usize;
            if lab >= k || any[lab] {
                continue;
            }
            buckets[lab][0].push(rgb[p * 3]);
            buckets[lab][1].push(rgb[p * 3 + 1]);
            buckets[lab][2].push(rgb[p * 3 + 2]);
        }
    }
    for lab in 0..k {
        if buckets[lab][0].is_empty() {
            continue;
        }
        let mut med = [0u8; 3];
        for c in 0..3 {
            let v = &mut buckets[lab][c];
            v.sort_unstable();
            med[c] = v[v.len() / 2];
        }
        pal[lab] = med;
    }
}

fn assign_nearest(rgb: &[u8], n: usize, pal: &[[u8; 3]]) -> Vec<u32> {
    let mut idx = vec![0u32; n];
    for p in 0..n {
        let c = [rgb[p * 3], rgb[p * 3 + 1], rgb[p * 3 + 2]];
        let mut best = 0u32;
        let mut best_d = i32::MAX;
        for (i, pal_c) in pal.iter().enumerate() {
            let d = dist2(c, *pal_c);
            if d < best_d {
                best_d = d;
                best = i as u32;
            }
        }
        idx[p] = best;
    }
    idx
}

pub(crate) fn chroma(c: [u8; 3]) -> i32 {
    let max = c[0].max(c[1]).max(c[2]) as i32;
    let min = c[0].min(c[1]).min(c[2]) as i32;
    max - min
}

/// Chromatic opponent-hue agreement. Low-chroma (cream, JPEG dirt) is
/// compatible with anyone so spatial merge can swallow a fringe.
pub(crate) fn hue_compatible(a: [u8; 3], b: [u8; 3]) -> bool {
    if chroma(a) < 32 || chroma(b) < 32 {
        return true;
    }
    let a_rg = a[0] as i32 - a[1] as i32;
    let a_yb = (a[0] as i32 + a[1] as i32) / 2 - a[2] as i32;
    let b_rg = b[0] as i32 - b[1] as i32;
    let b_yb = (b[0] as i32 + b[1] as i32) / 2 - b[2] as i32;
    let dot = a_rg as i64 * b_rg as i64 + a_yb as i64 * b_yb as i64;
    let na = a_rg as i64 * a_rg as i64 + a_yb as i64 * a_yb as i64;
    let nb = b_rg as i64 * b_rg as i64 + b_yb as i64 * b_yb as i64;
    if na == 0 || nb == 0 {
        return true;
    }
    // cos² < 0.75 → more than ~30° apart in opponent hue
    dot * dot * 4 >= na * nb * 3
}

/// FZ merge barrier. Low-chroma cream is **not** a universal solvent:
/// cream–pink would otherwise leak through the JPEG AA ramp.
pub(crate) fn same_flat(a: [u8; 3], b: [u8; 3]) -> bool {
    let ca = chroma(a);
    let cb = chroma(b);
    if ca < 32 && cb < 32 {
        return true;
    }
    if ca < 32 || cb < 32 {
        return false;
    }
    hue_compatible(a, b)
}

fn merge_ok(a: [u8; 3], b: [u8; 3], t2: i32) -> bool {
    if dist2(a, b) > t2 {
        return false;
    }
    hue_compatible(a, b)
}

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}


