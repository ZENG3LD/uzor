use crate::color::{chroma, dist2, same_flat};

fn should_join(a: [u8; 3], b: [u8; 3], thresh: f32, t2: i32) -> bool {
    if !same_flat(a, b) {
        return false;
    }
    let cap = if chroma(a) < 32 && chroma(b) < 32 {
        let t = thresh * 1.6;
        (t * t) as i32
    } else {
        t2
    };
    dist2(a, b) <= cap
}

pub fn merge_adjacent_similar(
    idx: &mut [u32],
    pal: &mut Vec<[u8; 3]>,
    w: usize,
    h: usize,
    thresh: f32,
) {
    let k = pal.len();
    if k == 0 || w == 0 || h == 0 {
        return;
    }
    let t2 = (thresh * thresh) as i32;
    let mut parent: Vec<u32> = (0..k as u32).collect();
    let find = |parent: &mut [u32], mut a: u32| -> u32 {
        while parent[a as usize] != a {
            let p = parent[a as usize];
            parent[a as usize] = parent[p as usize];
            a = p;
        }
        a
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let a = idx[i];
            if (a as usize) >= k {
                continue;
            }
            if x + 1 < w {
                let b = idx[i + 1];
                if (b as usize) < k && a != b {
                    let ca = pal[a as usize];
                    let cb = pal[b as usize];
                    if should_join(ca, cb, thresh, t2) {
                        let ra = find(&mut parent, a);
                        let rb = find(&mut parent, b);
                        if ra != rb {
                            parent[rb as usize] = ra;
                        }
                    }
                }
            }
            if y + 1 < h {
                let b = idx[i + w];
                if (b as usize) < k && a != b {
                    let ca = pal[a as usize];
                    let cb = pal[b as usize];
                    if should_join(ca, cb, thresh, t2) {
                        let ra = find(&mut parent, a);
                        let rb = find(&mut parent, b);
                        if ra != rb {
                            parent[rb as usize] = ra;
                        }
                    }
                }
            }
        }
    }
    let mut remap = vec![u32::MAX; k];
    let mut next = 0u32;
    for i in 0..k {
        let r = find(&mut parent, i as u32);
        if remap[r as usize] == u32::MAX {
            remap[r as usize] = next;
            next += 1;
        }
        remap[i] = remap[r as usize];
    }
    for lab in idx.iter_mut() {
        if (*lab as usize) < k {
            *lab = remap[*lab as usize];
        }
    }
    pal.resize(next as usize, [0; 3]);
}

pub fn pal_from_idx(rgb: &[u8], idx: &[u32], k: usize) -> Vec<[u8; 3]> {
    let mut acc = vec![[0u64; 3]; k];
    let mut cnt = vec![0u64; k];
    let n = rgb.len() / 3;
    for p in 0..n {
        let lab = idx[p] as usize;
        if lab >= k {
            continue;
        }
        acc[lab][0] += rgb[p * 3] as u64;
        acc[lab][1] += rgb[p * 3 + 1] as u64;
        acc[lab][2] += rgb[p * 3 + 2] as u64;
        cnt[lab] += 1;
    }
    let mut pal = vec![[0u8; 3]; k];
    for i in 0..k {
        let c = cnt[i].max(1);
        pal[i] = [
            (acc[i][0] / c) as u8,
            (acc[i][1] / c) as u8,
            (acc[i][2] / c) as u8,
        ];
    }
    pal
}
