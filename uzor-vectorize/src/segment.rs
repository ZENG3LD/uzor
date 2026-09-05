//! Felzenszwalb–Huttenlocher graph segmentation.
//!
//! Pixels are nodes, 4-edges weighted by max-channel RGB. Neighbouring
//! components merge while the edge is cheaper than `intern + tau/size`.
//! Spatially separate pastels (mint ray vs cyan triangle) never meet, so
//! they cannot collapse the way a global median-cut box does.

pub fn felzenszwalb(
    rgb: &[u8],
    w: usize,
    h: usize,
    tau: f32,
    min_size: u32,
) -> (Vec<u32>, Vec<[u8; 3]>) {
    let n = w * h;
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut edges: Vec<(u16, u32, u32)> = Vec::with_capacity(n * 2);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if x + 1 < w {
                edges.push((maxc(rgb, i, i + 1), i as u32, (i + 1) as u32));
            }
            if y + 1 < h {
                edges.push((maxc(rgb, i, i + w), i as u32, (i + w) as u32));
            }
        }
    }
    edges.sort_unstable_by_key(|e| e.0);

    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut size = vec![1u32; n];
    let mut intern = vec![0u16; n];

    for &(wt, a, b) in &edges {
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        if ra == rb {
            continue;
        }
        let ta = intern[ra as usize] as f32 + tau / size[ra as usize] as f32;
        let tb = intern[rb as usize] as f32 + tau / size[rb as usize] as f32;
        if (wt as f32) <= ta.min(tb) {
            let r = union(&mut parent, &mut size, ra, rb);
            intern[r as usize] = intern[ra as usize]
                .max(intern[rb as usize])
                .max(wt);
        }
    }

    let min_size = min_size.max(1);
    for &(wt, a, b) in &edges {
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        if ra == rb {
            continue;
        }
        // Colour-gated: a colour-blind min-size pass chains through AA
        // into one region on burst/memphis. Only eat crumbs whose border
        // is already a small step.
        if (size[ra as usize] < min_size || size[rb as usize] < min_size) && wt <= 18 {
            let r = union(&mut parent, &mut size, ra, rb);
            intern[r as usize] = intern[ra as usize]
                .max(intern[rb as usize])
                .max(wt);
        }
    }

    let mut remap = vec![u32::MAX; n];
    let mut next = 0u32;
    let mut idx = vec![0u32; n];
    for i in 0..n {
        let r = find(&mut parent, i as u32) as usize;
        if remap[r] == u32::MAX {
            remap[r] = next;
            next += 1;
        }
        idx[i] = remap[r];
    }
    let k = next as usize;
    let mut acc = vec![[0u64; 3]; k];
    let mut cnt = vec![0u64; k];
    for p in 0..n {
        let lab = idx[p] as usize;
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
    (idx, pal)
}

fn maxc(rgb: &[u8], a: usize, b: usize) -> u16 {
    let mut m = 0u16;
    for c in 0..3 {
        let d = (rgb[a * 3 + c] as i16 - rgb[b * 3 + c] as i16).unsigned_abs();
        if d > m {
            m = d;
        }
    }
    m
}

fn find(parent: &mut [u32], mut a: u32) -> u32 {
    while parent[a as usize] != a {
        let p = parent[a as usize];
        parent[a as usize] = parent[p as usize];
        a = p;
    }
    a
}

fn union(parent: &mut [u32], size: &mut [u32], a: u32, b: u32) -> u32 {
    let (ra, rb) = if size[a as usize] >= size[b as usize] {
        (a, b)
    } else {
        (b, a)
    };
    parent[rb as usize] = ra;
    size[ra as usize] += size[rb as usize];
    ra
}
