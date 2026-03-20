use super::context::RenderContext;

/// Draw an SVG icon scaled to fit within the given rectangle.
///
/// The SVG is parsed and rendered using the stroke color.
/// Supports: path, circle, rect, line, polyline, polygon elements.
///
/// # Arguments
/// * `ctx` - Render context
/// * `svg` - SVG string content
/// * `x`, `y` - Top-left corner position
/// * `width`, `height` - Target dimensions
/// * `color` - Stroke color (hex string)
pub fn draw_svg_icon(ctx: &mut dyn RenderContext, svg: &str, x: f64, y: f64, width: f64, height: f64, color: &str) {
    // Parse viewBox to get source dimensions (default 24x24)
    let (vb_width, vb_height) = parse_viewbox(svg).unwrap_or((24.0, 24.0));

    // Calculate scale and offset for centering
    let scale_x = width / vb_width;
    let scale_y = height / vb_height;
    let scale = scale_x.min(scale_y); // Uniform scale to fit

    let offset_x = (x + (width - vb_width * scale) / 2.0).floor();
    let offset_y = (y + (height - vb_height * scale) / 2.0).floor();

    // Check if root SVG has fill="none" - if so, children default to stroke-only
    // This is the SVG inheritance model: fill="none" on root means no fill unless overridden
    let has_fill_none = svg_root_has_fill_none(svg);
    let default_filled = !has_fill_none;

    // Fixed stroke width for crisp rendering — round to nearest 0.5 for pixel-aligned strokes
    let stroke_width = (1.5 * scale * 2.0).round() / 2.0;

    // Set stroke style
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(stroke_width);
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.set_line_dash(&[]);

    // Parse and render all path elements
    for path_info in parse_svg_paths(svg, default_filled) {
        ctx.begin_path();
        render_path_data(ctx, &path_info.d, offset_x, offset_y, scale);

        // Fill first, then stroke (so stroke is on top)
        if path_info.filled {
            ctx.set_fill_color(color);
            ctx.fill();
        }
        if path_info.stroked {
            // Apply dash array if present (scaled)
            if let Some(ref dash) = path_info.dash_array {
                let scaled_dash: Vec<f64> = dash.iter().map(|d| d * scale).collect();
                ctx.set_line_dash(&scaled_dash);
            }
            ctx.stroke();
            // Reset dash after stroke
            if path_info.dash_array.is_some() {
                ctx.set_line_dash(&[]);
            }
        }
    }

    // Parse and render all circle elements
    for (cx, cy, r, filled) in parse_svg_circles(svg, default_filled) {
        let tx = offset_x + cx * scale;
        let ty = offset_y + cy * scale;
        let tr = r * scale;

        ctx.begin_path();
        ctx.arc(tx, ty, tr, 0.0, std::f64::consts::PI * 2.0);
        if filled {
            ctx.set_fill_color(color);
            ctx.fill();
        } else {
            ctx.stroke();
        }
    }

    // Parse and render all rect elements
    for (rx, ry, rw, rh, rounding, filled) in parse_svg_rects(svg, default_filled) {
        let tx = offset_x + rx * scale;
        let ty = offset_y + ry * scale;
        let tw = rw * scale;
        let th = rh * scale;
        let tr = rounding * scale;

        if filled {
            ctx.set_fill_color(color);
            if tr > 0.0 {
                ctx.fill_rounded_rect(tx, ty, tw, th, tr);
            } else {
                ctx.fill_rect(tx, ty, tw, th);
            }
        } else if tr > 0.0 {
            ctx.stroke_rounded_rect(tx, ty, tw, th, tr);
        } else {
            ctx.stroke_rect(tx, ty, tw, th);
        }
    }

    // Parse and render all line elements
    for (x1, y1, x2, y2) in parse_svg_lines(svg) {
        let tx1 = offset_x + x1 * scale;
        let ty1 = offset_y + y1 * scale;
        let tx2 = offset_x + x2 * scale;
        let ty2 = offset_y + y2 * scale;

        ctx.begin_path();
        ctx.move_to(tx1, ty1);
        ctx.line_to(tx2, ty2);
        ctx.stroke();
    }

    // Parse and render all polyline elements
    for (points, closed) in parse_svg_polylines(svg) {
        if points.len() >= 2 {
            ctx.begin_path();
            let (px, py) = points[0];
            ctx.move_to(offset_x + px * scale, offset_y + py * scale);
            for &(px, py) in &points[1..] {
                ctx.line_to(offset_x + px * scale, offset_y + py * scale);
            }
            if closed {
                ctx.close_path();
            }
            ctx.stroke();
        }
    }
}

// =============================================================================
// SVG Path Parsing
// =============================================================================

/// Parse viewBox from SVG string
/// Returns (width, height) or None if not found
fn parse_viewbox(svg: &str) -> Option<(f64, f64)> {
    // Look for viewBox="x y w h"
    let vb_start = svg.find("viewBox=\"")?;
    let vb_content_start = vb_start + 9;
    let vb_end = svg[vb_content_start..].find('"')?;
    let vb_str = &svg[vb_content_start..vb_content_start + vb_end];

    let parts: Vec<&str> = vb_str.split_whitespace().collect();
    if parts.len() >= 4 {
        let w = parts[2].parse::<f64>().ok()?;
        let h = parts[3].parse::<f64>().ok()?;
        Some((w, h))
    } else {
        None
    }
}

/// Path rendering info
struct PathInfo {
    d: String,
    filled: bool,
    stroked: bool,
    dash_array: Option<Vec<f64>>,
}

