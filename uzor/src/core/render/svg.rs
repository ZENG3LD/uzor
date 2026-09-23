use super::context::RenderContext;
use super::parse_color;

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

    // Ancestor opacity from the root <svg>/<g>, multiplied onto every element's own
    // opacity/fill-opacity/stroke-opacity below (SVG's cascading opacity model).
    let root_opacity = parse_root_opacity(svg);

    // Device pixel ratio — lets the pixel-grid snapping below land on actual device
    // pixels rather than logical (CSS) ones. On a backend that reports `dpr() == 1.0`
    // (no HiDPI info) this collapses to snapping in the context's own coordinate space.
    let dpr = ctx.dpr();

    // Base pen weight: the root `stroke-width` (default 1.5 — this crate's historical
    // Lucide-icon weight — when the root doesn't declare one), scaled and quantized to
    // whole device pixels so the stroke doesn't straddle a pixel row/column and blur.
    let stroke_width = quantize_stroke_width(parse_root_stroke_width(svg) * scale, dpr);

    // Set stroke style
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(stroke_width);
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.set_line_dash(&[]);

    let gt = parse_g_transform(svg);
    let eff_offset_x = offset_x + gt.tx * scale;
    let eff_offset_y = offset_y + gt.ty * scale;
    let eff_scale_x = gt.sx * scale;
    let eff_scale_y = gt.sy * scale;

    // Parse and render all path elements. Fill and stroke are built as SEPARATE path
    // geometries (rather than one shared begin_path/render reused for both) so a path
    // that is both filled and stroked can keep its fill exact while snapping only the
    // axis-aligned (H/V) portions of its stroke — "fills are not snapped".
    for path_info in parse_svg_paths(svg, default_filled) {
        if path_info.filled {
            ctx.begin_path();
            render_path_data(ctx, &path_info.d, eff_offset_x, eff_offset_y, eff_scale_x, eff_scale_y, None);
            let fill_alpha = path_info.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            ctx.fill();
        }
        if path_info.stroked {
            // Per-element stroke-width override (e.g. thin construction lines).
            let override_width = path_info.stroke_width.map(|w| scaled_stroke_width_override(w, scale, dpr));
            if let Some(w) = override_width {
                ctx.set_stroke_width(w);
            }
            let effective_width = override_width.unwrap_or(stroke_width);

            ctx.begin_path();
            render_path_data(
                ctx, &path_info.d, eff_offset_x, eff_offset_y, eff_scale_x, eff_scale_y,
                Some(AxisSnap { width: effective_width, dpr }),
            );

            // Per-element stroke opacity — only touch stroke color when it actually
            // differs from the up-front default, and restore it afterward so nothing
            // leaks into the next (unmodified) stroked element.
            let stroke_alpha = (path_info.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &path_info.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
            if override_width.is_some() {
                ctx.set_stroke_width(stroke_width);
            }
        }
    }

    // Parse and render all circle elements. Circles are curved, not axis-aligned, so
    // per item 2's scope (h/v lines, rect edges) their geometry is never snapped.
    for (cx, cy, r, filled, style) in parse_svg_circles(svg, default_filled) {
        let tx = eff_offset_x + cx * eff_scale_x;
        let ty = eff_offset_y + cy * eff_scale_y;
        let tr = r * scale;

        ctx.begin_path();
        draw_circle_bezier(ctx, tx, ty, tr);
        if filled {
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            ctx.fill();
        } else if tr < 3.0 {
            // Too small for stroke to be visible — fill it as a dot. It stands in for
            // the stroke visually, so it takes the stroke's opacity, not the fill's.
            let stroke_alpha = style.opacity.stroke * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, stroke_alpha));
            ctx.fill();
        } else {
            let override_width = style.stroke_width.map(|w| scaled_stroke_width_override(w, scale, dpr));
            if let Some(w) = override_width {
                ctx.set_stroke_width(w);
            }
            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
            if override_width.is_some() {
                ctx.set_stroke_width(stroke_width);
            }
        }
    }

    // Parse and render all rect elements. Rect edges are axis-aligned, so a stroked
    // rect's corner is snapped (both x and y — a corner is where one h and one v edge
    // meet); a filled rect's corner is left exact ("fills are not snapped").
    for (rx, ry, rw, rh, rounding, filled, style) in parse_svg_rects(svg, default_filled) {
        let raw_tx = eff_offset_x + rx * eff_scale_x;
        let raw_ty = eff_offset_y + ry * eff_scale_y;
        let tw = rw * eff_scale_x;
        let th = rh * eff_scale_y;
        let tr = rounding * scale;

        if filled {
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            if tr > 0.0 {
                ctx.fill_rounded_rect(raw_tx, raw_ty, tw, th, tr);
            } else {
                ctx.fill_rect(raw_tx, raw_ty, tw, th);
            }
        } else {
            let override_width = style.stroke_width.map(|w| scaled_stroke_width_override(w, scale, dpr));
            if let Some(w) = override_width {
                ctx.set_stroke_width(w);
            }
            let effective_width = override_width.unwrap_or(stroke_width);
            let tx = snap_axis(raw_tx, effective_width, dpr);
            let ty = snap_axis(raw_ty, effective_width, dpr);

            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| {
                if tr > 0.0 {
                    ctx.stroke_rounded_rect(tx, ty, tw, th, tr);
                } else {
                    ctx.stroke_rect(tx, ty, tw, th);
                }
            });
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
            if override_width.is_some() {
                ctx.set_stroke_width(stroke_width);
            }
        }
    }

    // Parse and render all line elements. Axis-aligned lines (x1 == x2, a vertical run;
    // y1 == y2, a horizontal run) get their constant coordinate snapped; diagonal lines
    // keep their exact geometry.
    for (x1, y1, x2, y2, style) in parse_svg_lines(svg) {
        let override_width = style.stroke_width.map(|w| scaled_stroke_width_override(w, scale, dpr));
        let effective_width = override_width.unwrap_or(stroke_width);

        let mut tx1 = eff_offset_x + x1 * eff_scale_x;
        let mut ty1 = eff_offset_y + y1 * eff_scale_y;
        let mut tx2 = eff_offset_x + x2 * eff_scale_x;
        let mut ty2 = eff_offset_y + y2 * eff_scale_y;
        if x1 == x2 {
            // Vertical run — snap the shared X so it doesn't straddle a pixel column.
            let sx = snap_axis(tx1, effective_width, dpr);
            tx1 = sx;
            tx2 = sx;
        } else if y1 == y2 {
            // Horizontal run — snap the shared Y so it doesn't straddle a pixel row.
            let sy = snap_axis(ty1, effective_width, dpr);
            ty1 = sy;
            ty2 = sy;
        }

        ctx.begin_path();
        ctx.move_to(tx1, ty1);
        ctx.line_to(tx2, ty2);
        if let Some(w) = override_width {
            ctx.set_stroke_width(w);
        }
        let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
        if stroke_alpha < 1.0 {
            ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
        }
        stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
        if stroke_alpha < 1.0 {
            ctx.set_stroke_color(color);
        }
        if override_width.is_some() {
            ctx.set_stroke_width(stroke_width);
        }
    }

    // Parse and render all polyline elements
    for (points, closed, style) in parse_svg_polylines(svg) {
        if points.len() >= 2 {
            ctx.begin_path();
            let (px, py) = points[0];
            ctx.move_to(eff_offset_x + px * eff_scale_x, eff_offset_y + py * eff_scale_y);
            for &(px, py) in &points[1..] {
                ctx.line_to(eff_offset_x + px * eff_scale_x, eff_offset_y + py * eff_scale_y);
            }
            if closed {
                ctx.close_path();
            }
            let override_width = style.stroke_width.map(|w| scaled_stroke_width_override(w, scale, dpr));
            if let Some(w) = override_width {
                ctx.set_stroke_width(w);
            }
            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
            if override_width.is_some() {
                ctx.set_stroke_width(stroke_width);
            }
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

// =============================================================================
// Opacity & Dash Parsing (shared by path/circle/rect/line/polyline/polygon)
// =============================================================================

/// Find the value of a quoted XML/SVG attribute, requiring a non-identifier character
/// (or start of string) immediately before the match.
///
/// Without this guard, a plain `content.find("opacity=\"")` would also match inside
/// `fill-opacity="..."` or `stroke-opacity="..."` — `"fill-opacity=\""` literally ends
/// with the substring `"opacity=\""` — misreading a channel-specific attribute as the
/// generic `opacity` one.
fn find_attr_value<'a>(content: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{attr}=\"");
    let mut search_from = 0usize;
    while let Some(rel) = content[search_from..].find(pattern.as_str()) {
        let abs = search_from + rel;
        let is_boundary = abs == 0
            || !matches!(
                content.as_bytes()[abs - 1],
                b'-' | b'_' | b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            );
        if is_boundary {
            let value_start = abs + pattern.len();
            return content[value_start..].find('"').map(|end| &content[value_start..value_start + end]);
        }
        search_from = abs + pattern.len();
    }
    None
}

/// Parse an SVG opacity value: a bare number (SVG allows any float; we clamp to the
/// valid `0.0..=1.0` range) or a percentage like `"50%"`. Returns `None` when the value
/// can't be parsed, so callers fall back to fully opaque.
fn parse_opacity_value(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if let Some(pct) = raw.strip_suffix('%') {
        return pct.trim().parse::<f64>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    raw.parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0))
}

