pub fn chroma(c: [u8; 3]) -> i32 {
    let max = c[0].max(c[1]).max(c[2]) as i32;
    let min = c[0].min(c[1]).min(c[2]) as i32;
    max - min
}

pub fn hue_compatible(a: [u8; 3], b: [u8; 3]) -> bool {
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
    dot * dot * 4 >= na * nb * 3
}

/// Cream is not a universal solvent.
pub fn same_flat(a: [u8; 3], b: [u8; 3]) -> bool {
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

pub fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}
