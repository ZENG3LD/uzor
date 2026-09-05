//! Median-cut palette + k-means refine + similar-color merge.

pub fn median_cut(rgb: &[u8], k: usize) -> (Vec<u32>, Vec<[u8; 3]>) {
    let n = rgb.len() / 3;
    let k = k.max(1).min(n.max(1));
    let mut indices: Vec<u32> = (0..n as u32).collect();
    let mut boxes: Vec<(usize, usize)> = vec![(0, n)]; // ranges into `indices`

    while boxes.len() < k {
        let mut best = 0usize;
        let mut best_range = 0u32;
        for (i, &(lo, hi)) in boxes.iter().enumerate() {
            if hi - lo < 2 {
                continue;
            }
            let r = channel_range(rgb, &indices[lo..hi]);
            if r >= best_range {
                best_range = r;
                best = i;
            }
        }
        if best_range == 0 {
            break;
        }
        let (lo, hi) = boxes[best];
        let axis = longest_axis(rgb, &indices[lo..hi]);
        indices[lo..hi].sort_by_key(|&p| rgb[p as usize * 3 + axis]);
        let mid = lo + (hi - lo) / 2;
        boxes[best] = (lo, mid);
        boxes.push((mid, hi));
    }

    let mut pal = vec![[0u8; 3]; boxes.len()];
    for (bi, &(lo, hi)) in boxes.iter().enumerate() {
        pal[bi] = mean_color(rgb, &indices[lo..hi]);
    }
    let idx = assign_nearest(rgb, n, &pal);
    (idx, pal)
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
        for lab in 0..k {
            if cnt[lab] == 0 {
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
            let d = dist2(pal[i], pal[j]);
            if d <= t2 {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    parent[rj] = ri;
                }
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

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

fn channel_range(rgb: &[u8], pix: &[u32]) -> u32 {
    let mut min = [255u8; 3];
    let mut max = [0u8; 3];
    for &p in pix {
        let o = p as usize * 3;
        for c in 0..3 {
            min[c] = min[c].min(rgb[o + c]);
            max[c] = max[c].max(rgb[o + c]);
        }
    }
    let r = max[0] as u32 - min[0] as u32;
    let g = max[1] as u32 - min[1] as u32;
    let b = max[2] as u32 - min[2] as u32;
    r.max(g).max(b)
}

fn longest_axis(rgb: &[u8], pix: &[u32]) -> usize {
    let mut min = [255u8; 3];
    let mut max = [0u8; 3];
    for &p in pix {
        let o = p as usize * 3;
        for c in 0..3 {
            min[c] = min[c].min(rgb[o + c]);
            max[c] = max[c].max(rgb[o + c]);
        }
    }
    let r = max[0] as u32 - min[0] as u32;
    let g = max[1] as u32 - min[1] as u32;
    let b = max[2] as u32 - min[2] as u32;
    if r >= g && r >= b {
        0
    } else if g >= b {
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