/// Multiplicative opacity for an element's fill and stroke, combining the generic
/// `opacity` attribute with the channel-specific `fill-opacity`/`stroke-opacity`.
/// Defaults to fully opaque (`1.0`) when none of the three are present.
#[derive(Clone, Copy)]
struct ElementOpacity {
    fill: f64,
    stroke: f64,
}

impl ElementOpacity {
    fn parse(content: &str) -> Self {
        let base = find_attr_value(content, "opacity").and_then(parse_opacity_value).unwrap_or(1.0);
        let fill_opacity = find_attr_value(content, "fill-opacity").and_then(parse_opacity_value).unwrap_or(1.0);
        let stroke_opacity = find_attr_value(content, "stroke-opacity").and_then(parse_opacity_value).unwrap_or(1.0);
        Self { fill: base * fill_opacity, stroke: base * stroke_opacity }
    }
}

/// Parse `stroke-dasharray="4 2"` or `"4,2"` into per-segment dash lengths.
/// Returns `None` when absent or empty (solid stroke).
fn parse_dash_array(content: &str) -> Option<Vec<f64>> {
    let raw = find_attr_value(content, "stroke-dasharray")?;
    let values: Vec<f64> = raw.split([' ', ',']).filter_map(|s| s.trim().parse::<f64>().ok()).collect();
    if values.is_empty() { None } else { Some(values) }
}

/// Ancestor opacity carried by the root `<svg>` and the first `<g>` (if any), multiplied
/// together per SVG's cascading opacity model. Applied on top of each element's own
/// `opacity`/`fill-opacity`/`stroke-opacity` at the point of use.
fn parse_root_opacity(svg: &str) -> f64 {
    let mut factor = 1.0;
    if let Some(start) = svg.find("<svg") {
        if let Some(end) = svg[start..].find('>') {
            let tag = &svg[start..start + end + 1];
            factor *= find_attr_value(tag, "opacity").and_then(parse_opacity_value).unwrap_or(1.0);
        }
    }
    if let Some(start) = svg.find("<g ") {
        if let Some(end) = svg[start..].find('>') {
            let tag = &svg[start..start + end + 1];
            factor *= find_attr_value(tag, "opacity").and_then(parse_opacity_value).unwrap_or(1.0);
        }
    }
    factor
}

/// Style attributes shared by circle/rect/line/polyline/polygon: an optional per-element
/// `stroke-width` override, an optional `stroke-dasharray`, and the element's opacity.
struct ElementStyle {
    stroke_width: Option<f64>,
    dash_array: Option<Vec<f64>>,
    opacity: ElementOpacity,
}

impl ElementStyle {
    fn parse(content: &str) -> Self {
        Self {
            stroke_width: extract_svg_attr(content, "stroke-width"),
            dash_array: parse_dash_array(content),
            opacity: ElementOpacity::parse(content),
        }
    }
}

/// Path rendering info
struct PathInfo {
    d: String,
    filled: bool,
    stroked: bool,
    dash_array: Option<Vec<f64>>,
    fill_color: Option<String>,    // Actual fill color from attribute (for multicolor SVGs)
    stroke_color: Option<String>,  // Actual stroke color from attribute (for multicolor SVGs)
    stroke_width: Option<f64>,     // Stroke width from attribute
    opacity: ElementOpacity,       // opacity / fill-opacity / stroke-opacity from attributes
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
                let (filled, fill_color) = if let Some(fill_start) = tag_content.find("fill=\"") {
                    let fill_content_start = fill_start + 6;
                    if let Some(fill_end) = tag_content[fill_content_start..].find('"') {
                        let fill_value = &tag_content[fill_content_start..fill_content_start + fill_end];
                        if fill_value != "none" {
                            (true, Some(fill_value.to_string()))
                        } else {
                            (false, None)
                        }
                    } else {
                        (false, None)
                    }
                } else {
                    (default_filled, None) // Use inherited default from root SVG
                };

                // Check stroke attribute (default is stroked for icons)
                let (stroked, stroke_color) = if let Some(stroke_start) = tag_content.find("stroke=\"") {
                    let stroke_content_start = stroke_start + 8;
                    if let Some(stroke_end) = tag_content[stroke_content_start..].find('"') {
                        let stroke_value = &tag_content[stroke_content_start..stroke_content_start + stroke_end];
                        if stroke_value != "none" {
                            (true, Some(stroke_value.to_string()))
                        } else {
                            (false, None)
                        }
                    } else {
                        (true, None)
                    }
                } else {
                    (!filled, None) // If not filled, assume stroked
                };

                // Check stroke-width attribute
                let stroke_width = if let Some(sw_start) = tag_content.find("stroke-width=\"") {
                    let sw_content_start = sw_start + 14;
                    if let Some(sw_end) = tag_content[sw_content_start..].find('"') {
                        tag_content[sw_content_start..sw_content_start + sw_end].parse::<f64>().ok()
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Check stroke-dasharray attribute (e.g., "4 2" for dashed lines)
                let dash_array = parse_dash_array(tag_content);

                // Check opacity / fill-opacity / stroke-opacity attributes
                let opacity = ElementOpacity::parse(tag_content);

                paths.push(PathInfo { d, filled, stroked, dash_array, fill_color, stroke_color, stroke_width, opacity });
            }
        }

        search_from = tag_end;
    }

    paths
}

/// Number of segments for arc approximation
const ARC_SEGMENTS: usize = 32;

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

/// Root `<svg>` `stroke-width` attribute — the base pen weight every element inherits
/// unless it declares its own override. Defaults to `1.5` (this crate's historical
/// Lucide-icon weight) when the root has no explicit `stroke-width`, so icons that
/// never specified one keep today's look.
fn parse_root_stroke_width(svg: &str) -> f64 {
    let Some(start) = svg.find("<svg") else { return 1.5 };
    let Some(end) = svg[start..].find('>') else { return 1.5 };
    let tag = &svg[start..start + end + 1];
    find_attr_value(tag, "stroke-width").and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(1.5)
}

