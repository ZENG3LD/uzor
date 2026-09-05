//! Pixel-boundary outlines and corner-preserving simplification.

use std::collections::{HashMap, HashSet};

pub type Pt = (i32, i32);

pub fn blob_loops(mask: &[bool], w: usize, h: usize) -> Vec<Vec<Pt>> {
    let mut nxt: HashMap<Pt, Vec<Pt>> = HashMap::new();
    let mut add = |a: Pt, b: Pt| {
        nxt.entry(a).or_default().push(b);
    };
    for y in 0..h {
        for x in 0..w {
            if !mask[y * w + x] {
                continue;
            }
            if y == 0 || !mask[(y - 1) * w + x] {
                add((x as i32, y as i32), (x as i32 + 1, y as i32));
            }
            if x + 1 == w || !mask[y * w + x + 1] {
                add((x as i32 + 1, y as i32), (x as i32 + 1, y as i32 + 1));
            }
            if y + 1 == h || !mask[(y + 1) * w + x] {
                add((x as i32 + 1, y as i32 + 1), (x as i32, y as i32 + 1));
            }
            if x == 0 || !mask[y * w + x - 1] {
                add((x as i32, y as i32 + 1), (x as i32, y as i32));
            }
        }
    }

    let mut used: HashSet<(Pt, Pt)> = HashSet::new();
    let mut loops = Vec::new();
    let keys: Vec<Pt> = nxt.keys().copied().collect();
    for start in keys {
        let dests = match nxt.get(&start) {
            Some(d) => d.clone(),
            None => continue,
        };
        for dest in dests {
            if used.contains(&(start, dest)) {
                continue;
            }
            let mut loop_pts = vec![start];
            let mut b = dest;
            used.insert((start, b));
            let mut incoming = (b.0 - start.0, b.1 - start.1);
            let mut guard = 0;
            let max_steps = (w + h) * 8;
            while b != start && guard < max_steps {
                guard += 1;
                loop_pts.push(b);
                let cands: Vec<Pt> = nxt
                    .get(&b)
                    .map(|v| v.iter().copied().filter(|c| !used.contains(&(b, *c))).collect())
                    .unwrap_or_default();
                if cands.is_empty() {
                    break;
                }
                let c = if cands.len() == 1 {
                    cands[0]
                } else {
                    let vecs: Vec<(i32, i32)> =
                        cands.iter().map(|c| (c.0 - b.0, c.1 - b.1)).collect();
                    let pick = rightmost_turn(incoming, &vecs);
                    (b.0 + pick.0, b.1 + pick.1)
                };
                used.insert((b, c));
                incoming = (c.0 - b.0, c.1 - b.1);
                b = c;
            }
            if loop_pts.len() >= 4 {
                loop_pts.push(loop_pts[0]);
                loops.push(loop_pts);
            }
        }
    }
    loops
}

fn rightmost_turn(incoming: (i32, i32), outgoing: &[(i32, i32)]) -> (i32, i32) {
    let (ix, iy) = incoming;
    let mut best = outgoing[0];
    let mut best_cross = i32::MIN;
    let mut best_dot = i32::MIN;
    for &v in outgoing {
        let cross = ix * v.1 - iy * v.0;
        let dot = ix * v.0 + iy * v.1;
        if cross > best_cross || (cross == best_cross && dot > best_dot) {
            best_cross = cross;
            best_dot = dot;
            best = v;
        }
    }
    best
}

/// Keep corners (turning angle away from 180°) and RDP the collinear runs.
pub fn simplify_loop(pts: &[Pt], epsilon: f32) -> Option<Vec<Pt>> {
    if pts.len() < 4 {
        return None;
    }
    let mut closed: Vec<Pt> = pts.to_vec();
    if closed.first() != closed.last() {
        closed.push(closed[0]);
    }
    if epsilon <= 0.0 {
        return Some(closed);
    }
    let body = rdp_corners(&closed[..closed.len() - 1], epsilon);
    if body.len() < 3 {
        return None;
    }
    let mut out = body;
    if out.first() != out.last() {
        let first = out[0];
        out.push(first);
    }
    Some(out)
}

fn rdp_corners(pts: &[Pt], epsilon: f32) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    // mark hard corners so RDP cannot delete them
    let n = pts.len();
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    for i in 1..n - 1 {
        let (ax, ay) = pts[i - 1];
        let (bx, by) = pts[i];
        let (cx, cy) = pts[i + 1];
        let ux = bx - ax;
        let uy = by - ay;
        let vx = cx - bx;
        let vy = cy - by;
        let cross = (ux * vy - uy * vx).abs();
        let dot = ux * vx + uy * vy;
        // pixel-grid: a 90° turn has |cross|=1 and dot=0 for unit steps
        if cross >= 1 && dot <= 0 {
            keep[i] = true;
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && !keep[j] {
            j += 1;
        }
        if j >= n {
            j = n - 1;
        }
        let run: Vec<Pt> = pts[i..=j].to_vec();
        let simp = rdp(&run, epsilon);
        if out.is_empty() {
            out.extend(simp);
        } else {
            out.extend(simp.into_iter().skip(1));
        }
        if j == n - 1 {
            break;
        }
        i = j;
    }
    out
}

fn rdp(pts: &[Pt], epsilon: f32) -> Vec<Pt> {
    if pts.len() < 3 || epsilon <= 0.0 {
        return pts.to_vec();
    }
    let start = pts[0];
    let end = pts[pts.len() - 1];
    let seg = (end.0 - start.0, end.1 - start.1);
    let length = ((seg.0 * seg.0 + seg.1 * seg.1) as f32).sqrt();
    let mut max_d = -1.0f32;
    let mut max_i = 0usize;
    for (i, &p) in pts.iter().enumerate() {
        let d = if length < 1e-6 {
            let dx = (p.0 - start.0) as f32;
            let dy = (p.1 - start.1) as f32;
            (dx * dx + dy * dy).sqrt()
        } else {
            let nx = -(seg.1 as f32) / length;
            let ny = seg.0 as f32 / length;
            ((p.0 - start.0) as f32 * nx + (p.1 - start.1) as f32 * ny).abs()
        };
        if d > max_d {
            max_d = d;
            max_i = i;
        }
    }
    if max_d > epsilon && max_i > 0 && max_i < pts.len() - 1 {
        let left = rdp(&pts[..=max_i], epsilon);
        let right = rdp(&pts[max_i..], epsilon);
        let mut out = left;
        out.extend(right.into_iter().skip(1));
        out
    } else {
        vec![start, end]
    }
}
