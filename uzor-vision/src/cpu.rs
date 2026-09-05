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
                    let drc = rgb[j] as f32 - cr;
                    let dgc = rgb[j + 1] as f32 - cg;
                    let dbc = rgb[j + 2] as f32 - cb;
                    let ds2 = (dx * dx + dy * dy) as f32;
                    let dc2 = drc * drc + dgc * dgc + dbc * dbc;
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

pub fn pack_rgb(rgb: &[u8], n: usize) -> Vec<u32> {
    let mut packed = vec![0u32; n];
    for i in 0..n {
        packed[i] = (rgb[i * 3] as u32) << 16
            | (rgb[i * 3 + 1] as u32) << 8
            | rgb[i * 3 + 2] as u32;
    }
    packed
}

pub fn unpack_rgb(packed: &[u32]) -> Vec<u8> {
    let mut out = vec![0u8; packed.len() * 3];
    for (i, &p) in packed.iter().enumerate() {
        out[i * 3] = (p >> 16) as u8;
        out[i * 3 + 1] = (p >> 8) as u8;
        out[i * 3 + 2] = p as u8;
    }
    out
}

pub fn feat(rgb: &[u8], i: usize) -> [f32; 3] {
    let r = rgb[i * 3] as f32;
    let g = rgb[i * 3 + 1] as f32;
    let b = rgb[i * 3 + 2] as f32;
    [r, g, b]
}
