//! SVG path string emitter — backend-agnostic, works with any [`Painter`].

use super::painter::Painter;

const ARC_SEGMENTS: usize = 32;

/// Parse an SVG path `d` string and emit path commands to the given painter.
///
/// Calls `ctx.begin_path()` first, then emits `move_to`, `line_to`,
/// `bezier_curve_to`, `quadratic_curve_to`, and `close_path` calls for each
/// recognised SVG command.  Coordinates are used as-is (identity transform —
/// no scaling or offsetting).
///
/// Supported commands: M m L l H h V v C c S s Q q T t A a Z z.
/// Unknown commands are silently skipped.
///
/// # Usage
///
/// ```ignore
/// emit_svg_path(ctx, "M 10 10 L 100 10 L 100 100 Z");
/// ctx.push_mask();
/// // draw clipped content …
/// ctx.pop_mask();
/// ```
pub fn emit_svg_path(ctx: &mut dyn Painter, d: &str) {
    emit_svg_path_generic(ctx, d);
}

/// Generic backing implementation — usable from a trait default without coercion.
///
/// Called by [`emit_svg_path`] (dyn) and by [`Masking::push_clip_svg_path`]
/// default impl (concrete `Self`).
pub(super) fn emit_svg_path_generic<P: Painter + ?Sized>(ctx: &mut P, d: &str) {
    ctx.begin_path();
    emit_svg_path_body(ctx, d);
}