/// Quantize a logical stroke width to whole device pixels for crisp rendering: widths
/// at or above 0.75 device px round to the nearest whole device pixel (never below 1,
/// so a hairline never rounds away to nothing); thinner widths are left at their exact
/// fractional value. `dpr` (`RenderContext::dpr`) converts between this context's
/// logical coordinate space and actual device pixels — on a backend that reports
/// `dpr() == 1.0` (no HiDPI scaling info) the two spaces coincide and this quantizes
/// directly in the context's own coordinate space.
#[inline]
fn quantize_stroke_width(logical_width: f64, dpr: f64) -> f64 {
    let device_width = logical_width * dpr;
    let device_width = if device_width >= 0.75 { device_width.round().max(1.0) } else { device_width };
    device_width / dpr
}

/// Per-axis pixel-grid snapping parameters for a stroked element's geometry — the
/// device-quantized stroke width (to decide odd/even centring) and the context's
/// device pixel ratio (to convert logical coordinates to the device grid and back).
#[derive(Clone, Copy)]
struct AxisSnap {
    width: f64,
    dpr: f64,
}

/// Snap a logical coordinate to the device-pixel grid appropriate for a stroke of the
/// given logical width: an odd device-pixel width centres the stroke on a half device
/// pixel (so e.g. a 1px axis-aligned line covers exactly one device pixel instead of
/// straddling two and reading soft); an even width centres on a whole device pixel.
/// Used ONLY for axis-aligned geometry — H/V path commands, `<line>` elements running
/// exactly horizontal/vertical, and stroked `<rect>` corners — diagonal and curved
/// geometry is left exact, matching real SVG rendering.
#[inline]
fn snap_axis(v: f64, stroke_width: f64, dpr: f64) -> f64 {
    let device_width = (stroke_width * dpr).round().max(1.0) as i64;
    let odd = device_width % 2 != 0;
    let device_v = v * dpr;
    let snapped_device = if odd { (device_v - 0.5).round() + 0.5 } else { device_v.round() };
    snapped_device / dpr
}

/// Scale a per-element `stroke-width` override into a device-pixel-quantized logical
/// width, using the same [`quantize_stroke_width`] rounding as the base width so an
/// override reads exactly like the base would at that same attribute value.
#[inline]
fn scaled_stroke_width_override(w: f64, scale: f64, dpr: f64) -> f64 {
    quantize_stroke_width(w * scale, dpr)
}

/// Compose an alpha factor (`0.0..=1.0`) onto a color string by scaling its existing alpha
/// channel, using uzor's shared CSS color parser (`crate::render::parse_color`) so hex,
/// `rgba()`, and named colors all compose identically — never hand-rolled.
///
/// Returns the input unchanged when `alpha` is fully opaque: a fast path that also keeps
/// output byte-identical to the pre-opacity behaviour for elements without any opacity
/// attribute (regression guard).
fn color_with_alpha(color: &str, alpha: f64) -> String {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha >= 1.0 {
        return color.to_string();
    }
    let (r, g, b, a) = parse_color(color);
    let composed = ((a as f64 / 255.0) * alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02x}{g:02x}{b:02x}{composed:02x}")
}

/// Run `paint` (a stroke call, or a shape helper like `stroke_rect` that strokes
/// internally) with a scaled `stroke-dasharray` pattern applied, resetting to solid
/// afterward. Shared by every `draw_svg_icon*` entry point and every element kind
/// (path/rect/circle/line/polyline/polygon) so dasharray handling lives in one place.
fn stroke_with_dash(ctx: &mut dyn RenderContext, dash_array: &Option<Vec<f64>>, scale: f64, paint: impl FnOnce(&mut dyn RenderContext)) {
    match dash_array {
        Some(dash) => {
            let scaled: Vec<f64> = dash.iter().map(|d| d * scale).collect();
            ctx.set_line_dash(&scaled);
            paint(ctx);
            ctx.set_line_dash(&[]);
        }
        None => {
            // Explicit solid reset (not just a no-op skip) so a dash pattern from
            // outside this loop — or from another element — never leaks in.
            ctx.set_line_dash(&[]);
            paint(ctx);
        }
    }
}

/// Draw a circle using 4 cubic Bézier curves (backend-agnostic).
/// Uses the standard kappa constant for near-perfect circular approximation.
fn draw_circle_bezier(ctx: &mut dyn RenderContext, cx: f64, cy: f64, r: f64) {
    const K: f64 = 0.5522847498; // 4/3 * (sqrt(2) - 1)
    let kr = K * r;

    // Start at rightmost point
    ctx.move_to(cx + r, cy);
    // Top-right quadrant
    ctx.bezier_curve_to(cx + r, cy - kr, cx + kr, cy - r, cx, cy - r);
    // Top-left quadrant
    ctx.bezier_curve_to(cx - kr, cy - r, cx - r, cy - kr, cx - r, cy);
    // Bottom-left quadrant
    ctx.bezier_curve_to(cx - r, cy + kr, cx - kr, cy + r, cx, cy + r);
    // Bottom-right quadrant
    ctx.bezier_curve_to(cx + kr, cy + r, cx + r, cy + kr, cx + r, cy);
    ctx.close_path();
}

