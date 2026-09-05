//! Same-color connected components, 3x3 majority, speckle absorb.

pub fn majority_snap(idx: &mut [u32], w: usize, h: usize) {
    let old = idx.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut counts: [(u32, u8); 9] = [(0, 0); 9];
            let mut nuniq = 0usize;
            let mut best_lab = old[y * w + x];
            let mut best_n = 0u8;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ny = y as i32 + dy;
                    let nx = x as i32 + dx;
                    if ny < 0 || nx < 0 || ny >= h as i32 || nx >= w as i32 {
                        continue;
                    }
                    let lab = old[ny as usize * w + nx as usize];
                    let mut found = false;
                    for slot in counts.iter_mut().take(nuniq) {
                        if slot.0 == lab {
                            slot.1 += 1;
                            if slot.1 > best_n {
                                best_n = slot.1;
                                best_lab = lab;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found && nuniq < 9 {
                        counts[nuniq] = (lab, 1);
                        if best_n == 0 {
                            best_n = 1;
                            best_lab = lab;
                        }
                        nuniq += 1;
                    }
                }
            }
            if best_n >= 5 {
                idx[y * w + x] = best_lab;
            }
        }
    }
}

pub fn label_same_color(idx: &[u32], w: usize, h: usize) -> (Vec<u32>, u32) {
    let n = w * h;
    let mut parent: Vec<u32> = vec![0; n + 1];
    let mut labels = vec![0u32; n];
    let mut next = 0u32;

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
            let color = idx[i];
            let left = if x > 0 && idx[i - 1] == color {
                labels[i - 1]
            } else {
                0
            };
            let up = if y > 0 && idx[i - w] == color {
                labels[i - w]
            } else {
                0
            };
            if left != 0 && up != 0 {
                labels[i] = left;
                let ra = find(&mut parent, left);
                let rb = find(&mut parent, up);
                if ra != rb {
                    parent[rb as usize] = ra;
                }
            } else if left != 0 {
                labels[i] = left;
            } else if up != 0 {
                labels[i] = up;
            } else {
                next += 1;
                parent[next as usize] = next;
                labels[i] = next;
            }
        }
    }

    let mut remap = vec![0u32; next as usize + 1];
    let mut nlab = 0u32;
    for i in 1..=next {
        let r = find(&mut parent, i);
        if remap[r as usize] == 0 {
            nlab += 1;
            remap[r as usize] = nlab;
        }
        remap[i as usize] = remap[r as usize];
    }
    for lab in labels.iter_mut() {
        *lab = remap[*lab as usize];
    }
    (labels, nlab)
}

pub fn absorb_speckles(
    idx: &mut [u32],
    pal: &[[u8; 3]],
    w: usize,
    h: usize,
    min_area: u32,
    max_dist: f32,
) {
    let dist2 = (max_dist * max_dist) as i32;
    for _ in 0..4 {
        let (labels, nlab) = label_same_color(idx, w, h);
        if nlab == 0 {
            return;
        }
        let mut areas = vec![0u32; nlab as usize + 1];
        for &lab in labels.iter() {
            areas[lab as usize] += 1;
        }
        let mut any_small = false;
        for lab in 1..=nlab {
            if areas[lab as usize] > 0 && areas[lab as usize] < min_area {
                any_small = true;
                break;
            }
        }
        if !any_small {
            return;
        }

        let mut votes: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nlab as usize + 1];
        let bump = |votes: &mut Vec<Vec<(u32, u32)>>, lab: u32, color: u32| {
            let slot = &mut votes[lab as usize];
            for pair in slot.iter_mut() {
                if pair.0 == color {
                    pair.1 += 1;
                    return;
                }
            }
            slot.push((color, 1));
        };

        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let a = labels[i];
                if x + 1 < w {
                    let b = labels[i + 1];
                    if a != b {
                        bump(&mut votes, a, idx[i + 1]);
                        bump(&mut votes, b, idx[i]);
                    }
                }
                if y + 1 < h {
                    let b = labels[i + w];
                    if a != b {
                        bump(&mut votes, a, idx[i + w]);
                        bump(&mut votes, b, idx[i]);
                    }
                }
            }
        }

        let mut own = vec![0u32; nlab as usize + 1];
        for (i, &lab) in labels.iter().enumerate() {
            own[lab as usize] = idx[i];
        }

        let mut remap = vec![None; nlab as usize + 1];
        for lab in 1..=nlab {
            let area = areas[lab as usize];
            if area == 0 || area >= min_area {
                continue;
            }
            let v = &mut votes[lab as usize];
            if v.is_empty() {
                continue;
            }
            v.sort_by(|a, b| b.1.cmp(&a.1));
            let src = pal[own[lab as usize] as usize];
            let mut target: Option<u32> = None;
            for &(cand, _) in v.iter() {
                let d = dist2_rgb(src, pal[cand as usize]);
                if d <= dist2 {
                    target = Some(cand);
                    break;
                }
            }
            if target.is_none() && area < 16 {
                target = Some(v[0].0);
            }
            remap[lab as usize] = target;
        }
        let mut changed = false;
        for (i, &lab) in labels.iter().enumerate() {
            if let Some(t) = remap[lab as usize] {
                idx[i] = t;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Union neighbouring labels whose fills are the same flat. Global
/// `merge_similar` is forbidden after Felzenszwalb (it chains 20k crumbs
/// into mud). This only touches a shared 4-border.
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
                    if crate::quantize::same_flat(ca, cb) && dist2_rgb(ca, cb) <= t2 {
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
                    if crate::quantize::same_flat(ca, cb) && dist2_rgb(ca, cb) <= t2 {
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

fn dist2_rgb(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}
