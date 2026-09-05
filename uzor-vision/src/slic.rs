use crate::cpu;
use crate::gpu::{self, Center, GpuMode};
use crate::merge;

pub fn slic(
    rgb: &[u8],
    w: usize,
    h: usize,
    k_target: u32,
    mode: GpuMode,
) -> (Vec<u32>, Vec<[u8; 3]>, String) {
    let n = w * h;
    if n == 0 {
        return (Vec::new(), Vec::new(), "none".into());
    }
    let k_target = k_target.max(4) as usize;
    let step = ((n as f32 / k_target as f32).sqrt()).max(4.0);
    let gw = ((w as f32 / step).ceil() as usize).max(1);
    let gh = ((h as f32 / step).ceil() as usize).max(1);
    let k = gw * gh;
    let mut centers = vec![
        Center {
            x: 0.0,
            y: 0.0,
            r: 0.0,
            g: 0.0,
            b: 0.0,
            _p0: 0.0,
            _p1: 0.0,
            _p2: 0.0,
        };
        k
    ];
    for gy in 0..gh {
        for gx in 0..gw {
            let x = ((gx as f32 + 0.5) * step).min((w - 1) as f32);
            let y = ((gy as f32 + 0.5) * step).min((h - 1) as f32);
            let i = y as usize * w + x as usize;
            let f = cpu::feat(rgb, i);
            centers[gy * gw + gx] = Center {
                x,
                y,
                r: f[0],
                g: f[1],
                b: f[2],
                _p0: 0.0,
                _p1: 0.0,
                _p2: 0.0,
            };
        }
    }
    let compact = (10.0 / step) * (10.0 / step);
    let (labels, device) = match mode {
        GpuMode::Gpu => match gpu::gpu_slic_full(rgb, w, h, &centers, step, compact, 8) {
            Ok(lab) => {
                let name = gpu::ctx()
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|_| "gpu".into());
                (lab, format!("gpu:{name}"))
            }
            Err(e) => {
                eprintln!("gpu slic loop failed ({e}); hybrid fallback");
                slic_hybrid(rgb, w, h, &mut centers, step, compact, gw)
            }
        },
        GpuMode::Hybrid => slic_hybrid(rgb, w, h, &mut centers, step, compact, gw),
        GpuMode::Cpu => {
            let mut labels = vec![0u32; n];
            for _ in 0..8 {
                labels = slic_assign_cpu(rgb, w, h, &centers, step, compact, gw);
                update_centers(rgb, w, h, &labels, &mut centers);
            }
            (labels, "cpu".into())
        }
    };
    let pal = merge::pal_from_idx(rgb, &labels, k);
    (labels, pal, device)
}

fn slic_hybrid(
    rgb: &[u8],
    w: usize,
    h: usize,
    centers: &mut [gpu::Center],
    step: f32,
    compact: f32,
    gw: usize,
) -> (Vec<u32>, String) {
    let n = w * h;
    let mut labels = vec![0u32; n];
    let mut device = "cpu".to_string();
    for _ in 0..8 {
        match gpu::gpu_slic_assign(rgb, w, h, centers, step, compact) {
            Ok(lab) => {
                labels = lab;
                if let Ok(g) = gpu::ctx() {
                    device = format!("hybrid:{}", g.name);
                }
            }
            Err(e) => {
                eprintln!("gpu slic assign failed ({e}); cpu fallback");
                labels = slic_assign_cpu(rgb, w, h, centers, step, compact, gw);
                device = format!("cpu(fallback:{e})");
            }
        }
        update_centers(rgb, w, h, &labels, centers);
    }
    (labels, device)
}

fn slic_assign_cpu(
    rgb: &[u8],
    w: usize,
    h: usize,
    centers: &[Center],
    step: f32,
    compact: f32,
    gw: usize,
) -> Vec<u32> {
    let n = w * h;
    let k = centers.len();
    let mut labels = vec![0u32; n];
    let reach = (2.0 * step) as i32;
    for y in 0..h {
        for x in 0..w {
            let gx = (x as f32 / step).floor() as i32;
            let gy = (y as f32 / step).floor() as i32;
            let f = cpu::feat(rgb, y * w + x);
            let mut best = 0u32;
            let mut best_d = f32::MAX;
            for oy in -1i32..=1 {
                for ox in -1i32..=1 {
                    let cx = gx + ox;
                    let cy = gy + oy;
                    if cx < 0 || cy < 0 {
                        continue;
                    }
                    let ci = cy as usize * gw + cx as usize;
                    if ci >= k {
                        continue;
                    }
                    let c = &centers[ci];
                    let dx = x as f32 - c.x;
                    let dy = y as f32 - c.y;
                    if dx.abs() > reach as f32 || dy.abs() > reach as f32 {
                        continue;
                    }
                    let dr = f[0] - c.r;
                    let dg = f[1] - c.g;
                    let db = f[2] - c.b;
                    let d = dr * dr + dg * dg + db * db + compact * (dx * dx + dy * dy);
                    if d < best_d {
                        best_d = d;
                        best = ci as u32;
                    }
                }
            }
            labels[y * w + x] = best;
        }
    }
    labels
}

fn update_centers(rgb: &[u8], w: usize, h: usize, labels: &[u32], centers: &mut [Center]) {
    let k = centers.len();
    let mut sx = vec![0.0f64; k];
    let mut sy = vec![0.0f64; k];
    let mut sr = vec![0.0f64; k];
    let mut sg = vec![0.0f64; k];
    let mut sb = vec![0.0f64; k];
    let mut cnt = vec![0.0f64; k];
    for y in 0..h {
        for x in 0..w {
            let lab = labels[y * w + x] as usize;
            if lab >= k {
                continue;
            }
            let i = y * w + x;
            sx[lab] += x as f64;
            sy[lab] += y as f64;
            sr[lab] += rgb[i * 3] as f64;
            sg[lab] += rgb[i * 3 + 1] as f64;
            sb[lab] += rgb[i * 3 + 2] as f64;
            cnt[lab] += 1.0;
        }
    }
    for i in 0..k {
        let c = cnt[i].max(1.0);
        centers[i].x = (sx[i] / c) as f32;
        centers[i].y = (sy[i] / c) as f32;
        centers[i].r = (sr[i] / c) as f32;
        centers[i].g = (sg[i] / c) as f32;
        centers[i].b = (sb[i] / c) as f32;
    }
}