fn emit_svg_path_body<P: Painter + ?Sized>(ctx: &mut P, d: &str) {

    let mut current_x = 0.0_f64;
    let mut current_y = 0.0_f64;
    let mut start_x = 0.0_f64;
    let mut start_y = 0.0_f64;
    let mut last_control: Option<(f64, f64)> = None;

    let mut chars = d.chars().peekable();
    let mut current_cmd = 'M';

    while chars.peek().is_some() {
        // Skip whitespace and commas between tokens.
        while chars
            .peek()
            .map(|c| c.is_whitespace() || *c == ',')
            .unwrap_or(false)
        {
            chars.next();
        }

        if let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                current_cmd = c;
                chars.next();
                while chars
                    .peek()
                    .map(|c| c.is_whitespace() || *c == ',')
                    .unwrap_or(false)
                {
                    chars.next();
                }
            }
        }

        match current_cmd {
            'M' => {
                if let Some((x, y)) = parse_two(&mut chars) {
                    current_x = x;
                    current_y = y;
                    start_x = x;
                    start_y = y;
                    ctx.move_to(x, y);
                    current_cmd = 'L';
                    last_control = None;
                }
            }
            'm' => {
                if let Some((dx, dy)) = parse_two(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    start_x = current_x;
                    start_y = current_y;
                    ctx.move_to(current_x, current_y);
                    current_cmd = 'l';
                    last_control = None;
                }
            }
            'L' => {
                if let Some((x, y)) = parse_two(&mut chars) {
                    current_x = x;
                    current_y = y;
                    ctx.line_to(x, y);
                    last_control = None;
                }
            }
            'l' => {
                if let Some((dx, dy)) = parse_two(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    ctx.line_to(current_x, current_y);
                    last_control = None;
                }
            }
            'H' => {
                if let Some(x) = parse_one(&mut chars) {
                    current_x = x;
                    ctx.line_to(current_x, current_y);
                    last_control = None;
                }
            }
            'h' => {
                if let Some(dx) = parse_one(&mut chars) {
                    current_x += dx;
                    ctx.line_to(current_x, current_y);
                    last_control = None;
                }
            }
            'V' => {
                if let Some(y) = parse_one(&mut chars) {
                    current_y = y;
                    ctx.line_to(current_x, current_y);
                    last_control = None;
                }
            }
            'v' => {
                if let Some(dy) = parse_one(&mut chars) {
                    current_y += dy;
                    ctx.line_to(current_x, current_y);
                    last_control = None;
                }
            }
            'C' => {
                if let Some((c1x, c1y, c2x, c2y, x, y)) = parse_six(&mut chars) {
                    ctx.bezier_curve_to(c1x, c1y, c2x, c2y, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'c' => {
                if let Some((dc1x, dc1y, dc2x, dc2y, dx, dy)) = parse_six(&mut chars) {
                    let c1x = current_x + dc1x;
                    let c1y = current_y + dc1y;
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.bezier_curve_to(c1x, c1y, c2x, c2y, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'S' => {
                if let Some((c2x, c2y, x, y)) = parse_four(&mut chars) {
                    let (c1x, c1y) = last_control
                        .map(|(lx, ly)| (2.0 * current_x - lx, 2.0 * current_y - ly))
                        .unwrap_or((current_x, current_y));
                    ctx.bezier_curve_to(c1x, c1y, c2x, c2y, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            's' => {
                if let Some((dc2x, dc2y, dx, dy)) = parse_four(&mut chars) {
                    let (c1x, c1y) = last_control
                        .map(|(lx, ly)| (2.0 * current_x - lx, 2.0 * current_y - ly))
                        .unwrap_or((current_x, current_y));
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.bezier_curve_to(c1x, c1y, c2x, c2y, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'Q' => {
                if let Some((cx, cy, x, y)) = parse_four(&mut chars) {
                    ctx.quadratic_curve_to(cx, cy, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'q' => {
                if let Some((dcx, dcy, dx, dy)) = parse_four(&mut chars) {
                    let cx = current_x + dcx;
                    let cy = current_y + dcy;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(cx, cy, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'T' => {
                if let Some((x, y)) = parse_two(&mut chars) {
                    let (cx, cy) = last_control
                        .map(|(lx, ly)| (2.0 * current_x - lx, 2.0 * current_y - ly))
                        .unwrap_or((current_x, current_y));
                    ctx.quadratic_curve_to(cx, cy, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            't' => {
                if let Some((dx, dy)) = parse_two(&mut chars) {
                    let (cx, cy) = last_control
                        .map(|(lx, ly)| (2.0 * current_x - lx, 2.0 * current_y - ly))
                        .unwrap_or((current_x, current_y));
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(cx, cy, x, y);
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'A' | 'a' => {
                let relative = current_cmd == 'a';
                if let Some((rx, ry, rotation, large, sweep, x, y)) = parse_arc_params(&mut chars) {
                    let (end_x, end_y) = if relative {
                        (current_x + x, current_y + y)
                    } else {
                        (x, y)
                    };
                    let pts = arc_to_points(
                        current_x,
                        current_y,
                        rx,
                        ry,
                        rotation,
                        large != 0.0,
                        sweep != 0.0,
                        end_x,
                        end_y,
                    );
                    for (px, py) in pts {
                        ctx.line_to(px, py);
                    }
                    current_x = end_x;
                    current_y = end_y;
                    last_control = None;
                }
            }
            'Z' | 'z' => {
                ctx.close_path();
                current_x = start_x;
                current_y = start_y;
                last_control = None;
            }
            _ => {
                chars.next();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Number parsers (self-contained — no dependency on svg.rs internals)
// ---------------------------------------------------------------------------

fn parse_one(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
    skip_sep(chars);
    let mut s = String::new();
    if chars.peek() == Some(&'-') || chars.peek() == Some(&'+') {
        s.push(chars.next().unwrap());
    }
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' {
            s.push(chars.next().unwrap());
        } else {
            break;
        }
    }
    // Handle exponent (e.g. 1e-4)
    if chars.peek() == Some(&'e') || chars.peek() == Some(&'E') {
        s.push(chars.next().unwrap());
        if chars.peek() == Some(&'-') || chars.peek() == Some(&'+') {
            s.push(chars.next().unwrap());
        }
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                s.push(chars.next().unwrap());
            } else {
                break;
            }
        }
    }
    s.parse().ok()
}

fn skip_sep(chars: &mut std::iter::Peekable<std::str::Chars>) {
    loop {
        match chars.peek() {
            Some(&c) if c.is_whitespace() || c == ',' => {
                chars.next();
            }
            Some(&'&') => {
                // Skip XML entities like &#xA;
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == ';' {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
}

fn parse_two(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<(f64, f64)> {
    Some((parse_one(chars)?, parse_one(chars)?))
}

fn parse_four(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<(f64, f64, f64, f64)> {
    Some((
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
    ))
}

fn parse_six(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<(f64, f64, f64, f64, f64, f64)> {
    Some((
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
    ))
}

fn parse_arc_params(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<(f64, f64, f64, f64, f64, f64, f64)> {
    Some((
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
        parse_one(chars)?,
    ))
}

// ---------------------------------------------------------------------------
// Arc helper (SVG spec §B.2.4 — parametric arc conversion)
// ---------------------------------------------------------------------------

fn arc_to_points(
    start_x: f64,
    start_y: f64,
    mut rx: f64,
    mut ry: f64,
    x_rotation: f64,
    large_arc: bool,
    sweep: bool,
    end_x: f64,
    end_y: f64,
) -> Vec<(f64, f64)> {
    let mut points = Vec::new();

    if (start_x - end_x).abs() < 0.001 && (start_y - end_y).abs() < 0.001 {
        return points;
    }

    rx = rx.abs();
    ry = ry.abs();

    if rx < 0.001 || ry < 0.001 {
        points.push((end_x, end_y));
        return points;
    }

    let phi = x_rotation.to_radians();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    let dx = (start_x - end_x) / 2.0;
    let dy = (start_y - end_y) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let rx2 = rx * rx;
    let ry2 = ry * ry;

    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    let rx2 = rx * rx;
    let ry2 = ry * ry;

    let num = rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2;
    let denom = rx2 * y1p2 + ry2 * x1p2;

    let factor = if denom > 0.0 && num > 0.0 {
        let mut f = (num / denom).sqrt();
        if large_arc == sweep {
            f = -f;
        }
        f
    } else {
        0.0
    };

    let cxp = factor * rx * y1p / ry;
    let cyp = -factor * ry * x1p / rx;

    let cx = cos_phi * cxp - sin_phi * cyp + (start_x + end_x) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (start_y + end_y) / 2.0;

    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;

    let n = (ux * ux + uy * uy).sqrt();
    let theta1 = if uy < 0.0 { -1.0_f64 } else { 1.0 } * (ux / n).clamp(-1.0, 1.0).acos();

    let n = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
    let dot = ux * vx + uy * vy;
    let mut dtheta =
        if ux * vy - uy * vx < 0.0 { -1.0_f64 } else { 1.0 }
        * (dot / n).clamp(-1.0, 1.0).acos();

    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * std::f64::consts::PI;
    } else if sweep && dtheta < 0.0 {
        dtheta += 2.0 * std::f64::consts::PI;
    }

    for i in 1..=ARC_SEGMENTS {
        let t = i as f64 / ARC_SEGMENTS as f64;
        let theta = theta1 + dtheta * t;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let px = rx * cos_t;
        let py = ry * sin_t;
        let x = cos_phi * px - sin_phi * py + cx;
        let y = sin_phi * px + cos_phi * py + cy;
        points.push((x, y));
    }

    points
}