/// Extract all path elements from SVG with fill/stroke info
/// `default_filled` is inherited from parent SVG element
fn parse_svg_paths(svg: &str, default_filled: bool) -> Vec<PathInfo> {
    let mut paths = Vec::new();
    let mut search_from = 0;

    while let Some(start) = svg[search_from..].find("<path") {
        let abs_start = search_from + start;
        // Find end of tag
        let tag_end = if let Some(end) = svg[abs_start..].find("/>") {
            abs_start + end + 2
        } else if let Some(end) = svg[abs_start..].find('>') {
            abs_start + end + 1
        } else {
            break;
        };

        let tag_content = &svg[abs_start..tag_end];

        // Extract d attribute
        if let Some(d_start) = tag_content.find(" d=\"") {
            let d_content_start = d_start + 4;
            if let Some(d_end) = tag_content[d_content_start..].find('"') {
                let d = tag_content[d_content_start..d_content_start + d_end].to_string();

                // Check fill attribute
                let filled = if let Some(fill_start) = tag_content.find("fill=\"") {
                    let fill_content_start = fill_start + 6;
                    if let Some(fill_end) = tag_content[fill_content_start..].find('"') {
                        let fill_value = &tag_content[fill_content_start..fill_content_start + fill_end];
                        fill_value != "none"
                    } else {
                        false
                    }
                } else {
                    default_filled // Use inherited default from root SVG
                };

                // Check stroke attribute (default is stroked for icons)
                let stroked = if let Some(stroke_start) = tag_content.find("stroke=\"") {
                    let stroke_content_start = stroke_start + 8;
                    if let Some(stroke_end) = tag_content[stroke_content_start..].find('"') {
                        let stroke_value = &tag_content[stroke_content_start..stroke_content_start + stroke_end];
                        stroke_value != "none"
                    } else {
                        true
                    }
                } else {
                    !filled // If not filled, assume stroked
                };

                // Check stroke-dasharray attribute (e.g., "4 2" for dashed lines)
                let dash_array = if let Some(dash_start) = tag_content.find("stroke-dasharray=\"") {
                    let dash_content_start = dash_start + 18;
                    if let Some(dash_end) = tag_content[dash_content_start..].find('"') {
                        let dash_value = &tag_content[dash_content_start..dash_content_start + dash_end];
                        // Parse "4 2" or "4,2" format
                        let values: Vec<f64> = dash_value
                            .split([' ', ','])
                            .filter_map(|s| s.trim().parse::<f64>().ok())
                            .collect();
                        if !values.is_empty() {
                            Some(values)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                paths.push(PathInfo { d, filled, stroked, dash_array });
            }
        }

        search_from = tag_end;
    }

    paths
}

/// Number of segments for arc approximation
const ARC_SEGMENTS: usize = 16;

/// Convert SVG arc parameters to a series of points
/// Based on the SVG arc to bezier algorithm
#[allow(clippy::too_many_arguments)]
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

    // Handle degenerate cases
    if (start_x - end_x).abs() < 0.001 && (start_y - end_y).abs() < 0.001 {
        return points;
    }

    rx = rx.abs();
    ry = ry.abs();

    if rx < 0.001 || ry < 0.001 {
        // Straight line
        points.push((end_x, end_y));
        return points;
    }

    let phi = x_rotation.to_radians();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    // Step 1: Compute (x1', y1')
    let dx = (start_x - end_x) / 2.0;
    let dy = (start_y - end_y) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    // Step 2: Compute (cx', cy')
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let rx2 = rx * rx;
    let ry2 = ry * ry;

    // Correct radii if needed
    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let sqrt_lambda = lambda.sqrt();
        rx *= sqrt_lambda;
        ry *= sqrt_lambda;
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

    // Step 3: Compute (cx, cy) from (cx', cy')
    let cx = cos_phi * cxp - sin_phi * cyp + (start_x + end_x) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (start_y + end_y) / 2.0;

    // Step 4: Compute angles
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;

    // Angle start
    let n = (ux * ux + uy * uy).sqrt();
    let theta1 = if uy < 0.0 { -1.0 } else { 1.0 } * (ux / n).clamp(-1.0, 1.0).acos();

    // Angle extent
    let n = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
    let dot = ux * vx + uy * vy;
    let mut dtheta = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 } * (dot / n).clamp(-1.0, 1.0).acos();

    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * std::f64::consts::PI;
    } else if sweep && dtheta < 0.0 {
        dtheta += 2.0 * std::f64::consts::PI;
    }

    // Generate points along the arc
    for i in 1..=ARC_SEGMENTS {
        let t = i as f64 / ARC_SEGMENTS as f64;
        let theta = theta1 + dtheta * t;

        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        // Point on unit circle, scaled by radii
        let px = rx * cos_theta;
        let py = ry * sin_theta;

        // Rotate and translate
        let x = cos_phi * px - sin_phi * py + cx;
        let y = sin_phi * px + cos_phi * py + cy;

        points.push((x, y));
    }

    points
}

