/// Make value crisp at device pixel boundaries
#[inline]
pub fn crisp(val: f64, dpr: f64) -> f64 {
    (val * dpr).round() / dpr + 0.5 / dpr
}

/// Make rectangle crisp at device pixel boundaries
#[inline]
pub fn crisp_rect(x: f64, y: f64, w: f64, h: f64, dpr: f64) -> (f64, f64, f64, f64) {
    let x1 = (x * dpr).round() / dpr;
    let y1 = (y * dpr).round() / dpr;
    let x2 = ((x + w) * dpr).round() / dpr;
    let y2 = ((y + h) * dpr).round() / dpr;
    (x1, y1, x2 - x1, y2 - y1)
}