/// Render SVG path data onto a RenderContext.
///
/// `snap` is `Some` only when rendering geometry for a stroke (fills always pass
/// `None` — "fills are not snapped"). Only the `H`/`V` commands consult it: each
/// snaps its OWN constant/perpendicular coordinate (a horizontal run's y, a
/// vertical run's x) to the pixel grid so the run doesn't straddle a pixel row or
/// column; every other command (`M`/`L`/curves/arcs) keeps its exact geometry,
/// since it may be diagonal or curved.
fn render_path_data(
    ctx: &mut dyn RenderContext, path_data: &str,
    offset_x: f64, offset_y: f64, scale_x: f64, scale_y: f64,
    snap: Option<AxisSnap>,
) {
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
                // Absolute move — exact geometry (may be the start of a diagonal run).
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    start_x = x;
                    start_y = y;
                    ctx.move_to(offset_x + x * scale_x, offset_y + y * scale_y);
                    current_cmd = 'L'; // Subsequent coordinates are line-to
                    last_control = None;
                }
            }
            'm' => {
                // Relative move — exact geometry.
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    start_x = current_x;
                    start_y = current_y;
                    ctx.move_to(offset_x + current_x * scale_x, offset_y + current_y * scale_y);
                    current_cmd = 'l'; // Subsequent coordinates are relative line-to
                    last_control = None;
                }
            }
            'L' => {
                // Absolute line — exact geometry (diagonal segments keep their shape).
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    current_x = x;
                    current_y = y;
                    ctx.line_to(offset_x + x * scale_x, offset_y + y * scale_y);
                    last_control = None;
                }
            }
            'l' => {
                // Relative line — exact geometry.
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    current_x += dx;
                    current_y += dy;
                    ctx.line_to(offset_x + current_x * scale_x, offset_y + current_y * scale_y);
                    last_control = None;
                }
            }
            'H' => {
                // Absolute horizontal line — axis-aligned: snap the CONSTANT y (the
                // perpendicular axis), leave x (along the run) exact.
                if let Some(x) = parse_number(&mut chars) {
                    current_x = x;
                    let py_raw = offset_y + current_y * scale_y;
                    let py = match snap {
                        Some(s) => snap_axis(py_raw, s.width, s.dpr),
                        None => py_raw,
                    };
                    ctx.line_to(offset_x + x * scale_x, py);
                    last_control = None;
                }
            }
            'h' => {
                // Relative horizontal line — same axis-aligned snapping as 'H'.
                if let Some(dx) = parse_number(&mut chars) {
                    current_x += dx;
                    let py_raw = offset_y + current_y * scale_y;
                    let py = match snap {
                        Some(s) => snap_axis(py_raw, s.width, s.dpr),
                        None => py_raw,
                    };
                    ctx.line_to(offset_x + current_x * scale_x, py);
                    last_control = None;
                }
            }
            'V' => {
                // Absolute vertical line — axis-aligned: snap the CONSTANT x, leave y
                // (along the run) exact.
                if let Some(y) = parse_number(&mut chars) {
                    current_y = y;
                    let px_raw = offset_x + current_x * scale_x;
                    let px = match snap {
                        Some(s) => snap_axis(px_raw, s.width, s.dpr),
                        None => px_raw,
                    };
                    ctx.line_to(px, offset_y + y * scale_y);
                    last_control = None;
                }
            }
            'v' => {
                // Relative vertical line — same axis-aligned snapping as 'V'.
                if let Some(dy) = parse_number(&mut chars) {
                    current_y += dy;
                    let px_raw = offset_x + current_x * scale_x;
                    let px = match snap {
                        Some(s) => snap_axis(px_raw, s.width, s.dpr),
                        None => px_raw,
                    };
                    ctx.line_to(px, offset_y + current_y * scale_y);
                    last_control = None;
                }
            }
            'C' => {
                // Absolute cubic bezier — curved, exact geometry.
                if let Some((c1x, c1y, c2x, c2y, x, y)) = parse_six_numbers(&mut chars) {
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale_x,
                        offset_y + c1y * scale_y,
                        offset_x + c2x * scale_x,
                        offset_y + c2y * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'c' => {
                // Relative cubic bezier — curved, exact geometry.
                if let Some((dc1x, dc1y, dc2x, dc2y, dx, dy)) = parse_six_numbers(&mut chars) {
                    let c1x = current_x + dc1x;
                    let c1y = current_y + dc1y;
                    let c2x = current_x + dc2x;
                    let c2y = current_y + dc2y;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale_x,
                        offset_y + c1y * scale_y,
                        offset_x + c2x * scale_x,
                        offset_y + c2y * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'S' => {
                // Smooth cubic bezier (absolute) — curved, exact geometry.
                if let Some((c2x, c2y, x, y)) = parse_four_numbers(&mut chars) {
                    // Reflect last control point
                    let (c1x, c1y) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    ctx.bezier_curve_to(
                        offset_x + c1x * scale_x,
                        offset_y + c1y * scale_y,
                        offset_x + c2x * scale_x,
                        offset_y + c2y * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            's' => {
                // Smooth cubic bezier (relative) — curved, exact geometry.
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
                        offset_x + c1x * scale_x,
                        offset_y + c1y * scale_y,
                        offset_x + c2x * scale_x,
                        offset_y + c2y * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((c2x, c2y));
                }
            }
            'Q' => {
                // Absolute quadratic bezier — curved, exact geometry.
                if let Some((cx, cy, x, y)) = parse_four_numbers(&mut chars) {
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale_x,
                        offset_y + cy * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'q' => {
                // Relative quadratic bezier — curved, exact geometry.
                if let Some((dcx, dcy, dx, dy)) = parse_four_numbers(&mut chars) {
                    let cx = current_x + dcx;
                    let cy = current_y + dcy;
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale_x,
                        offset_y + cy * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'T' => {
                // Smooth quadratic bezier (absolute) — curved, exact geometry.
                if let Some((x, y)) = parse_two_numbers(&mut chars) {
                    let (cx, cy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale_x,
                        offset_y + cy * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            't' => {
                // Smooth quadratic bezier (relative) — curved, exact geometry.
                if let Some((dx, dy)) = parse_two_numbers(&mut chars) {
                    let (cx, cy) = match last_control {
                        Some((lx, ly)) => (2.0 * current_x - lx, 2.0 * current_y - ly),
                        None => (current_x, current_y),
                    };
                    let x = current_x + dx;
                    let y = current_y + dy;
                    ctx.quadratic_curve_to(
                        offset_x + cx * scale_x,
                        offset_y + cy * scale_y,
                        offset_x + x * scale_x,
                        offset_y + y * scale_y,
                    );
                    current_x = x;
                    current_y = y;
                    last_control = Some((cx, cy));
                }
            }
            'A' | 'a' => {
                // Arc command: rx ry x-rotation large-arc-flag sweep-flag x y — curved,
                // exact geometry.
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
                        ctx.line_to(offset_x + px * scale_x, offset_y + py * scale_y);
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

    // Collect digits and decimal point. Per the SVG path grammar a number
    // ends at the SECOND '.', so the compact form "-.3.06" is TWO numbers
    // (-.3 and .06) — the old greedy collect produced "-.3.06", failed to
    // parse, and silently truncated the whole path (mangled icons).
    let mut seen_dot = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num_str.push(chars.next().unwrap());
        } else if c == '.' && !seen_dot {
            seen_dot = true;
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
/// Returns Vec of (cx, cy, r, filled, style)
/// `default_filled` is inherited from parent SVG element
fn parse_svg_circles(svg: &str, default_filled: bool) -> Vec<(f64, f64, f64, bool, ElementStyle)> {
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
            let style = ElementStyle::parse(tag_content);

            if r > 0.0 {
                circles.push((cx, cy, r, filled, style));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let cx = extract_svg_attr(tag_content, "cx").unwrap_or(0.0);
            let cy = extract_svg_attr(tag_content, "cy").unwrap_or(0.0);
            let r = extract_svg_attr(tag_content, "r").unwrap_or(0.0);
            let filled = is_svg_filled_with_default(tag_content, default_filled);
            let style = ElementStyle::parse(tag_content);

            if r > 0.0 {
                circles.push((cx, cy, r, filled, style));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    circles
}

/// Parse all <rect> elements from SVG
/// Returns Vec of (x, y, width, height, rx/rounding, filled, style)
/// `default_filled` is inherited from parent SVG element
fn parse_svg_rects(svg: &str, default_filled: bool) -> Vec<(f64, f64, f64, f64, f64, bool, ElementStyle)> {
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
            let style = ElementStyle::parse(tag_content);

            if w > 0.0 && h > 0.0 {
                rects.push((x, y, w, h, rx, filled, style));
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
            let style = ElementStyle::parse(tag_content);

            if w > 0.0 && h > 0.0 {
                rects.push((x, y, w, h, rx, filled, style));
            }
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }

    rects
}

/// Parse all <line> elements from SVG
/// Returns Vec of (x1, y1, x2, y2, style)
fn parse_svg_lines(svg: &str) -> Vec<(f64, f64, f64, f64, ElementStyle)> {
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
            let style = ElementStyle::parse(tag_content);

            lines.push((x1, y1, x2, y2, style));
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let x1 = extract_svg_attr(tag_content, "x1").unwrap_or(0.0);
            let y1 = extract_svg_attr(tag_content, "y1").unwrap_or(0.0);
            let x2 = extract_svg_attr(tag_content, "x2").unwrap_or(0.0);
            let y2 = extract_svg_attr(tag_content, "y2").unwrap_or(0.0);
            let style = ElementStyle::parse(tag_content);

            lines.push((x1, y1, x2, y2, style));
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
/// Returns Vec of (points, closed, style)
fn parse_svg_polylines(svg: &str) -> Vec<(Vec<(f64, f64)>, bool, ElementStyle)> {
    let mut polylines = Vec::new();

    // Parse polylines (not closed)
    let mut search_from = 0;
    while let Some(start) = svg[search_from..].find("<polyline") {
        let abs_start = search_from + start;
        if let Some(end) = svg[abs_start..].find("/>") {
            let tag_content = &svg[abs_start..abs_start + end + 2];
            let points = extract_svg_points(tag_content);
            let style = ElementStyle::parse(tag_content);
            if !points.is_empty() {
                polylines.push((points, false, style));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let points = extract_svg_points(tag_content);
            let style = ElementStyle::parse(tag_content);
            if !points.is_empty() {
                polylines.push((points, false, style));
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
            let style = ElementStyle::parse(tag_content);
            if !points.is_empty() {
                polylines.push((points, true, style));
            }
            search_from = abs_start + end + 2;
        } else if let Some(end) = svg[abs_start..].find('>') {
            let tag_content = &svg[abs_start..abs_start + end + 1];
            let points = extract_svg_points(tag_content);
            let style = ElementStyle::parse(tag_content);
            if !points.is_empty() {
                polylines.push((points, true, style));
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
    // Same base-width quantization as draw_svg_icon (item 1 + item 2's width rounding).
    // Geometry snapping itself is skipped here — a rotated icon has no axis-aligned
    // edges left in screen space to snap.
    let stroke_width = quantize_stroke_width(parse_root_stroke_width(svg) * scale, ctx.dpr());

    // Ancestor opacity from the root <svg>/<g>, multiplied onto every element's own
    // opacity/fill-opacity/stroke-opacity below — same model as draw_svg_icon.
    let root_opacity = parse_root_opacity(svg);

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
            let fill_alpha = path_info.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            ctx.fill();
        }
        if path_info.stroked {
            let stroke_alpha = (path_info.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &path_info.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
        }
    }

    // Circles - rotate center point
    for (ccx, ccy, r, filled, style) in parse_svg_circles(svg, default_filled) {
        let tx = offset_x + ccx * scale;
        let ty = offset_y + ccy * scale;
        let (rtx, rty) = rotate_pt(tx, ty, cx, cy, sin_a, cos_a);
        let tr = r * scale;
        ctx.begin_path();
        draw_circle_bezier(ctx, rtx, rty, tr);
        if filled {
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            ctx.fill();
        } else {
            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
        }
    }

    // Rects - convert to 4 rotated corner points
    for (rect_x, rect_y, rw, rh, _rounding, filled, style) in parse_svg_rects(svg, default_filled) {
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
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
            ctx.fill();
        } else {
            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
        }
    }

    // Lines - rotate both endpoints
    for (x1, y1, x2, y2, style) in parse_svg_lines(svg) {
        let tx1 = offset_x + x1 * scale;
        let ty1 = offset_y + y1 * scale;
        let tx2 = offset_x + x2 * scale;
        let ty2 = offset_y + y2 * scale;
        let (rtx1, rty1) = rotate_pt(tx1, ty1, cx, cy, sin_a, cos_a);
        let (rtx2, rty2) = rotate_pt(tx2, ty2, cx, cy, sin_a, cos_a);
        ctx.begin_path();
        ctx.move_to(rtx1, rty1);
        ctx.line_to(rtx2, rty2);
        let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
        if stroke_alpha < 1.0 {
            ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
        }
        stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
        if stroke_alpha < 1.0 {
            ctx.set_stroke_color(color);
        }
    }

    // Polylines - rotate each point
    for (points, closed, style) in parse_svg_polylines(svg) {
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
            let stroke_alpha = (style.opacity.stroke * root_opacity).clamp(0.0, 1.0);
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(&color_with_alpha(color, stroke_alpha));
            }
            stroke_with_dash(ctx, &style.dash_array, scale, |ctx| ctx.stroke());
            if stroke_alpha < 1.0 {
                ctx.set_stroke_color(color);
            }
        }
    }
}

// =============================================================================
// Multicolor SVG Rendering
// =============================================================================

/// Parsed gradient info extracted from an SVG `<linearGradient>` element.
struct GradientInfo {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    /// (offset 0.0..=1.0, color hex/name)
    stops: Vec<(f32, String)>,
}

/// Extract the value of a quoted attribute from a tag snippet.
///
/// `attr_prefix` should include the opening quote, e.g. `"x1=\""`.
fn extract_attr(text: &str, attr_prefix: &str) -> Option<String> {
    let start = text.find(attr_prefix)? + attr_prefix.len();
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

/// Resolve a gradient URL reference to a fallback solid color by extracting
/// the first stop-color from the gradient definition in the SVG.
///
/// `url_ref` is like `"url(#paint0_linear_1_13)"`.
#[allow(dead_code)]
fn resolve_gradient_color(svg: &str, url_ref: &str) -> Option<String> {
    let id = url_ref.strip_prefix("url(#")?.strip_suffix(')')?;
    let search = format!("id=\"{}\"", id);
    let grad_pos = svg.find(&search)?;
    let after_grad = &svg[grad_pos..];
    let stop_pos = after_grad.find("stop-color=\"")?;
    let color_start = stop_pos + 12; // len("stop-color=\"")
    let color_end = after_grad[color_start..].find('"')?;
    Some(after_grad[color_start..color_start + color_end].to_string())
}

/// Parse a `<linearGradient>` element from the SVG source by its `id` attribute.
///
/// Returns `None` if the gradient is not found or cannot be parsed.
fn parse_gradient(svg: &str, gradient_id: &str) -> Option<GradientInfo> {
    let search = format!("id=\"{}\"", gradient_id);
    let grad_pos = svg.find(&search)?;
    let after = &svg[grad_pos..];

    let grad_end = after.find("</linearGradient>")?;
    let grad_text = &after[..grad_end];

    // Parse coordinate attributes; default to a top→bottom gradient when missing
    let x1 = extract_attr(grad_text, "x1=\"")
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(0.0);
    let y1 = extract_attr(grad_text, "y1=\"")
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(0.0);
    let x2 = extract_attr(grad_text, "x2=\"")
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(0.0);
    let y2 = extract_attr(grad_text, "y2=\"")
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(1.0);

    // Parse all <stop …/> elements within the gradient block
    let mut stops: Vec<(f32, String)> = Vec::new();
    let mut search_from = 0usize;
    while let Some(stop_rel) = grad_text[search_from..].find("<stop") {
        let abs = search_from + stop_rel;
        let remaining = &grad_text[abs..];
        let stop_end = match remaining.find("/>") {
            Some(e) => e,
            None => break,
        };
        let stop_tag = &remaining[..stop_end + 2];

        let offset = extract_attr(stop_tag, "offset=\"")
            .and_then(|s| {
                // offset may be a bare number ("0.5") or a percentage ("50%")
                let s = s.trim_end_matches('%');
                s.parse::<f32>().ok().map(|v| if v > 1.0 { v / 100.0 } else { v })
            })
            .unwrap_or(0.0);

        // stop-color can appear as an attribute or inside a style="..." attribute
        let color = extract_attr(stop_tag, "stop-color=\"")
            .or_else(|| {
                // Try style="stop-color:#xxx"
                extract_attr(stop_tag, "style=\"").and_then(|style| {
                    let sc_pos = style.find("stop-color:")?;
                    let after_sc = style[sc_pos + 11..].trim_start();
                    let end = after_sc.find(|c: char| c == ';' || c == '"').unwrap_or(after_sc.len());
                    Some(after_sc[..end].trim().to_string())
                })
            })
            .unwrap_or_else(|| "black".to_string());

        stops.push((offset, color));
        search_from = abs + stop_end + 2;
    }

    if stops.is_empty() {
        return None;
    }

    Some(GradientInfo { x1, y1, x2, y2, stops })
}

/// Parsed group transform from a `<g transform="...">` element.
/// Supports `translate(tx,ty)` and `scale(sx,sy)` — the most common SVG group transforms.
#[derive(Clone, Copy)]
struct GTransform {
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
}

impl GTransform {
    fn identity() -> Self {
        Self { tx: 0.0, ty: 0.0, sx: 1.0, sy: 1.0 }
    }

    /// Apply the group transform to an SVG-space point, returning the transformed point.
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.tx + x * self.sx, self.ty + y * self.sy)
    }
}

/// Parse the first `<g transform="...">` in the SVG.
/// Supports `translate(tx,ty)` and `scale(sx,sy)` (or `scale(s)`).
/// Returns an identity transform if no `<g transform>` is found.
fn parse_g_transform(svg: &str) -> GTransform {
    let Some(g_start) = svg.find("<g ") else { return GTransform::identity() };
    let g_slice = &svg[g_start..];
    let Some(tag_end) = g_slice.find('>') else { return GTransform::identity() };
    let tag = &g_slice[..tag_end];

    let Some(tf_start) = tag.find("transform=\"") else { return GTransform::identity() };
    let tf_content_start = tf_start + 11;
    let Some(tf_end) = tag[tf_content_start..].find('"') else { return GTransform::identity() };
    let tf = &tag[tf_content_start..tf_content_start + tf_end];

    let mut result = GTransform::identity();

    // Parse translate(tx,ty) or translate(tx ty)
    if let Some(t_start) = tf.find("translate(") {
        let inner_start = t_start + 10;
        if let Some(inner_end) = tf[inner_start..].find(')') {
            let inner = &tf[inner_start..inner_start + inner_end];
            let nums: Vec<f64> = inner
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if nums.len() >= 2 {
                result.tx = nums[0];
                result.ty = nums[1];
            } else if nums.len() == 1 {
                result.tx = nums[0];
            }
        }
    }

    // Parse scale(sx,sy) or scale(s)
    if let Some(s_start) = tf.find("scale(") {
        let inner_start = s_start + 6;
        if let Some(inner_end) = tf[inner_start..].find(')') {
            let inner = &tf[inner_start..inner_start + inner_end];
            let nums: Vec<f64> = inner
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if nums.len() >= 2 {
                result.sx = nums[0];
                result.sy = nums[1];
            } else if nums.len() == 1 {
                result.sx = nums[0];
                result.sy = nums[0];
            }
        }
    }

    result
}

/// Render a multi-color SVG, preserving each path element's original `fill` color
/// attribute instead of overriding with a single color. Suitable for mascot/logo SVGs
/// that use multiple fill colors across their paths.
///
/// # Arguments
/// * `ctx` - Render context
/// * `svg` - SVG string content
/// * `x`, `y` - Top-left position
/// * `width`, `height` - Target dimensions
pub fn draw_svg_multicolor(ctx: &mut dyn RenderContext, svg: &str, x: f64, y: f64, width: f64, height: f64) {
    let (vb_width, vb_height) = parse_viewbox(svg).unwrap_or((24.0, 24.0));
    let scale_x = width / vb_width;
    let scale_y = height / vb_height;
    let scale = scale_x.min(scale_y);
    let offset_x = (x + (width - vb_width * scale) / 2.0).floor();
    let offset_y = (y + (height - vb_height * scale) / 2.0).floor();
    let has_fill_none = svg_root_has_fill_none(svg);
    let default_filled = !has_fill_none;

    // Ancestor opacity from the root <svg>/<g>, multiplied onto every element's own
    // opacity/fill-opacity/stroke-opacity below — same model as draw_svg_icon.
    let root_opacity = parse_root_opacity(svg);

    let dpr = ctx.dpr();
    let gt = parse_g_transform(svg);
    let eff_offset_x = offset_x + gt.tx * scale;
    let eff_offset_y = offset_y + gt.ty * scale;
    let eff_scale_x = gt.sx * scale;
    let eff_scale_y = gt.sy * scale;

    // Render path elements preserving each path's fill and stroke colors.
    // NOTE: vello's fill()/stroke()/fill_linear_gradient() consume the path
    // (path_builder.take()), so when a path needs BOTH fill AND stroke we must
    // build the path geometry twice — which also gives us "fills are not
    // snapped" for free: the fill pass renders with `None`, the stroke pass
    // with `Some(AxisSnap { .. })`, per element loop shared with draw_svg_icon.
    for path_info in parse_svg_paths(svg, default_filled) {
        // ── Fill pass ───────────────────────────────────────────────────
        if path_info.filled {
            ctx.begin_path();
            render_path_data(ctx, &path_info.d, eff_offset_x, eff_offset_y, eff_scale_x, eff_scale_y, None);

            let fill_alpha = path_info.opacity.fill * root_opacity;
            if let Some(ref color) = path_info.fill_color {
                if color.starts_with("url(#") {
                    let grad_id = color.strip_prefix("url(#").and_then(|s| s.strip_suffix(')'));
                    if let Some(id) = grad_id {
                        if let Some(grad) = parse_gradient(svg, id) {
                            // Gradient coords are in root SVG space (defs are outside <g>),
                            // so apply the group transform to map them into screen space.
                            let gx1 = offset_x + gt.apply(grad.x1, grad.y1).0 * scale;
                            let gy1 = offset_y + gt.apply(grad.x1, grad.y1).1 * scale;
                            let gx2 = offset_x + gt.apply(grad.x2, grad.y2).0 * scale;
                            let gy2 = offset_y + gt.apply(grad.x2, grad.y2).1 * scale;
                            let stops_refs: Vec<(f32, &str)> = grad
                                .stops
                                .iter()
                                .map(|(o, c)| (*o, c.as_str()))
                                .collect();
                            ctx.fill_linear_gradient(&stops_refs, gx1, gy1, gx2, gy2);
                        } else {
                            ctx.set_fill_color(&color_with_alpha("black", fill_alpha));
                            ctx.fill();
                        }
                    } else {
                        ctx.set_fill_color(&color_with_alpha("black", fill_alpha));
                        ctx.fill();
                    }
                } else {
                    ctx.set_fill_color(&color_with_alpha(color, fill_alpha));
                    ctx.fill();
                }
            } else {
                ctx.set_fill_color(&color_with_alpha("black", fill_alpha));
                ctx.fill();
            }
        }

        // ── Stroke pass (rebuild path since fill consumed it) ───────────
        if path_info.stroked {
            let sw = quantize_stroke_width(path_info.stroke_width.unwrap_or(1.0) * scale, dpr);

            ctx.begin_path();
            render_path_data(
                ctx, &path_info.d, eff_offset_x, eff_offset_y, eff_scale_x, eff_scale_y,
                Some(AxisSnap { width: sw, dpr }),
            );

            let sc = path_info.stroke_color.as_deref().unwrap_or("black");
            let stroke_alpha = path_info.opacity.stroke * root_opacity;
            ctx.set_stroke_color(&color_with_alpha(sc, stroke_alpha));
            ctx.set_stroke_width(sw);
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            stroke_with_dash(ctx, &path_info.dash_array, scale, |ctx| ctx.stroke());
        }
    }

    // Render rect elements (skip full-size background rects)
    for (rx, ry, rw, rh, rounding, filled, style) in parse_svg_rects(svg, default_filled) {
        if filled {
            // Skip full-size background rects that cover the entire viewbox
            if rw >= vb_width * 0.95 && rh >= vb_height * 0.95 {
                continue;
            }
            let tx = eff_offset_x + rx * eff_scale_x;
            let ty = eff_offset_y + ry * eff_scale_y;
            let tw = rw * eff_scale_x;
            let th = rh * eff_scale_y;
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha("black", fill_alpha));
            if rounding > 0.0 {
                ctx.fill_rounded_rect(tx, ty, tw, th, rounding * scale);
            } else {
                ctx.fill_rect(tx, ty, tw, th);
            }
        }
    }

    // Render circle elements
    for (cx_val, cy_val, r, filled, style) in parse_svg_circles(svg, default_filled) {
        if filled {
            let tx = eff_offset_x + cx_val * eff_scale_x;
            let ty = eff_offset_y + cy_val * eff_scale_y;
            let tr = r * scale;
            ctx.begin_path();
            draw_circle_bezier(ctx, tx, ty, tr);
            let fill_alpha = style.opacity.fill * root_opacity;
            ctx.set_fill_color(&color_with_alpha("black", fill_alpha));
            ctx.fill();
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

    /// Mock RenderContext that tracks path operations, plus the actual color/dash
    /// values passed to `set_fill_color`/`set_stroke_color`/`set_line_dash` (the
    /// `ops` log only records call *names*, which can't tell an opacity-composed
    /// color apart from the plain input color).
    struct MockContext {
        ops: Vec<String>,
        fill_colors: Vec<String>,
        stroke_colors: Vec<String>,
        dash_calls: Vec<Vec<f64>>,
    }

    impl MockContext {
        fn new() -> Self {
            Self { ops: Vec::new(), fill_colors: Vec::new(), stroke_colors: Vec::new(), dash_calls: Vec::new() }
        }
    }

    impl crate::render::Painter for MockContext {
        fn save(&mut self) { self.ops.push("save".to_string()); }
        fn restore(&mut self) { self.ops.push("restore".to_string()); }
        fn translate(&mut self, _x: f64, _y: f64) { self.ops.push("translate".to_string()); }
        fn rotate(&mut self, _angle: f64) { self.ops.push("rotate".to_string()); }
        fn scale(&mut self, _x: f64, _y: f64) { self.ops.push("scale".to_string()); }
        fn set_fill_color(&mut self, color: &str) { self.ops.push("set_fill_color".to_string()); self.fill_colors.push(color.to_string()); }
        fn set_global_alpha(&mut self, _alpha: f64) { self.ops.push("set_global_alpha".to_string()); }
        fn set_stroke_color(&mut self, color: &str) { self.ops.push("set_stroke_color".to_string()); self.stroke_colors.push(color.to_string()); }
        fn set_stroke_width(&mut self, w: f64) { self.ops.push(format!("set_stroke_width({:.2})", w)); }
        fn set_line_cap(&mut self, _cap: &str) { self.ops.push("set_line_cap".to_string()); }
        fn set_line_join(&mut self, _join: &str) { self.ops.push("set_line_join".to_string()); }
        fn set_line_dash(&mut self, pattern: &[f64]) { self.ops.push("set_line_dash".to_string()); self.dash_calls.push(pattern.to_vec()); }
        fn begin_path(&mut self) { self.ops.push("begin_path".to_string()); }
        fn move_to(&mut self, x: f64, y: f64) { self.ops.push(format!("move_to({:.1},{:.1})", x, y)); }
        fn line_to(&mut self, x: f64, y: f64) { self.ops.push(format!("line_to({:.1},{:.1})", x, y)); }
        fn close_path(&mut self) { self.ops.push("close_path".to_string()); }
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("rect".to_string()); }
        fn arc(&mut self, _x: f64, _y: f64, _r: f64, _start: f64, _end: f64) { self.ops.push("arc".to_string()); }
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _start: f64, _end: f64) { self.ops.push("ellipse".to_string()); }
        fn bezier_curve_to(&mut self, _c1x: f64, _c1y: f64, _c2x: f64, _c2y: f64, _x: f64, _y: f64) { self.ops.push("bezier_curve_to".to_string()); }
        fn quadratic_curve_to(&mut self, _cx: f64, _cy: f64, _x: f64, _y: f64) { self.ops.push("quadratic_curve_to".to_string()); }
        fn stroke(&mut self) { self.ops.push("stroke".to_string()); }
        fn fill(&mut self) { self.ops.push("fill".to_string()); }
    }
    impl crate::render::TextRenderer for MockContext {
        fn set_font(&mut self, _font: &str) { self.ops.push("set_font".to_string()); }
        fn set_text_align(&mut self, _align: crate::render::types::TextAlign) { self.ops.push("set_text_align".to_string()); }
        fn set_text_baseline(&mut self, _baseline: crate::render::types::TextBaseline) { self.ops.push("set_text_baseline".to_string()); }
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) { self.ops.push("fill_text".to_string()); }
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) { self.ops.push("stroke_text".to_string()); }
    }
    impl crate::render::TextMetrics for MockContext {
        fn measure_text(&self, _text: &str) -> f64 { 0.0 }
        fn text_bounds(&self, _text: &str, _font: &str) -> crate::render::TextBounds {
            crate::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl crate::render::Masking for MockContext {
        fn clip(&mut self) { self.ops.push("clip".to_string()); }
    }
    impl crate::render::Effects for MockContext {}
    impl crate::render::ShapeHelpers for MockContext {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("fill_rect".to_string()); }
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) { self.ops.push("stroke_rect".to_string()); }
        fn fill_rounded_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64, _r: f64) { self.ops.push("fill_rounded_rect".to_string()); }
        fn stroke_rounded_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64, _r: f64) { self.ops.push("stroke_rounded_rect".to_string()); }
    }
    impl crate::render::GradientPainter for MockContext {}
    impl crate::render::UiEffectHelpers for MockContext {}
    impl crate::render::BatchPainter for MockContext {}
    impl crate::render::context::RenderContext for MockContext { fn dpr(&self) -> f64 { 1.0 } }

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

    #[test]
    fn test_path_stroke_width_override_thinner_line() {
        // Lucide idiom: root stroke-width="2" (honored as the base now — item 1), one
        // construction path overrides to stroke-width="1.2" (thinner). At scale=1.0,
        // dpr=1.0 the base is quantize_stroke_width(2.0 * 1.0, 1.0) == 2.0 (already a
        // whole device pixel); the override is quantize_stroke_width(1.2 * 1.0, 1.0) ==
        // 1.0 (rounds down to the nearest whole device pixel) — strictly thinner.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 1L23 23" stroke-width="1.2"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        println!("Override-width ops:");
        for (i, op) in ctx.ops.iter().enumerate() {
            println!("  [{:3}] {}", i, op);
        }

        // The default width is set once up-front, honoring the root stroke-width="2".
        assert!(ctx.ops.contains(&"set_stroke_width(2.00)".to_string()), "Should set the honored root stroke-width (2.0) up-front");
        // The per-element override (thinner) must appear before the stroke() it applies to.
        let override_idx = ctx.ops.iter().position(|op| op == "set_stroke_width(1.00)");
        assert!(override_idx.is_some(), "Should set the thinner overridden stroke width (1.0) for the path with stroke-width=\"1.2\"");
        let stroke_idx = ctx.ops.iter().position(|op| op == "stroke");
        assert!(stroke_idx.is_some(), "Should call stroke");
        assert!(override_idx.unwrap() < stroke_idx.unwrap(), "Override width must be set before stroke() is called");

        // Width must be restored to the default afterward (no permanent width change).
        let restore_idx = ctx.ops.iter().skip(stroke_idx.unwrap()).position(|op| op == "set_stroke_width(2.00)");
        assert!(restore_idx.is_some(), "Should restore the default stroke width after the overridden stroke");
    }

    #[test]
    fn test_icon_without_per_element_widths_uses_single_default_width() {
        // No per-element stroke-width anywhere — must produce exactly one
        // set_stroke_width call (the fixed default), with no overrides.
        let cloud_svg = crate::render::icons::weather::CLOUD;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, cloud_svg, 100.0, 100.0, 22.0, 22.0, "#ffa726");

        let width_calls: Vec<&String> = ctx.ops.iter().filter(|op| op.starts_with("set_stroke_width")).collect();
        println!("CLOUD set_stroke_width calls: {:?}", width_calls);
        assert_eq!(width_calls.len(), 1, "Icon without per-element stroke-width overrides should call set_stroke_width exactly once (the default)");
    }

    // =============================================================================
    // Opacity / fill-opacity / stroke-opacity / stroke-dasharray tests
    // =============================================================================

    #[test]
    fn test_opacity_composes_alpha_onto_fill_color() {
        // Lucide zone-icon idiom: a tint rect with opacity="0.25" over a full-strength
        // stroke outline (order block / FVG / dealing range / imbalance icons).
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" fill="currentColor" opacity="0.25"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#112233");

        // alpha channel of "#112233" (0xff) scaled by 0.25 -> round(0.25 * 255) = 64 = 0x40.
        assert_eq!(
            ctx.fill_colors,
            vec!["#11223340".to_string()],
            "opacity=\"0.25\" should compose a fill color whose alpha is 25% of the input"
        );
    }

    #[test]
    fn test_nested_root_opacity_multiplies() {
        // Root <svg opacity="0.5"> combined with an element's own fill-opacity="0.5"
        // must multiply (0.5 * 0.5 = 0.25), not just take one of the two factors.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" opacity="0.5"><rect x="4" y="4" width="16" height="16" fill="currentColor" fill-opacity="0.5"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#112233");

        assert_eq!(
            ctx.fill_colors,
            vec!["#11223340".to_string()],
            "root opacity and element fill-opacity should multiply (0.5 * 0.5 = 0.25 -> alpha 0x40), not override each other"
        );
    }

    #[test]
    fn test_dasharray_applied_and_reset_on_rect_and_line() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor"><rect x="2" y="2" width="10" height="10" stroke-dasharray="4 2"/><line x1="1" y1="1" x2="20" y2="20" stroke-dasharray="3 1"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        let rect_dash_idx = ctx.dash_calls.iter().position(|d| d == &vec![4.0, 2.0]);
        assert!(rect_dash_idx.is_some(), "Rect stroke-dasharray should be applied (scaled by the icon's uniform scale)");
        assert_eq!(
            ctx.dash_calls[rect_dash_idx.unwrap() + 1],
            Vec::<f64>::new(),
            "Dash must be reset to solid right after the rect stroke, so it doesn't leak into the next element"
        );

        let line_dash_idx = ctx.dash_calls.iter().position(|d| d == &vec![3.0, 1.0]);
        assert!(line_dash_idx.is_some(), "Line stroke-dasharray should be applied (scaled)");
        assert_eq!(
            ctx.dash_calls[line_dash_idx.unwrap() + 1],
            Vec::<f64>::new(),
            "Dash must be reset to solid right after the line stroke"
        );
    }

    #[test]
    fn test_element_without_opacity_paints_original_color_unchanged() {
        // Regression guard: an icon with no opacity/fill-opacity/stroke-opacity
        // attributes anywhere must still be painted with the exact input color.
        let mut ctx = MockContext::new();
        let jet_svg = crate::render::icons::aviation::JET;
        draw_svg_icon(&mut ctx, jet_svg, 100.0, 100.0, 22.0, 22.0, "#4fc3f7");

        assert_eq!(
            ctx.fill_colors,
            vec!["#4fc3f7".to_string()],
            "Element without opacity should be painted with the exact input color, unchanged"
        );
    }

    #[test]
    fn test_element_without_opacity_stroke_color_set_once_up_front() {
        // Regression guard: without any stroke-opacity, the per-element loop must not
        // add extra set_stroke_color calls beyond the single up-front default.
        let mut ctx = MockContext::new();
        let cloud_svg = crate::render::icons::weather::CLOUD;
        draw_svg_icon(&mut ctx, cloud_svg, 100.0, 100.0, 22.0, 22.0, "#ffa726");

        assert_eq!(
            ctx.stroke_colors,
            vec!["#ffa726".to_string()],
            "Without any stroke-opacity, stroke color should be set exactly once (up-front), with no per-element overrides"
        );
    }

    #[test]
    fn test_rotated_icon_composes_opacity_too() {
        // draw_svg_icon_rotated shares the same opacity/dash helpers as draw_svg_icon.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" fill="currentColor" opacity="0.25"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon_rotated(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#112233", 0.0);

        assert_eq!(
            ctx.fill_colors,
            vec!["#11223340".to_string()],
            "draw_svg_icon_rotated must compose opacity the same way as draw_svg_icon (shared helper)"
        );
    }

    // =============================================================================
    // Pixel-grid stroke width / snapping tests (owner: toolbar icons look blurry)
    // =============================================================================

    #[test]
    fn test_root_stroke_width_honored() {
        // Root declares an explicit stroke-width="4" — item 1 requires it to be
        // honored as the base pen weight instead of the old hardcoded 1.5. At
        // scale=1.0, dpr=1.0 (MockContext) the quantized base is exactly 4.0.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4"><path d="M1 1L23 23"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        assert!(
            ctx.ops.contains(&"set_stroke_width(4.00)".to_string()),
            "Root stroke-width=\"4\" should be honored as the base width, ops: {:?}", ctx.ops
        );
    }

    #[test]
    fn test_root_stroke_width_default_when_absent() {
        // No root stroke-width attribute must fall back to exactly the same default
        // as an SVG that declares stroke-width="1.5" explicitly — "existing icons
        // keep today's weight" (item 1).
        let svg_absent = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor"><path d="M1 1L23 23"/></svg>"#;
        let svg_explicit = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M1 1L23 23"/></svg>"#;

        let mut ctx_absent = MockContext::new();
        draw_svg_icon(&mut ctx_absent, svg_absent, 0.0, 0.0, 24.0, 24.0, "#000000");
        let mut ctx_explicit = MockContext::new();
        draw_svg_icon(&mut ctx_explicit, svg_explicit, 0.0, 0.0, 24.0, 24.0, "#000000");

        let base_width = |ctx: &MockContext| ctx.ops.iter().find(|op| op.starts_with("set_stroke_width")).cloned();
        assert_eq!(
            base_width(&ctx_absent), base_width(&ctx_explicit),
            "Absent root stroke-width should produce the exact same base width as an explicit stroke-width=\"1.5\""
        );
    }

    #[test]
    fn test_h_command_snapped_to_half_pixel_at_1px_width() {
        // Root stroke-width="1" quantizes to an ODD device pixel width — the H
        // command's constant y (the perpendicular axis, 10) must snap to the HALF
        // pixel 10.5 so the 1px horizontal run covers exactly one device pixel row
        // instead of straddling two and reading soft.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1"><path d="M0 10H20"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        assert!(
            ctx.ops.iter().any(|op| op == "line_to(20.0,10.5)"),
            "1px H run should snap its constant y to the half pixel 10.5, ops: {:?}", ctx.ops
        );
    }

    #[test]
    fn test_h_command_snapped_to_whole_pixel_at_2px_width() {
        // Root stroke-width="2" quantizes to an EVEN device pixel width — the H
        // command's constant y (10.3) must snap to the WHOLE pixel 10.0, not a half
        // pixel, so the 2px run centres symmetrically on the pixel grid.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M0 10.3H20"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        assert!(
            ctx.ops.iter().any(|op| op == "line_to(20.0,10.0)"),
            "2px H run should snap its constant y to the whole pixel 10.0, ops: {:?}", ctx.ops
        );
    }

    #[test]
    fn test_diagonal_path_segment_not_snapped() {
        // 'L' is a general line-to (may be diagonal) — per item 2 it must keep its
        // exact geometry always, never snapped to the pixel grid like H/V are.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1"><path d="M1 1L7.3 12.7"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000");

        assert!(
            ctx.ops.iter().any(|op| op == "line_to(7.3,12.7)"),
            "Diagonal L segment must keep its exact (unsnapped) coordinates, ops: {:?}", ctx.ops
        );
    }

    #[test]
    fn test_rotated_icon_h_command_not_snapped() {
        // draw_svg_icon_rotated must skip geometry snapping entirely (item 2: "skip
        // snapping when rotated") — even for an H command, whose non-rotated sibling
        // (draw_svg_icon, see test_h_command_snapped_to_whole_pixel_at_2px_width)
        // WOULD snap the constant y to the pixel grid.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1"><path d="M0 10.3H20"/></svg>"#;

        let mut ctx = MockContext::new();
        draw_svg_icon_rotated(&mut ctx, svg, 0.0, 0.0, 24.0, 24.0, "#000000", 0.0);

        assert!(
            ctx.ops.iter().any(|op| op == "line_to(20.0,10.3)"),
            "Rotated icon must render the H command's exact (unsnapped) y — 10.3, not pixel-grid-snapped, ops: {:?}", ctx.ops
        );
    }
}