/// Render SVG path data onto a RenderContext
fn render_path_data(ctx: &mut dyn RenderContext, path_data: &str, offset_x: f64, offset_y: f64, scale: f64) {
    let mut current_x = 0.0;
    let mut current_y = 0.0;
    let mut start_x = 0.0;
    let mut start_y = 0.0;
    let mut last_control: Option<(f64, f64)> = None; // For smooth curves (S, T)

    let mut chars = path_data.chars().peekable();
    let mut current_cmd = 'M';

    while chars.peek().is_some() {
        // Skip whitespace and commas
        while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
            chars.next();
        }

        // Check for command
        if let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                current_cmd = c;
                chars.next();
                // Skip whitespace after command
                while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
                    chars.next();
                }
            }
        }

        match current_cmd {
            'M' => {
                // Absolute move
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    start_x = x;
                    start_y = y;
                    ctx.move_to(offset_x + x * scale, offset_y + y * scale);
                    current_cmd = 'L'; // Subsequent coordinates are line-to
                    last_control = None;
                }
            }
            'm' => {
                // Relative move
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    start_x = current_x;
                    start_y = current_y;
                    ctx.move_to(offset_x + current_x * scale, offset_y + current_y * scale);
                    current_cmd = 'l'; // Subsequent coordinates are relative line-to
                    last_control = None;
                }
            }
            'L' => {
                // Absolute line
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    ctx.line_to(offset_x + x * scale, offset_y + y * scale);
                    last_control = None;
                }
            }
            'l' => {
                // Relative line
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    ctx.line_to(offset_x + current_x * scale, offset_y + current_y * scale);
                    last_control = None;
                }
            }
            'H' => {
                // Absolute horizontal line
                if let Some(x) = parse_number(&mut chars) {
                    current_x = x;
                    ctx.line_to(offset_x + x * scale, offset_y + current_y * scale);
                    last_control = None;
                }
            }
            'h' => {
                // Relative horizontal line
                if let Some(dx) = parse_number(&mut chars) {
                    current_x += dx;
                    ctx.line_to(offset_x + current_x * scale, offset_y + current_y * scale);
                    last_control = None;
                }
            }
            'V' => {
                // Absolute vertical line
                if let Some(y) = parse_number(&mut chars) {
                    current_y = y;
                    ctx.line_to(offset_x + current_x * scale, offset_y + y * scale);
                    last_control = None;
                }
            }
            'v' => {
                // Relative vertical line
                if let Some(dy) = parse_number(&mut chars) {
                    current_y += dy;
                    ctx.line_to(offset_x + current_x * scale, offset_y + current_y * scale);
                    last_control = None;
                }
            }
            'C' => {
                // Absolute cubic bezier
                if let Some((c1x, c1y, c2x, c2y, x, y)) = parse_six_numbers(&mut chars) {
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale,
                        offset_y + c1y * scale,
                        offset_x + c2x * scale,
                        offset_y + c2y * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'c' => {
                // Relative cubic bezier
                if let Some((dc1x, dc1y, dc2x, dc2y, dx, dy)) = parse_six_numbers(&mut chars) {
                    let c1x = current_x + dc1x;
                    let c1y = current_y + dc1y;
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale,
                        offset_y + c1y * scale,
                        offset_x + c2x * scale,
                        offset_y + c2y * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'S' => {
                // Smooth cubic bezier (absolute)
                if let Some((c2x, c2y, x, y)) = parse_four_numbers(&mut chars) {
                    // Reflect last control point
                    let (c1x, c1y) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale,
                        offset_y + c1y * scale,
                        offset_x + c2x * scale,
                        offset_y + c2y * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            's' => {
                // Smooth cubic bezier (relative)
                if let Some((dc2x, dc2y, dx, dy)) = parse_four_numbers(&mut chars) {
                    let (c1x, c1y) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale,
                        offset_y + c1y * scale,
                        offset_x + c2x * scale,
                        offset_y + c2y * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'Q' => {
                // Absolute quadratic bezier
                if let Some((cx, cy, x, y)) = parse_four_numbers(&mut chars) {
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale,
                        offset_y + cy * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'q' => {
                // Relative quadratic bezier
                if let Some((dcx, dcy, dx, dy)) = parse_four_numbers(&mut chars) {
                    let cx = current_x + dcx;
                    let cy = current_y + dcy;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale,
                        offset_y + cy * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'T' => {
                // Smooth quadratic bezier (absolute)
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    let (cx, cy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale,
                        offset_y + cy * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            't' => {
                // Smooth quadratic bezier (relative)
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    let (cx, cy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale,
                        offset_y + cy * scale,
                        offset_x + x * scale,
                        offset_y + y * scale,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'A' | 'a' => {
                // Arc command: rx ry x-rotation large-arc-flag sweep-flag x y
                let is_relative = current_cmd == 'a';
                if let Some((rx, ry, rotation, large, sweep, x, y)) = parse_arc_params(&mut chars) {
                    let (end_x, end_y) = if is_relative {
                        (current_x + x, current_y + y)
                    } else {
                        (x, y)
                    };

                    // Convert arc to points and draw them
                    let arc_points = arc_to_points(
                        current_x, current_y,
                        rx, ry,
                        rotation,
                        large != 0.0,
                        sweep != 0.0,
                        end_x, end_y,
                    );

                    for (px, py) in arc_points {
                        ctx.line_to(offset_x + px * scale, offset_y + py * scale);
                    }

                    current_x = end_x;
                    current_y = end_y;
                    last_control = None;
                }
            }
            'Z' | 'z' => {
                // Close path
                ctx.close_path();
                current_x = start_x;
                current_y = start_y;
                last_control = None;
            }
            _ => {
                // Unknown command, skip
                chars.next();
            }
        }
    }
}

fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
    // Skip whitespace, commas, AND XML entities (&#xA; &#x9; etc.)
    loop {
        match chars.peek() {
            Some(&c) if c.is_whitespace() || c == ',' => { chars.next(); }
            Some(&'&') => {
                // Skip XML entity like &#xA; or &#x9;
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == ';' { break; }
                }
            }
            _ => break,
        }
    }

    let mut num_str = String::new();

    // Handle sign
    if let Some(&c) = chars.peek() {
        if c == '-' || c == '+' {
            num_str.push(chars.next().unwrap());
        }
    }

    // Collect digits and decimal point
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' {
            num_str.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    num_str.parse::<f64>().ok()
}

fn parse_two_numbers(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<(f64, f64)> {
    let x = parse_number(chars)?;
    let y = parse_number(chars)?;
    Some((x, y))
}

fn parse_four_numbers(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<(f64, f64, f64, f64)> {
    let a = parse_number(chars)?;
    let b = parse_number(chars)?;
    let c = parse_number(chars)?;
    let d = parse_number(chars)?;
    Some((a, b, c, d))
}

fn parse_six_numbers(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let a = parse_number(chars)?;
    let b = parse_number(chars)?;
    let c = parse_number(chars)?;
    let d = parse_number(chars)?;
    let e = parse_number(chars)?;
    let f = parse_number(chars)?;
    Some((a, b, c, d, e, f))
}

fn parse_arc_params(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<(f64, f64, f64, f64, f64, f64, f64)> {
    let rx = parse_number(chars)?;
    let ry = parse_number(chars)?;
    let rotation = parse_number(chars)?;
    let large_arc = parse_number(chars)?;
    let sweep = parse_number(chars)?;
    let x = parse_number(chars)?;
    let y = parse_number(chars)?;
    Some((rx, ry, rotation, large_arc, sweep, x, y))
}

// =============================================================================
// SVG Element Parsing (circle, rect, line, polyline, polygon)
// =============================================================================

/// Extract attribute value from SVG tag content
fn extract_svg_attr(content: &str, attr: &str) -> Option<f64> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = content.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = content[value_start..].find('"') {
            return content[value_start..value_start + end].parse().ok();
        }
    }
    None
}

/// Check if element has fill="none" (stroked) or not (filled)
/// `default_filled` is the inherited fill state from parent SVG element
fn is_svg_filled_with_default(content: &str, default_filled: bool) -> bool {
    if let Some(start) = content.find("fill=\"") {
        let value_start = start + 6;
        if let Some(end) = content[value_start..].find('"') {
            let fill_value = &content[value_start..value_start + end];
            return fill_value != "none";
        }
    }
    // No fill attribute - use inherited default
    default_filled
}

/// Check if root SVG element has fill="none" (meaning children default to stroke-only)
fn svg_root_has_fill_none(svg: &str) -> bool {
    // Find the root <svg> tag
    if let Some(start) = svg.find("<svg") {
        if let Some(end) = svg[start..].find('>') {
            let svg_tag = &svg[start..start + end + 1];
            if let Some(fill_start) = svg_tag.find("fill=\"") {
                let value_start = fill_start + 6;
                if let Some(fill_end) = svg_tag[value_start..].find('"') {
                    let fill_value = &svg_tag[value_start..value_start + fill_end];
                    return fill_value == "none";
                }
            }
        }
    }
    false
}

/// Parse all <circle> elements from SVG
/// Returns Vec of (cx, cy, r, filled)
/// `default_filled` is inherited from parent SVG element
fn parse_svg_circles(svg: &str, default_filled: bool) -> Vec<(f64, f64, f64, bool)> {
    let mut circles = Vec::new();
    let mut search_from = 0;

    while let Some(start) = svg[search_from..].find("<circle") {
        let abs_start = search_from + start;
        // Find end of tag
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let cx = extract_svg_attr(tag_content, "cx").unwrap_or(0.0);
            let cy = extract_svg_attr(tag_content, "cy").unwrap_or(0.0);
            let r = extract_svg_attr(tag_content, "r").unwrap_or(0.0);
            let filled = is_svg_filled_with_default(tag_content, default_filled);

            if r > 0.0 {
                circles.push((cx, cy, r, filled));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let cx = extract_svg_attr(tag_content, "cx").unwrap_or(0.0);
            let cy = extract_svg_attr(tag_content, "cy").unwrap_or(0.0);
            let r = extract_svg_attr(tag_content, "r").unwrap_or(0.0);
            let filled = is_svg_filled_with_default(tag_content, default_filled);

            if r > 0.0 {
                circles.push((cx, cy, r, filled));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    circles
}

/// Parse all <rect> elements from SVG
/// Returns Vec of (x, y, width, height, rx/rounding, filled)
/// `default_filled` is inherited from parent SVG element
fn parse_svg_rects(svg: &str, default_filled: bool) -> Vec<(f64, f64, f64, f64, f64, bool)> {
    let mut rects = Vec::new();
    let mut search_from = 0;

    while let Some(start) = svg[search_from..].find("<rect") {
        let abs_start = search_from + start;
        // Find end of tag
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let x = extract_svg_attr(tag_content, "x").unwrap_or(0.0);
            let y = extract_svg_attr(tag_content, "y").unwrap_or(0.0);
            let w = extract_svg_attr(tag_content, "width").unwrap_or(0.0);
            let h = extract_svg_attr(tag_content, "height").unwrap_or(0.0);
            let rx = extract_svg_attr(tag_content, "rx").unwrap_or(0.0);
            let filled = is_svg_filled_with_default(tag_content, default_filled);

            if w > 0.0 && h > 0.0 {
                rects.push((x, y, w, h, rx, filled));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let x = extract_svg_attr(tag_content, "x").unwrap_or(0.0);
            let y = extract_svg_attr(tag_content, "y").unwrap_or(0.0);
            let w = extract_svg_attr(tag_content, "width").unwrap_or(0.0);
            let h = extract_svg_attr(tag_content, "height").unwrap_or(0.0);
            let rx = extract_svg_attr(tag_content, "rx").unwrap_or(0.0);
            let filled = is_svg_filled_with_default(tag_content, default_filled);

            if w > 0.0 && h > 0.0 {
                rects.push((x, y, w, h, rx, filled));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    rects
}

/// Parse all <line> elements from SVG
/// Returns Vec of (x1, y1, x2, y2)
fn parse_svg_lines(svg: &str) -> Vec<(f64, f64, f64, f64)> {
    let mut lines = Vec::new();
    let mut search_from = 0;

    while let Some(start) = svg[search_from..].find("<line") {
        let abs_start = search_from + start;
        // Find end of tag
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let x1 = extract_svg_attr(tag_content, "x1").unwrap_or(0.0);
            let y1 = extract_svg_attr(tag_content, "y1").unwrap_or(0.0);
            let x2 = extract_svg_attr(tag_content, "x2").unwrap_or(0.0);
            let y2 = extract_svg_attr(tag_content, "y2").unwrap_or(0.0);

            lines.push((x1, y1, x2, y2));
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let x1 = extract_svg_attr(tag_content, "x1").unwrap_or(0.0);
            let y1 = extract_svg_attr(tag_content, "y1").unwrap_or(0.0);
            let x2 = extract_svg_attr(tag_content, "x2").unwrap_or(0.0);
            let y2 = extract_svg_attr(tag_content, "y2").unwrap_or(0.0);

            lines.push((x1, y1, x2, y2));
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    lines
}

/// Extract points attribute from polyline/polygon
fn extract_svg_points(content: &str) -> Vec<(f64, f64)> {
    let mut points = Vec::new();

    if let Some(start) = content.find("points=\"") {
        let value_start = start + 8;
        if let Some(end) = content[value_start..].find('"') {
            let points_str = &content[value_start..value_start + end];
            // Parse "x1,y1 x2,y2 x3,y3" format
            let mut chars = points_str.chars().peekable();

            loop {
                // Skip whitespace
                while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                    chars.next();
                }

                if chars.peek().is_none() {
                    break;
                }

                // Parse number for x
                let mut num_str = String::new();
                if chars.peek() == Some(&'-') || chars.peek() == Some(&'+') {
                    num_str.push(chars.next().unwrap());
                }
                while chars.peek().map(|c| c.is_ascii_digit() || *c == '.').unwrap_or(false) {
                    num_str.push(chars.next().unwrap());
                }
                let x: f64 = match num_str.parse() {
                    Ok(v) => v,
                    Err(_) => break,
                };

                // Skip comma or space
                while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
                    chars.next();
                }

                // Parse number for y
                let mut num_str = String::new();
                if chars.peek() == Some(&'-') || chars.peek() == Some(&'+') {
                    num_str.push(chars.next().unwrap());
                }
                while chars.peek().map(|c| c.is_ascii_digit() || *c == '.').unwrap_or(false) {
                    num_str.push(chars.next().unwrap());
                }
                let y: f64 = match num_str.parse() {
                    Ok(v) => v,
                    Err(_) => break,
                };

                points.push((x, y));

                // Skip comma or space
                while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
                    chars.next();
                }
            }
        }
    }

    points
}

/// Parse all <polyline> and <polygon> elements from SVG
/// Returns Vec of (points, closed)
fn parse_svg_polylines(svg: &str) -> Vec<(Vec<(f64, f64)>, bool)> {
    let mut polylines = Vec::new();

    // Parse polylines (not closed)
    let mut search_from = 0;
    while let Some(start) = svg[search_from..].find("<polyline") {
        let abs_start = search_from + start;
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let points = extract_svg_points(tag_content);
            if !points.is_empty() {
                polylines.push((points, false));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let points = extract_svg_points(tag_content);
            if !points.is_empty() {
                polylines.push((points, false));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    // Parse polygons (closed)
    search_from = 0;
    while let Some(start) = svg[search_from..].find("<polygon") {
        let abs_start = search_from + start;
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let points = extract_svg_points(tag_content);
            if !points.is_empty() {
                polylines.push((points, true));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let points = extract_svg_points(tag_content);
            if !points.is_empty() {
                polylines.push((points, true));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    polylines
}

/// Rotate point (px, py) around center (cx, cy) by angle (sin_a, cos_a precomputed)
#[inline]
fn rotate_pt(px: f64, py: f64, cx: f64, cy: f64, sin_a: f64, cos_a: f64) -> (f64, f64) {
    let dx = px - cx;
    let dy = py - cy;
    (cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)
}

/// Render SVG path data with rotation applied to every coordinate
#[allow(clippy::too_many_arguments)]
fn render_path_data_rotated(
    ctx: &mut dyn RenderContext, path_data: &str,
    offset_x: f64, offset_y: f64, scale: f64,
    cx: f64, cy: f64, sin_a: f64, cos_a: f64,
) {
    let mut current_x = 0.0;
    let mut current_y = 0.0;
    let mut start_x = 0.0;
    let mut start_y = 0.0;
    let mut last_control: Option<(f64, f64)> = None;

    let mut chars = path_data.chars().peekable();
    let mut current_cmd = 'M';

    while chars.peek().is_some() {
        // Skip whitespace and commas
        while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
            chars.next();
        }

        // Check for command
        if let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                current_cmd = c;
                chars.next();
                while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
                    chars.next();
                }
            }
        }

        match current_cmd {
            'M' => {
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    start_x = x;
                    start_y = y;
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.move_to(rx, ry);
                    current_cmd = 'L';
                    last_control = None;
                }
            }
            'm' => {
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    start_x = current_x;
                    start_y = current_y;
                    let (rx, ry) = rotate_pt(offset_x + current_x * scale, offset_y + current_y * scale, cx, cy, sin_a, cos_a);
                    ctx.move_to(rx, ry);
                    current_cmd = 'l';
                    last_control = None;
                }
            }
            'L' => {
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'l' => {
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    let (rx, ry) = rotate_pt(offset_x + current_x * scale, offset_y + current_y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'H' => {
                if let Some(x) = parse_number(&mut chars) {
                    current_x = x;
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + current_y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'h' => {
                if let Some(dx) = parse_number(&mut chars) {
                    current_x += dx;
                    let (rx, ry) = rotate_pt(offset_x + current_x * scale, offset_y + current_y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'V' => {
                if let Some(y) = parse_number(&mut chars) {
                    current_y = y;
                    let (rx, ry) = rotate_pt(offset_x + current_x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'v' => {
                if let Some(dy) = parse_number(&mut chars) {
                    current_y += dy;
                    let (rx, ry) = rotate_pt(offset_x + current_x * scale, offset_y + current_y * scale, cx, cy, sin_a, cos_a);
                    ctx.line_to(rx, ry);
                    last_control = None;
                }
            }
            'C' => {
                if let Some((c1x, c1y, c2x, c2y, x, y)) = parse_six_numbers(&mut chars) {
                    let (r1x, r1y) = rotate_pt(offset_x + c1x * scale, offset_y + c1y * scale, cx, cy, sin_a, cos_a);
                    let (r2x, r2y) = rotate_pt(offset_x + c2x * scale, offset_y + c2y * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.bezier_curve_to(r1x, r1y, r2x, r2y, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'c' => {
                if let Some((dc1x, dc1y, dc2x, dc2y, dx, dy)) = parse_six_numbers(&mut chars) {
                    let c1x = current_x + dc1x;
                    let c1y = current_y + dc1y;
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    let (r1x, r1y) = rotate_pt(offset_x + c1x * scale, offset_y + c1y * scale, cx, cy, sin_a, cos_a);
                    let (r2x, r2y) = rotate_pt(offset_x + c2x * scale, offset_y + c2y * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.bezier_curve_to(r1x, r1y, r2x, r2y, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'S' => {
                if let Some((c2x, c2y, x, y)) = parse_four_numbers(&mut chars) {
                    let (c1x, c1y) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let (r1x, r1y) = rotate_pt(offset_x + c1x * scale, offset_y + c1y * scale, cx, cy, sin_a, cos_a);
                    let (r2x, r2y) = rotate_pt(offset_x + c2x * scale, offset_y + c2y * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.bezier_curve_to(r1x, r1y, r2x, r2y, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            's' => {
                if let Some((dc2x, dc2y, dx, dy)) = parse_four_numbers(&mut chars) {
                    let (c1x, c1y) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    let (r1x, r1y) = rotate_pt(offset_x + c1x * scale, offset_y + c1y * scale, cx, cy, sin_a, cos_a);
                    let (r2x, r2y) = rotate_pt(offset_x + c2x * scale, offset_y + c2y * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.bezier_curve_to(r1x, r1y, r2x, r2y, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'Q' => {
                if let Some((qcx, qcy, x, y)) = parse_four_numbers(&mut chars) {
                    let (rcx, rcy) = rotate_pt(offset_x + qcx * scale, offset_y + qcy * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.quadratic_curve_to(rcx, rcy, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((qcx, qcy));
                }
            }
            'q' => {
                if let Some((dcx, dcy, dx, dy)) = parse_four_numbers(&mut chars) {
                    let qcx = current_x + dcx;
                    let qcy = current_y + dcy;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    let (rcx, rcy) = rotate_pt(offset_x + qcx * scale, offset_y + qcy * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.quadratic_curve_to(rcx, rcy, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((qcx, qcy));
                }
            }
            'T' => {
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    let (qcx, qcy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let (rcx, rcy) = rotate_pt(offset_x + qcx * scale, offset_y + qcy * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.quadratic_curve_to(rcx, rcy, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((qcx, qcy));
                }
            }
            't' => {
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    let (qcx, qcy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let x = current_x + dx;
                    let y = current_y + dy;
                    let (rcx, rcy) = rotate_pt(offset_x + qcx * scale, offset_y + qcy * scale, cx, cy, sin_a, cos_a);
                    let (rx, ry) = rotate_pt(offset_x + x * scale, offset_y + y * scale, cx, cy, sin_a, cos_a);
                    ctx.quadratic_curve_to(rcx, rcy, rx, ry);
                    current_x = x;
                    current_y = y;
                    last_control = Some((qcx, qcy));
                }
            }
            'A' | 'a' => {
                let is_relative = current_cmd == 'a';
                if let Some((rx, ry, rotation, large, sweep, x, y)) = parse_arc_params(&mut chars) {
                    let (end_x, end_y) = if is_relative {
                        (current_x + x, current_y + y)
                    } else {
                        (x, y)
                    };

                    let arc_points = arc_to_points(
                        current_x, current_y,
                        rx, ry,
                        rotation,
                        large != 0.0,
                        sweep != 0.0,
                        end_x, end_y,
                    );

                    for (px, py) in arc_points {
                        let (rpx, rpy) = rotate_pt(offset_x + px * scale, offset_y + py * scale, cx, cy, sin_a, cos_a);
                        ctx.line_to(rpx, rpy);
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

/// Draw an SVG icon with rotation around its center using pure trigonometric rotation.
///
/// This implementation uses direct coordinate rotation via sin/cos math instead of
/// ctx.save/translate/rotate/restore to avoid transform composition issues.
///
/// # Arguments
/// * `ctx` - Render context
/// * `svg` - SVG string content
/// * `x`, `y` - Top-left corner position
/// * `width`, `height` - Target dimensions
/// * `color` - Stroke color (hex string)
/// * `angle` - Rotation angle in radians
#[allow(clippy::too_many_arguments)]
pub fn draw_svg_icon_rotated(
    ctx: &mut dyn RenderContext, svg: &str,
    x: f64, y: f64, width: f64, height: f64,
    color: &str, angle: f64,
) {
    let (vb_width, vb_height) = parse_viewbox(svg).unwrap_or((24.0, 24.0));
    let scale_x = width / vb_width;
    let scale_y = height / vb_height;
    let scale = scale_x.min(scale_y);

    let offset_x = (x + (width - vb_width * scale) / 2.0).floor();
    let offset_y = (y + (height - vb_height * scale) / 2.0).floor();

    // Center of the icon in screen space
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;

    // Precompute sin/cos for rotation
    let sin_a = angle.sin();
    let cos_a = angle.cos();

    let has_fill_none = svg_root_has_fill_none(svg);
    let default_filled = !has_fill_none;
    let stroke_width = 1.5 * scale;

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(stroke_width);
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.set_line_dash(&[]);

    // Paths - use rotated renderer
    for path_info in parse_svg_paths(svg, default_filled) {
        ctx.begin_path();
        render_path_data_rotated(ctx, &path_info.d, offset_x, offset_y, scale, cx, cy, sin_a, cos_a);
        if path_info.filled {
            ctx.set_fill_color(color);
            ctx.fill();
        }
        if path_info.stroked {
            if let Some(ref dash) = path_info.dash_array {
                let scaled_dash: Vec<f64> = dash.iter().map(|d| d * scale).collect();
                ctx.set_line_dash(&scaled_dash);
            }
            ctx.stroke();
            if path_info.dash_array.is_some() {
                ctx.set_line_dash(&[]);
            }
        }
    }

    // Circles - rotate center point
    for (ccx, ccy, r, filled) in parse_svg_circles(svg, default_filled) {
        let tx = offset_x + ccx * scale;
        let ty = offset_y + ccy * scale;
        let (rtx, rty) = rotate_pt(tx, ty, cx, cy, sin_a, cos_a);
        let tr = r * scale;
        ctx.begin_path();
        ctx.arc(rtx, rty, tr, 0.0, std::f64::consts::PI * 2.0);
        if filled {
            ctx.set_fill_color(color);
            ctx.fill();
        } else {
            ctx.stroke();
        }
    }

    // Rects - convert to 4 rotated corner points
    for (rect_x, rect_y, rw, rh, _rounding, filled) in parse_svg_rects(svg, default_filled) {
        let tx = offset_x + rect_x * scale;
        let ty = offset_y + rect_y * scale;
        let tw = rw * scale;
        let th = rh * scale;

        // Define 4 corners
        let corners = [
            (tx, ty),
            (tx + tw, ty),
            (tx + tw, ty + th),
            (tx, ty + th),
        ];
        let rotated: Vec<(f64, f64)> = corners.iter()
            .map(|&(px, py)| rotate_pt(px, py, cx, cy, sin_a, cos_a))
            .collect();

        ctx.begin_path();
        ctx.move_to(rotated[0].0, rotated[0].1);
        for &(rx, ry) in &rotated[1..] {
            ctx.line_to(rx, ry);
        }
        ctx.close_path();

        if filled {
            ctx.set_fill_color(color);
            ctx.fill();
        } else {
            ctx.stroke();
        }
    }

    // Lines - rotate both endpoints
    for (x1, y1, x2, y2) in parse_svg_lines(svg) {
        let tx1 = offset_x + x1 * scale;
        let ty1 = offset_y + y1 * scale;
        let tx2 = offset_x + x2 * scale;
        let ty2 = offset_y + y2 * scale;
        let (rtx1, rty1) = rotate_pt(tx1, ty1, cx, cy, sin_a, cos_a);
        let (rtx2, rty2) = rotate_pt(tx2, ty2, cx, cy, sin_a, cos_a);
        ctx.begin_path();
        ctx.move_to(rtx1, rty1);
        ctx.line_to(rtx2, rty2);
        ctx.stroke();
    }

    // Polylines - rotate each point
    for (points, closed) in parse_svg_polylines(svg) {
        if points.len() >= 2 {
            ctx.begin_path();
            let (px, py) = points[0];
            let tx = offset_x + px * scale;
            let ty = offset_y + py * scale;
            let (rtx, rty) = rotate_pt(tx, ty, cx, cy, sin_a, cos_a);
            ctx.move_to(rtx, rty);
            for &(px, py) in &points[1..] {
                let tx = offset_x + px * scale;
                let ty = offset_y + py * scale;
                let (rtx, rty) = rotate_pt(tx, ty, cx, cy, sin_a, cos_a);
                ctx.line_to(rtx, rty);
            }
            if closed {
                ctx.close_path();
            }
            ctx.stroke();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_viewbox_64x64() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,1 2,1z"/></svg>"#;
        let (w, h) = parse_viewbox(svg).unwrap();
        assert_eq!(w, 64.0);
        assert_eq!(h, 64.0);
    }

    #[test]
    fn test_parse_viewbox_24x24() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"><path d="M1 1L23 23"/></svg>"#;
        let (w, h) = parse_viewbox(svg).unwrap();
        assert_eq!(w, 24.0);
        assert_eq!(h, 24.0);
    }

    #[test]
    fn test_svg_root_fill_none() {
        let stroke_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor"><path d="M1 1"/></svg>"#;
        assert!(svg_root_has_fill_none(stroke_svg));

        let fill_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,1z"/></svg>"#;
        assert!(!svg_root_has_fill_none(fill_svg));
    }

    #[test]
    fn test_parse_paths_fill_based() {
        // FlightAware-style SVG: no fill="none" on root, no stroke attributes
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,0 32,64 -64,0z"/></svg>"#;
        let paths = parse_svg_paths(svg, true); // default_filled = true (no fill="none" on root)
        assert_eq!(paths.len(), 1);
        assert!(paths[0].filled, "Fill-based SVG path should be filled");
        assert!(!paths[0].stroked, "Fill-based SVG path should NOT be stroked");
        assert_eq!(paths[0].d, "m 32,0 32,64 -64,0z");
    }

    #[test]
    fn test_parse_paths_stroke_based() {
        // Lucide-style SVG: fill="none" on root, stroke="currentColor"
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor"><path d="M1 1L23 23" /></svg>"#;
        let paths = parse_svg_paths(svg, false); // default_filled = false (fill="none" on root)
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].filled, "Stroke-based SVG path should NOT be filled");
        assert!(paths[0].stroked, "Stroke-based SVG path should be stroked");
    }

    #[test]
    fn test_jet_svg_parses() {
        // The actual JET SVG from aviation.rs
        let jet_svg = crate::render::icons::aviation::JET;
        let has_fill_none = svg_root_has_fill_none(jet_svg);
        assert!(!has_fill_none, "JET SVG should NOT have fill=none on root");

        let paths = parse_svg_paths(jet_svg, !has_fill_none);
        assert_eq!(paths.len(), 1, "JET SVG should have exactly one path");
        assert!(paths[0].filled, "JET path should be filled");
        assert!(!paths[0].d.is_empty(), "JET path data should not be empty");
    }

    #[test]
    fn test_military_jet_svg_parses() {
        // Simple triangle SVG
        let mil_svg = crate::render::icons::aviation::MILITARY_JET;
        let paths = parse_svg_paths(mil_svg, true);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].filled);
        assert_eq!(paths[0].d, "m 32,0 32,64 -64,0z");
    }

    #[test]
    fn test_maki_airport_with_xml_entities() {
        // Maki AIRPORT has &#xA; and &#x9; (newline/tab entities) in path data
        let airport_svg = crate::render::icons::infrastructure::AIRPORT;
        let (w, h) = parse_viewbox(airport_svg).unwrap();
        assert_eq!(w, 15.0);
        assert_eq!(h, 15.0);

        let has_fill_none = svg_root_has_fill_none(airport_svg);
        assert!(!has_fill_none, "AIRPORT should not have fill=none");

        let paths = parse_svg_paths(airport_svg, true);
        assert_eq!(paths.len(), 1, "AIRPORT should have one path");
        // Check that the path data contains the XML entities (they won't be decoded)
        println!("AIRPORT path d: {:?}", &paths[0].d);
    }

    #[test]
    fn test_parse_number_basic() {
        let input = "32,1 2,3";
        let mut chars = input.chars().peekable();
        assert_eq!(parse_number(&mut chars), Some(32.0));
        assert_eq!(parse_number(&mut chars), Some(1.0));
        assert_eq!(parse_number(&mut chars), Some(2.0));
        assert_eq!(parse_number(&mut chars), Some(3.0));
    }

    #[test]
    fn test_parse_number_negative() {
        let input = "-15,-2 -9,0";
        let mut chars = input.chars().peekable();
        assert_eq!(parse_number(&mut chars), Some(-15.0));
        assert_eq!(parse_number(&mut chars), Some(-2.0));
        assert_eq!(parse_number(&mut chars), Some(-9.0));
        assert_eq!(parse_number(&mut chars), Some(0.0));
    }

    #[test]
    fn test_parse_number_decimal_no_leading_zero() {
        let input = ".2761 -.5";
        let mut chars = input.chars().peekable();
        assert_eq!(parse_number(&mut chars), Some(0.2761));
        assert_eq!(parse_number(&mut chars), Some(-0.5));
    }

    #[test]
    fn test_parse_number_with_xml_entities() {
        // XML entities &#xA; (newline) and &#x9; (tab) should be skipped like whitespace
        // Test case: "6.5-1" followed by entities, then "-0.3182"
        let input = "6.5-1&#xA;&#x9;l-0.3182,4.7727";
        let mut chars = input.chars().peekable();
        assert_eq!(parse_number(&mut chars), Some(6.5), "Should parse 6.5");
        assert_eq!(parse_number(&mut chars), Some(-1.0), "Should parse -1.0");
        // Now at '&#xA;&#x9;l-0.3182...'
        // Skip 'l' command manually (not a number)
        assert_eq!(chars.next(), Some('&'), "Should be at first entity");
        // Try to parse again - should skip entities and find nothing (hits 'l')
        assert_eq!(parse_number(&mut chars), None, "Should return None when encountering 'l' after entities");

        // Better test: entities BEFORE a number
        let input2 = "15,8.5&#xA;&#x9;l-6.5-1";
        let mut chars2 = input2.chars().peekable();
        assert_eq!(parse_number(&mut chars2), Some(15.0), "Should parse 15");
        assert_eq!(parse_number(&mut chars2), Some(8.5), "Should parse 8.5");
        assert_eq!(chars2.peek(), Some(&'&'), "Should be at entity after 8.5");
        // Skip entities manually (or they're consumed by the NEXT parse_number call)
        // In the real path parser, the while loop skips entities in the outer loop
        // Let's test the actual use case: entities between numbers in a path command
        let input3 = "l-6.5&#xA;&#x9;-1";
        let mut chars3 = input3.chars().peekable();
        assert_eq!(chars3.next(), Some('l'), "Skip command");
        assert_eq!(parse_number(&mut chars3), Some(-6.5), "Should parse -6.5");
        // Now at entity - next parse_number() should skip it
        assert_eq!(parse_number(&mut chars3), Some(-1.0), "Should skip entities and parse -1.0");
    }

    // =============================================================================
    // Mock RenderContext for Unit Testing
    // =============================================================================

    /// Mock RenderContext that tracks path operations
    struct MockContext {
        ops: Vec<String>,
    }

    impl MockContext {
        fn new() -> Self {
            Self { ops: Vec::new() }
        }
    }

    impl crate::render::context::RenderContext for MockContext {
        fn dpr(&self) -> f64 { 1.0 }

        fn begin_path(&mut self) { self.ops.push("begin_path".to_string()); }
        fn move_to(&mut self, x: f64, y: f64) { self.ops.push(format!("move_to({:.1},{:.1})", x, y)); }
        fn line_to(&mut self, x: f64, y: f64) { self.ops.push(format!("line_to({:.1},{:.1})", x, y)); }
        fn close_path(&mut self) { self.ops.push("close_path".to_string()); }
        fn fill(&mut self) { self.ops.push("fill".to_string()); }
        fn stroke(&mut self) { self.ops.push("stroke".to_string()); }

        fn set_fill_color(&mut self, _color: &str) { self.ops.push("set_fill_color".to_string()); }
        fn set_stroke_color(&mut self, _color: &str) { self.ops.push("set_stroke_color".to_string()); }
        fn set_stroke_width(&mut self, _w: f64) { self.ops.push("set_stroke_width".to_string()); }
        fn set_line_cap(&mut self, _cap: &str) { self.ops.push("set_line_cap".to_string()); }
        fn set_line_join(&mut self, _join: &str) { self.ops.push("set_line_join".to_string()); }
        fn set_line_dash(&mut self, _pattern: &[f64]) { self.ops.push("set_line_dash".to_string()); }
        fn set_global_alpha(&mut self, _alpha: f64) { self.ops.push("set_global_alpha".to_string()); }

        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("rect".to_string()); }
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("fill_rect".to_string()); }
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("stroke_rect".to_string()); }
        fn fill_rounded_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64, _r: f64) { self.ops.push("fill_rounded_rect".to_string()); }
        fn stroke_rounded_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64, _r: f64) { self.ops.push("stroke_rounded_rect".to_string()); }

        fn arc(&mut self, _x: f64, _y: f64, _r: f64, _start: f64, _end: f64) { self.ops.push("arc".to_string()); }
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _start: f64, _end: f64) { self.ops.push("ellipse".to_string()); }
        fn bezier_curve_to(&mut self, _c1x: f64, _c1y: f64, _c2x: f64, _c2y: f64, _x: f64, _y: f64) { self.ops.push("bezier_curve_to".to_string()); }
        fn quadratic_curve_to(&mut self, _cx: f64, _cy: f64, _x: f64, _y: f64) { self.ops.push("quadratic_curve_to".to_string()); }
        fn clip(&mut self) { self.ops.push("clip".to_string()); }

        fn set_font(&mut self, _font: &str) { self.ops.push("set_font".to_string()); }
        fn set_text_align(&mut self, _align: crate::render::types::TextAlign) { self.ops.push("set_text_align".to_string()); }
        fn set_text_baseline(&mut self, _baseline: crate::render::types::TextBaseline) { self.ops.push("set_text_baseline".to_string()); }
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) { self.ops.push("fill_text".to_string()); }
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) { self.ops.push("stroke_text".to_string()); }
        fn measure_text(&self, _text: &str) -> f64 { 0.0 }

        fn save(&mut self) { self.ops.push("save".to_string()); }
        fn restore(&mut self) { self.ops.push("restore".to_string()); }
        fn translate(&mut self, _x: f64, _y: f64) { self.ops.push("translate".to_string()); }
        fn rotate(&mut self, _angle: f64) { self.ops.push("rotate".to_string()); }
        fn scale(&mut self, _x: f64, _y: f64) { self.ops.push("scale".to_string()); }
    }

    #[test]
    fn test_draw_jet_icon_produces_fill_ops() {
        let mut ctx = MockContext::new();
        let jet_svg = crate::render::icons::aviation::JET;

        // Draw at (100, 100) with size 22
        draw_svg_icon(&mut ctx, jet_svg, 100.0, 100.0, 22.0, 22.0, "#4fc3f7");

        // Should have begin_path, many move_to/line_to, close_path, and fill
        println!("JET ops count: {}", ctx.ops.len());
        for (i, op) in ctx.ops.iter().enumerate() {
            println!("  [{:3}] {}", i, op);
        }

        assert!(ctx.ops.contains(&"begin_path".to_string()), "Should call begin_path");
        assert!(ctx.ops.iter().any(|op| op.starts_with("move_to")), "Should call move_to");
        assert!(ctx.ops.iter().any(|op| op.starts_with("line_to")), "Should call line_to");
        assert!(ctx.ops.contains(&"close_path".to_string()), "Should call close_path");
        assert!(ctx.ops.contains(&"set_fill_color".to_string()), "Should call set_fill_color");
        assert!(ctx.ops.contains(&"fill".to_string()), "Should call fill");

        // Should NOT call stroke (it's a fill-based icon)
        assert!(!ctx.ops.contains(&"stroke".to_string()), "Should NOT call stroke for fill-based icon");

        // Count line_to operations - should be many (30+ for the full JET path)
        let line_count = ctx.ops.iter().filter(|op| op.starts_with("line_to")).count();
        println!("JET line_to count: {}", line_count);
        assert!(line_count > 20, "JET path should have 30+ line segments, got {}", line_count);
    }

    #[test]
    fn test_draw_cloud_icon_produces_stroke_ops() {
        let mut ctx = MockContext::new();
        let cloud_svg = crate::render::icons::weather::CLOUD;

        draw_svg_icon(&mut ctx, cloud_svg, 100.0, 100.0, 22.0, 22.0, "#ffa726");

        println!("CLOUD ops count: {}", ctx.ops.len());
        for (i, op) in ctx.ops.iter().enumerate() {
            println!("  [{:3}] {}", i, op);
        }

        assert!(ctx.ops.contains(&"begin_path".to_string()), "Should call begin_path");
        assert!(ctx.ops.contains(&"stroke".to_string()), "Should call stroke");
        assert!(!ctx.ops.contains(&"fill".to_string()), "Should NOT call fill for stroke-based icon");
    }

    #[test]
    fn test_draw_military_jet_simple_triangle() {
        let mut ctx = MockContext::new();
        let mil_svg = crate::render::icons::aviation::MILITARY_JET;

        draw_svg_icon(&mut ctx, mil_svg, 100.0, 100.0, 22.0, 22.0, "#ff0000");

        println!("MILITARY_JET ops:");
        for (i, op) in ctx.ops.iter().enumerate() {
            println!("  [{:3}] {}", i, op);
        }

        assert!(ctx.ops.contains(&"fill".to_string()), "Triangle should be filled");
        // Should have exactly: begin_path, move_to, line_to, line_to, close_path, set_fill_color, fill
        let line_count = ctx.ops.iter().filter(|op| op.starts_with("line_to")).count();
        assert_eq!(line_count, 2, "Triangle should have 2 line_to ops (3 points: move + 2 lines + close)");
    }

    #[test]
    fn test_draw_airport_icon_with_xml_entities() {
        // Test that AIRPORT icon (with &#xA; and &#x9; entities) renders without infinite loop
        let mut ctx = MockContext::new();
        let airport_svg = crate::render::icons::infrastructure::AIRPORT;

        // This should complete instantly, not hang
        draw_svg_icon(&mut ctx, airport_svg, 100.0, 100.0, 16.0, 16.0, "#2196f3");

        println!("AIRPORT ops count: {}", ctx.ops.len());
        println!("First 10 ops:");
        for (i, op) in ctx.ops.iter().take(10).enumerate() {
            println!("  [{:3}] {}", i, op);
        }

        // Should have path operations (not empty)
        assert!(ctx.ops.contains(&"begin_path".to_string()), "Should call begin_path");
        assert!(ctx.ops.iter().any(|op| op.starts_with("move_to")), "Should call move_to");
        assert!(ctx.ops.iter().any(|op| op.starts_with("line_to")), "Should call line_to");
        assert!(ctx.ops.contains(&"fill".to_string()), "AIRPORT should be filled");

        // Check that we have a reasonable number of path operations (not infinite)
        let line_count = ctx.ops.iter().filter(|op| op.starts_with("line_to")).count();
        println!("AIRPORT line_to count: {}", line_count);
        assert!(line_count > 5, "AIRPORT should have several line segments");
        assert!(line_count < 100, "AIRPORT should not have excessive line segments (would indicate parsing issue)");
    }

    #[test]
    fn test_all_maki_icons_with_xml_entities() {
        // Test all 4 Maki icons that contain XML entities (&#xA; &#x9;)
        // These should all render without hanging
        let icons = vec![
            ("AIRPORT", crate::render::icons::infrastructure::AIRPORT),
            ("HELIPORT", crate::render::icons::infrastructure::HELIPORT),
            ("FUEL", crate::render::icons::infrastructure::FUEL),
            ("HOSPITAL", crate::render::icons::infrastructure::HOSPITAL),
        ];

        for (name, svg) in icons {
            println!("Testing {} icon...", name);
            let mut ctx = MockContext::new();
            draw_svg_icon(&mut ctx, svg, 100.0, 100.0, 16.0, 16.0, "#2196f3");

            let line_count = ctx.ops.iter().filter(|op| op.starts_with("line_to")).count();
            println!("  {} line_to ops: {}", name, line_count);

            assert!(ctx.ops.contains(&"begin_path".to_string()), "{} should call begin_path", name);
            assert!(line_count > 0, "{} should have line segments", name);
            assert!(line_count < 200, "{} should not have excessive line segments", name);
        }
        println!("All Maki icons with XML entities render successfully!");
    }
}
