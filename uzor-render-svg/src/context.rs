//! `SvgRenderContext` — [`RenderContext`] backend that SERIALIZES draw
//! calls into a standalone SVG document string instead of rasterizing them.
//!
//! ## Frame lifecycle
//!
//! ```rust
//! use uzor::render::{Painter, ShapeHelpers};
//! use uzor_render_svg::SvgRenderContext;
//!
//! let mut ctx = SvgRenderContext::new(200, 100, 1.0);
//! ctx.set_fill_color("#ff0000");
//! ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
//! let svg = ctx.finish(); // complete <svg>...</svg> document string
//! assert!(svg.starts_with("<svg"));
//! ```

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use uzor::render::{
    BatchPainter, Effects, GlyphMetric, GradientPainter, ImagePainter, Masking, Painter,
    RenderContext, ShapeHelpers, TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer,
    UiEffectHelpers, WrappedLine,
};

use crate::transform::Transform;
use crate::xml::{escape_xml, fmt_num};

// ---------------------------------------------------------------------------
// CSS color storage (parsed once, combined with `global_alpha` at emit time)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    fn parse(s: &str) -> Self {
        let (r, g, b, a) = uzor::render::parse_color(s);
        Self { r, g, b, a }
    }

    /// Multiply this color's own alpha channel by `frac` (`0.0..=1.0`) —
    /// the same "bake the extra alpha into the stored color, don't touch
    /// `global_alpha`" convention `uzor-render-tiny-skia`'s own
    /// `set_fill_color_alpha` override uses.
    fn with_alpha_frac(self, frac: f64) -> Self {
        let frac = frac.clamp(0.0, 1.0);
        let a = ((self.a as f64 / 255.0) * frac * 255.0).round().clamp(0.0, 255.0) as u8;
        Self { a, ..self }
    }

    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    fn alpha_frac(self) -> f64 {
        self.a as f64 / 255.0
    }
}

const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };
const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };

// ---------------------------------------------------------------------------
// Save/restore state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SavedState {
    fill_color: Rgba,
    stroke_color: Rgba,
    stroke_width: f64,
    line_cap: String,
    line_join: String,
    line_dash: Vec<f64>,
    global_alpha: f64,
    font: String,
    text_align: TextAlign,
    text_baseline: TextBaseline,
    transform: Transform,
    current_clip: Option<String>,
}

// ---------------------------------------------------------------------------
// SvgRenderContext
// ---------------------------------------------------------------------------

/// SVG-serializing [`RenderContext`] — accumulates draw calls as SVG markup
/// instead of rasterizing them; [`Self::finish`] returns the complete
/// document.
pub struct SvgRenderContext {
    width: u32,
    height: u32,
    dpr: f64,

    // Style state
    fill_color: Rgba,
    stroke_color: Rgba,
    stroke_width: f64,
    line_cap: String,
    line_join: String,
    line_dash: Vec<f64>,
    global_alpha: f64,

    // Text state
    font: String,
    text_align: TextAlign,
    text_baseline: TextBaseline,

    // Transform (baked into every emitted coordinate — see lib.rs's own
    // module doc for the "why", and `draw_image_rgba`'s documented
    // exception).
    transform: Transform,

    // In-progress path `d` string, accumulated by `begin_path`/`move_to`/
    // `line_to`/`arc`/etc. and consumed by `fill`/`stroke`/`clip`/gradients.
    path_d: String,
    path_has_point: bool,

    state_stack: Vec<SavedState>,

    // Output accumulators
    body: String,
    defs: Vec<String>,

    // Clip state — an id into `defs`' `<clipPath>` entries, or `None`.
    current_clip: Option<String>,
    next_clip_id: u64,
    next_gradient_id: u64,
}

impl SvgRenderContext {
    /// Create a new SVG document context of `width x height` SVG user units
    /// (one unit == one CSS px). `dpr` is not baked into the document's own
    /// coordinate space — it is only surfaced back to draw code via
    /// [`RenderContext::dpr`], exactly like every other backend in this
    /// workspace (`uzor-render-tiny-skia::TinySkiaCpuRenderContext::new` has
    /// the identical contract).
    pub fn new(width: u32, height: u32, dpr: f64) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            dpr,
            fill_color: BLACK,
            stroke_color: WHITE,
            stroke_width: 1.0,
            line_cap: "butt".to_string(),
            line_join: "miter".to_string(),
            line_dash: Vec::new(),
            global_alpha: 1.0,
            font: "12px sans-serif".to_string(),
            text_align: TextAlign::default(),
            text_baseline: TextBaseline::default(),
            transform: Transform::identity(),
            path_d: String::new(),
            path_has_point: false,
            state_stack: Vec::new(),
            body: String::new(),
            defs: Vec::new(),
            current_clip: None,
            next_clip_id: 0,
            next_gradient_id: 0,
        }
    }

    /// Serialize every draw call issued so far into a complete, standalone
    /// SVG document — a bare `<svg>` root (no `<?xml?>` prolog), which every
    /// real consumer (browsers, `resvg`, Inkscape) accepts equally well.
    pub fn finish(&self) -> String {
        let mut out = String::with_capacity(256 + self.body.len());
        out.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            self.width, self.height, self.width, self.height
        ));
        if !self.defs.is_empty() {
            out.push_str("<defs>\n");
            for d in &self.defs {
                out.push_str(d);
                out.push('\n');
            }
            out.push_str("</defs>\n");
        }
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    // -----------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        self.transform.apply(x, y)
    }

    fn transform_scale(&self) -> f64 {
        self.transform.scale_factor()
    }

    fn clip_attr(&self) -> String {
        match &self.current_clip {
            Some(id) => format!(" clip-path=\"url(#{id})\""),
            None => String::new(),
        }
    }

    fn fill_paint_attrs(&self) -> (String, String) {
        let alpha = (self.fill_color.alpha_frac() * self.global_alpha).clamp(0.0, 1.0);
        (self.fill_color.hex(), fmt_num(alpha))
    }

    fn stroke_paint_attrs(&self) -> (String, String) {
        let alpha = (self.stroke_color.alpha_frac() * self.global_alpha).clamp(0.0, 1.0);
        (self.stroke_color.hex(), fmt_num(alpha))
    }

    fn take_path_d(&mut self) -> Option<String> {
        self.path_has_point = false;
        if self.path_d.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.path_d))
        }
    }

    fn append_moveto(&mut self, x: f64, y: f64) {
        let (tx, ty) = self.apply(x, y);
        if !self.path_d.is_empty() {
            self.path_d.push(' ');
        }
        self.path_d.push_str(&format!("M {} {}", fmt_num(tx), fmt_num(ty)));
    }

    fn append_lineto(&mut self, x: f64, y: f64) {
        let (tx, ty) = self.apply(x, y);
        if !self.path_d.is_empty() {
            self.path_d.push(' ');
        }
        self.path_d.push_str(&format!("L {} {}", fmt_num(tx), fmt_num(ty)));
    }

    fn append_quadto(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        let (tcx, tcy) = self.apply(cx, cy);
        let (tx, ty) = self.apply(x, y);
        if !self.path_d.is_empty() {
            self.path_d.push(' ');
        }
        self.path_d.push_str(&format!("Q {} {} {} {}", fmt_num(tcx), fmt_num(tcy), fmt_num(tx), fmt_num(ty)));
    }

    fn append_cubicto(&mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) {
        let (t1x, t1y) = self.apply(c1x, c1y);
        let (t2x, t2y) = self.apply(c2x, c2y);
        let (tx, ty) = self.apply(x, y);
        if !self.path_d.is_empty() {
            self.path_d.push(' ');
        }
        self.path_d.push_str(&format!(
            "C {} {} {} {} {} {}",
            fmt_num(t1x), fmt_num(t1y), fmt_num(t2x), fmt_num(t2y), fmt_num(tx), fmt_num(ty)
        ));
    }

    fn append_close(&mut self) {
        if !self.path_d.is_empty() {
            self.path_d.push(' ');
        }
        self.path_d.push('Z');
    }

    /// Cubic-bezier approximation of a circular arc — ported from
    /// `uzor-render-tiny-skia`'s own `arc_to_cubics` (f64 instead of f32,
    /// appending SVG path commands via `append_lineto`/`append_moveto`/
    /// `append_cubicto` instead of a `tiny_skia::PathBuilder`).
    fn arc_to_cubics(&mut self, cx: f64, cy: f64, r: f64, start: f64, end: f64, has_current_point: bool) {
        let mut sweep = end - start;
        if sweep == 0.0 {
            return;
        }
        if sweep > std::f64::consts::TAU {
            sweep = std::f64::consts::TAU;
        }
        if sweep < -std::f64::consts::TAU {
            sweep = -std::f64::consts::TAU;
        }

        let n_segs = ((sweep.abs() / (std::f64::consts::PI / 2.0)).ceil() as u32).max(1);
        let seg_angle = sweep / n_segs as f64;
        let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

        let a0 = start;
        let start_x = cx + r * a0.cos();
        let start_y = cy + r * a0.sin();
        if has_current_point {
            self.append_lineto(start_x, start_y);
        } else {
            self.append_moveto(start_x, start_y);
        }

        let mut a = start;
        for _ in 0..n_segs {
            let a1 = a + seg_angle;
            let (sin_a, cos_a) = a.sin_cos();
            let (sin_a1, cos_a1) = a1.sin_cos();

            let p0x = cx + r * cos_a;
            let p0y = cy + r * sin_a;
            let p3x = cx + r * cos_a1;
            let p3y = cy + r * sin_a1;

            let cp1x = p0x - k * r * sin_a;
            let cp1y = p0y + k * r * cos_a;
            let cp2x = p3x + k * r * sin_a1;
            let cp2y = p3y - k * r * cos_a1;

            self.append_cubicto(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
            a = a1;
        }
    }

    /// Same as [`Self::arc_to_cubics`] with independent x/y radii — ported
    /// from `ellipse_to_cubics`.
    #[allow(clippy::too_many_arguments)]
    fn ellipse_to_cubics(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, start: f64, end: f64, has_current_point: bool) {
        let mut sweep = end - start;
        if sweep == 0.0 {
            return;
        }
        if sweep > std::f64::consts::TAU {
            sweep = std::f64::consts::TAU;
        }
        if sweep < -std::f64::consts::TAU {
            sweep = -std::f64::consts::TAU;
        }

        let n_segs = ((sweep.abs() / (std::f64::consts::PI / 2.0)).ceil() as u32).max(1);
        let seg_angle = sweep / n_segs as f64;
        let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

        let a0 = start;
        let start_x = cx + rx * a0.cos();
        let start_y = cy + ry * a0.sin();
        if has_current_point {
            self.append_lineto(start_x, start_y);
        } else {
            self.append_moveto(start_x, start_y);
        }

        let mut a = start;
        for _ in 0..n_segs {
            let a1 = a + seg_angle;
            let (sin_a, cos_a) = a.sin_cos();
            let (sin_a1, cos_a1) = a1.sin_cos();

            let p0x = cx + rx * cos_a;
            let p0y = cy + ry * sin_a;
            let p3x = cx + rx * cos_a1;
            let p3y = cy + ry * sin_a1;

            let cp1x = p0x - k * rx * sin_a;
            let cp1y = p0y + k * ry * cos_a;
            let cp2x = p3x + k * rx * sin_a1;
            let cp2y = p3y - k * ry * cos_a1;

            self.append_cubicto(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
            a = a1;
        }
    }

    fn axis_aligned_rect(&self, x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.apply(x, y);
        let (x1, y1) = self.apply(x + w, y + h);
        (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }

    fn rect_path_d(&self, x: f64, y: f64, w: f64, h: f64) -> String {
        let p0 = self.apply(x, y);
        let p1 = self.apply(x + w, y);
        let p2 = self.apply(x + w, y + h);
        let p3 = self.apply(x, y + h);
        format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            fmt_num(p0.0), fmt_num(p0.1),
            fmt_num(p1.0), fmt_num(p1.1),
            fmt_num(p2.0), fmt_num(p2.1),
            fmt_num(p3.0), fmt_num(p3.1)
        )
    }

    /// Native `<rect>` when the current transform is a plain
    /// translate+scale (axis-aligned); a closed 4-point `<path>` fallback
    /// otherwise (rotation/skew can't be expressed by a `<rect>`'s own
    /// axis-aligned `x/y/width/height` attributes).
    fn shape_element(&self, x: f64, y: f64, w: f64, h: f64, paint_attrs: &str, clip: &str) -> String {
        if self.transform.is_axis_aligned() {
            let (rx, ry, rw, rh) = self.axis_aligned_rect(x, y, w, h);
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {paint_attrs}{clip}/>\n",
                fmt_num(rx), fmt_num(ry), fmt_num(rw), fmt_num(rh)
            )
        } else {
            let d = self.rect_path_d(x, y, w, h);
            format!("<path d=\"{d}\" {paint_attrs}{clip}/>\n")
        }
    }

    fn clip_with_rule(&mut self, rule: Option<&str>) {
        let Some(d) = self.take_path_d() else { return };
        let id = format!("uzorclip{}", self.next_clip_id);
        self.next_clip_id += 1;
        let rule_attr = rule.map(|r| format!(" clip-rule=\"{r}\"")).unwrap_or_default();
        let parent_attr = self.current_clip.as_ref().map(|p| format!(" clip-path=\"url(#{p})\"")).unwrap_or_default();
        self.defs.push(format!("<clipPath id=\"{id}\"{parent_attr}><path d=\"{d}\"{rule_attr}/></clipPath>"));
        self.current_clip = Some(id);
    }
}

// ---------------------------------------------------------------------------
// Glyph-outline path re-anchoring (fill_text)
// ---------------------------------------------------------------------------

/// Re-anchor + re-transform a raw glyph-outline `d` string (as produced by
/// `uzor::shaper::text_to_path`, whose own local frame has the text's TOP
/// edge at y≈0 and the first line's baseline at y≈`ascent`) so it paints at
/// `(pen_x, pen_y)` in LOCAL canvas coordinates, then bakes the current
/// transform into every coordinate exactly like every other path this
/// backend emits.
///
/// `dx`/`dy` are the offset that maps the raw local frame's own origin onto
/// the caller's local `(pen_x, pen_y)` anchor (see `fill_text`'s own call
/// site: `dy = pen_y - ascent` undoes the raw frame's own
/// baseline-below-origin offset).
///
/// Only handles the exact command set `text_to_path` emits (`M`/`L`/`Q`/`C`/
/// `Z`, absolute integer coordinates) — not a general SVG path parser
/// (that's `uzor::render::path`'s job, for a different input shape).
fn retarget_glyph_path(d: &str, dx: f64, dy: f64, transform: &Transform) -> String {
    let mut out = String::new();
    let mut tokens = d.split_whitespace();
    while let Some(tok) = tokens.next() {
        match tok {
            "M" | "L" => {
                let (Some(x), Some(y)) = (next_f64(&mut tokens), next_f64(&mut tokens)) else { break };
                let (tx, ty) = transform.apply(x + dx, y + dy);
                push_cmd(&mut out, tok, &[tx, ty]);
            }
            "Q" => {
                let Some((cx, cy, ex, ey)) = next_four(&mut tokens) else { break };
                let (tcx, tcy) = transform.apply(cx + dx, cy + dy);
                let (tex, tey) = transform.apply(ex + dx, ey + dy);
                push_cmd(&mut out, "Q", &[tcx, tcy, tex, tey]);
            }
            "C" => {
                let Some((c1x, c1y, c2x, c2y, ex, ey)) = next_six(&mut tokens) else { break };
                let (t1x, t1y) = transform.apply(c1x + dx, c1y + dy);
                let (t2x, t2y) = transform.apply(c2x + dx, c2y + dy);
                let (tex, tey) = transform.apply(ex + dx, ey + dy);
                push_cmd(&mut out, "C", &[t1x, t1y, t2x, t2y, tex, tey]);
            }
            "Z" => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push('Z');
            }
            _ => {}
        }
    }
    out
}

fn next_f64<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<f64> {
    tokens.next()?.parse().ok()
}

fn next_four<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<(f64, f64, f64, f64)> {
    Some((next_f64(tokens)?, next_f64(tokens)?, next_f64(tokens)?, next_f64(tokens)?))
}

fn next_six<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<(f64, f64, f64, f64, f64, f64)> {
    Some((
        next_f64(tokens)?, next_f64(tokens)?, next_f64(tokens)?,
        next_f64(tokens)?, next_f64(tokens)?, next_f64(tokens)?,
    ))
}

fn push_cmd(out: &mut String, cmd: &str, coords: &[f64]) {
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(cmd);
    for c in coords {
        out.push(' ');
        out.push_str(&fmt_num(*c));
    }
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl Painter for SvgRenderContext {
    fn save(&mut self) {
        self.state_stack.push(SavedState {
            fill_color: self.fill_color,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            line_cap: self.line_cap.clone(),
            line_join: self.line_join.clone(),
            line_dash: self.line_dash.clone(),
            global_alpha: self.global_alpha,
            font: self.font.clone(),
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            transform: self.transform,
            current_clip: self.current_clip.clone(),
        });
    }

    fn restore(&mut self) {
        if let Some(s) = self.state_stack.pop() {
            self.fill_color = s.fill_color;
            self.stroke_color = s.stroke_color;
            self.stroke_width = s.stroke_width;
            self.line_cap = s.line_cap;
            self.line_join = s.line_join;
            self.line_dash = s.line_dash;
            self.global_alpha = s.global_alpha;
            self.font = s.font;
            self.text_align = s.text_align;
            self.text_baseline = s.text_baseline;
            self.transform = s.transform;
            self.current_clip = s.current_clip;
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_translate(x, y);
    }

    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.pre_rotate(angle);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_scale(x, y);
    }

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = Rgba::parse(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        self.fill_color = Rgba::parse(color).with_alpha_frac(alpha);
    }

    fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = Rgba::parse(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width;
    }

    fn set_line_dash(&mut self, pattern: &[f64]) {
        self.line_dash = pattern.to_vec();
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round" => "round",
            "square" => "square",
            _ => "butt",
        }
        .to_string();
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => "round",
            "bevel" => "bevel",
            _ => "miter",
        }
        .to_string();
    }

    fn begin_path(&mut self) {
        self.path_d.clear();
        self.path_has_point = false;
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.append_moveto(x, y);
        self.path_has_point = true;
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.append_lineto(x, y);
        self.path_has_point = true;
    }

    fn close_path(&mut self) {
        self.append_close();
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.append_moveto(x, y);
        self.append_lineto(x + w, y);
        self.append_lineto(x + w, y + h);
        self.append_lineto(x, y + h);
        self.append_close();
        self.path_has_point = true;
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        let has_point = self.path_has_point;
        self.arc_to_cubics(cx, cy, radius, start_angle, end_angle, has_point);
        self.path_has_point = true;
    }

    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, _rotation: f64, start: f64, end: f64) {
        let has_point = self.path_has_point;
        self.ellipse_to_cubics(cx, cy, rx, ry, start, end, has_point);
        self.path_has_point = true;
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.append_quadto(cpx, cpy, x, y);
        self.path_has_point = true;
    }

    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.append_cubicto(cp1x, cp1y, cp2x, cp2y, x, y);
        self.path_has_point = true;
    }

    fn stroke(&mut self) {
        let Some(d) = self.take_path_d() else { return };
        let (color, op) = self.stroke_paint_attrs();
        let width = self.stroke_width * self.transform_scale();
        let clip = self.clip_attr();
        let dash_attr = if self.line_dash.is_empty() {
            String::new()
        } else {
            let scale = self.transform_scale();
            let vals: Vec<String> = self.line_dash.iter().map(|v| fmt_num(v * scale)).collect();
            format!(" stroke-dasharray=\"{}\"", vals.join(","))
        };
        self.body.push_str(&format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-opacity=\"{op}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"{dash_attr}{clip}/>\n",
            fmt_num(width), self.line_cap, self.line_join
        ));
    }

    fn fill(&mut self) {
        let Some(d) = self.take_path_d() else { return };
        let (color, op) = self.fill_paint_attrs();
        let clip = self.clip_attr();
        self.body.push_str(&format!("<path d=\"{d}\" fill=\"{color}\" fill-opacity=\"{op}\"{clip}/>\n"));
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers
// ---------------------------------------------------------------------------

impl ShapeHelpers for SvgRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (fill, op) = self.fill_paint_attrs();
        let attrs = format!("fill=\"{fill}\" fill-opacity=\"{op}\"");
        let clip = self.clip_attr();
        let el = self.shape_element(x, y, w, h, &attrs, &clip);
        self.body.push_str(&el);
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (color, op) = self.stroke_paint_attrs();
        let width = self.stroke_width * self.transform_scale();
        let attrs = format!(
            "fill=\"none\" stroke=\"{color}\" stroke-opacity=\"{op}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
            fmt_num(width), self.line_cap, self.line_join
        );
        let clip = self.clip_attr();
        let el = self.shape_element(x, y, w, h, &attrs, &clip);
        self.body.push_str(&el);
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for SvgRenderContext {
    fn set_font(&mut self, font: &str) {
        self.font = font.to_string();
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    /// Paints outlined glyph paths (via `uzor::shaper::text_to_path`)
    /// positioned per the same baseline/alignment conventions
    /// `uzor-render-tiny-skia::fill_text` uses (`x_off` from `text_align`,
    /// `y_off` from `text_baseline`, both relative to the real font ascent),
    /// plus an invisible `opacity="0"` `<text>` companion so the document
    /// stays searchable/selectable (see `lib.rs`'s own "Text" design note).
    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() {
            return;
        }
        let font = self.font.clone();
        let total_w = self.measure_text(text);
        let x_off = match self.text_align {
            TextAlign::Center => -(total_w / 2.0),
            TextAlign::Right => -total_w,
            TextAlign::Left => 0.0,
        };

        let font_info = uzor::fonts::parse_css_font(&font);
        let font_size = font_info.size as f64;
        let lines = uzor::shaper::measure_glyphs_wrapped(text, &font, f64::MAX);
        let ascent = lines.first().map(|l| l.baseline_y).unwrap_or(font_size * 0.9);

        let y_off = match self.text_baseline {
            TextBaseline::Top => ascent,
            TextBaseline::Middle => ascent / 2.0,
            TextBaseline::Bottom => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };

        let pen_x = x + x_off;
        let pen_y = y + y_off;

        let escaped = escape_xml(text);
        let clip = self.clip_attr();
        let raw_d = uzor::shaper::text_to_path(text, &font);

        if raw_d.is_empty() {
            // Whitespace-only (or shaping-failed) run — nothing to paint,
            // but still emit the invisible companion so the string stays
            // searchable/selectable in the output document.
            let (tx, ty) = self.apply(pen_x, pen_y);
            self.body.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" opacity=\"0\" aria-hidden=\"true\">{escaped}</text>\n",
                fmt_num(tx), fmt_num(ty), fmt_num(font_size)
            ));
            return;
        }

        let glyph_d = retarget_glyph_path(&raw_d, pen_x, pen_y - ascent, &self.transform);
        let (fill, op) = self.fill_paint_attrs();
        let (tx, ty) = self.apply(pen_x, pen_y);

        self.body.push_str(&format!(
            "<g data-text=\"{escaped}\"{clip}>\n<path d=\"{glyph_d}\" fill=\"{fill}\" fill-opacity=\"{op}\"/>\n<text x=\"{}\" y=\"{}\" font-size=\"{}\" opacity=\"0\" aria-hidden=\"true\">{escaped}</text>\n</g>\n",
            fmt_num(tx), fmt_num(ty), fmt_num(font_size)
        ));
    }

    /// Same "solid fill of the stroke color" approximation
    /// `uzor-render-tiny-skia::stroke_text` uses (real stroked/outlined
    /// glyph rendering is out of scope for both backends).
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        let saved = self.fill_color;
        self.fill_color = self.stroke_color;
        self.fill_text(text, x, y);
        self.fill_color = saved;
    }
}

// ---------------------------------------------------------------------------
// TextMetrics — delegates entirely to `uzor::shaper` (cosmic-text), same as
// every other `shaper`-feature backend in this workspace.
// ---------------------------------------------------------------------------

impl TextMetrics for SvgRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        uzor::shaper::measure_glyphs(text, &self.font).iter().map(|g| g.advance).sum()
    }

    /// `ascent` comes from the SAME cosmic-text line metrics
    /// `uzor::shaper::measure_glyphs_wrapped` already computes (the first
    /// visual line's `baseline_y`, at `line_top == 0.0`) rather than a
    /// separate fontdue-based lookup — this backend depends only on the
    /// `shaper` feature, not `fontdue`, so there is no second metrics
    /// source to keep in sync. `descent` is derived from the SAME
    /// `Metrics::new(font_size, font_size * 1.2)` line-height convention
    /// `uzor::shaper`'s own internals use.
    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        if text.is_empty() {
            return TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 };
        }
        let font_info = uzor::fonts::parse_css_font(font);
        let font_size = font_info.size as f64;
        let w: f64 = uzor::shaper::measure_glyphs(text, font).iter().map(|g| g.advance).sum();
        let lines = uzor::shaper::measure_glyphs_wrapped(text, font, f64::MAX);
        let ascent = lines.first().map(|l| l.baseline_y).unwrap_or(font_size * 0.9);
        let descent = (font_size * 1.2 - ascent).max(0.0);
        TextBounds { x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent }
    }

    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<GlyphMetric> {
        uzor::shaper::measure_glyphs(text, font)
    }

    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<WrappedLine> {
        uzor::shaper::measure_glyphs_wrapped(text, font, max_width)
    }

    fn text_to_path(&self, text: &str, font: &str) -> String {
        uzor::shaper::text_to_path(text, font)
    }
}

// ---------------------------------------------------------------------------
// Masking — clip regions become `<clipPath>` defs, referenced via
// `clip-path="url(#id)"` on every subsequently emitted element.
// ---------------------------------------------------------------------------

impl Masking for SvgRenderContext {
    fn clip(&mut self) {
        self.clip_with_rule(None);
    }

    /// Even-odd clip rule (ring-shaped clips, e.g. outer rect + inner
    /// shape) — the default trait impl would fall back to nonzero winding;
    /// this override matches `uzor-render-tiny-skia`'s own real support.
    fn push_clip_svg_path_even_odd(&mut self, d: &str) {
        uzor::render::emit_svg_path(self, d);
        self.save();
        self.clip_with_rule(Some("evenodd"));
    }
}

// ---------------------------------------------------------------------------
// Effects — NOT implemented (documented, not silent): shadow/blend-mode
// have no vector-SVG equivalent attempted this pass. The trait's own doc
// comment already declares this a "visually tolerable" absence (unlike
// `BackdropBlur`'s opt-in-only shape) — every method keeps its own no-op
// trait default.
// ---------------------------------------------------------------------------

impl Effects for SvgRenderContext {}

// ---------------------------------------------------------------------------
// BatchPainter — every default impl already loops over this backend's own
// `Painter` primitives ("implement via loops over the single-ops, correct
// beats clever" — this task's own instruction) — no override needed.
// ---------------------------------------------------------------------------

impl BatchPainter for SvgRenderContext {}

// ---------------------------------------------------------------------------
// GradientPainter — a REAL implementation (`<linearGradient>`/
// `<radialGradient>` defs), not the trait's own flat-fill fallback.
// ---------------------------------------------------------------------------

impl GradientPainter for SvgRenderContext {
    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        let Some(d) = self.take_path_d() else { return };
        if stops.is_empty() {
            return;
        }
        let id = format!("uzorgrad{}", self.next_gradient_id);
        self.next_gradient_id += 1;
        let (tx1, ty1) = self.apply(x1, y1);
        let (tx2, ty2) = self.apply(x2, y2);
        let global_alpha = self.global_alpha;
        let mut stops_svg = String::new();
        for &(offset, color) in stops {
            let (r, g, b, a) = uzor::render::parse_color(color);
            let alpha = ((a as f64 / 255.0) * global_alpha).clamp(0.0, 1.0);
            stops_svg.push_str(&format!(
                "<stop offset=\"{}\" stop-color=\"#{r:02x}{g:02x}{b:02x}\" stop-opacity=\"{}\"/>",
                fmt_num(offset.clamp(0.0, 1.0) as f64), fmt_num(alpha)
            ));
        }
        self.defs.push(format!(
            "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops_svg}</linearGradient>",
            fmt_num(tx1), fmt_num(ty1), fmt_num(tx2), fmt_num(ty2)
        ));
        let clip = self.clip_attr();
        self.body.push_str(&format!("<path d=\"{d}\" fill=\"url(#{id})\"{clip}/>\n"));
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f32, &str)], _x: f64, _y: f64, _w: f64, _h: f64) {
        let Some(d) = self.take_path_d() else { return };
        if stops.is_empty() {
            return;
        }
        let id = format!("uzorgrad{}", self.next_gradient_id);
        self.next_gradient_id += 1;
        let (tcx, tcy) = self.apply(cx, cy);
        let tr = r * self.transform_scale();
        let global_alpha = self.global_alpha;
        let mut stops_svg = String::new();
        for &(offset, color) in stops {
            let (rr, gg, bb, a) = uzor::render::parse_color(color);
            let alpha = ((a as f64 / 255.0) * global_alpha).clamp(0.0, 1.0);
            stops_svg.push_str(&format!(
                "<stop offset=\"{}\" stop-color=\"#{rr:02x}{gg:02x}{bb:02x}\" stop-opacity=\"{}\"/>",
                fmt_num(offset.clamp(0.0, 1.0) as f64), fmt_num(alpha)
            ));
        }
        self.defs.push(format!(
            "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\">{stops_svg}</radialGradient>",
            fmt_num(tcx), fmt_num(tcy), fmt_num(tr)
        ));
        let clip = self.clip_attr();
        self.body.push_str(&format!("<path d=\"{d}\" fill=\"url(#{id})\"{clip}/>\n"));
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — no backdrop-blur support (vector SVG has no raster
// backdrop to sample) — every method's own no-op/`false` trait default
// applies.
// ---------------------------------------------------------------------------

impl UiEffectHelpers for SvgRenderContext {}

// ---------------------------------------------------------------------------
// ImagePainter — `draw_image_rgba` is a REAL embed: straight-alpha RGBA8 is
// PNG-encoded (via the `png` crate) then base64'd into a `data:` URI on a
// native `<image>` element.
// ---------------------------------------------------------------------------

impl ImagePainter for SvgRenderContext {
    /// No URL/id-based image cache in this backend — matches
    /// `uzor-render-tiny-skia`'s own documented `false` for the identical
    /// reason (this backend never loads images by id, only raw RGBA via
    /// `draw_image_rgba`).
    fn draw_image(&mut self, _image_id: &str, _x: f64, _y: f64, _width: f64, _height: f64) -> bool {
        false
    }

    fn draw_image_rgba(&mut self, data: &[u8], img_width: u32, img_height: u32, x: f64, y: f64, width: f64, height: f64) {
        if img_width == 0 || img_height == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let expected_len = img_width as usize * img_height as usize * 4;
        if data.len() != expected_len {
            return;
        }

        let mut png_bytes: Vec<u8> = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, img_width, img_height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let Ok(mut writer) = encoder.write_header() else { return };
            if writer.write_image_data(data).is_err() {
                return;
            }
        }
        let b64 = BASE64.encode(&png_bytes);
        let href = format!("data:image/png;base64,{b64}");
        let opacity = fmt_num(self.global_alpha);
        let clip = self.clip_attr();

        if self.transform.is_axis_aligned() {
            let (rx, ry, rw, rh) = self.axis_aligned_rect(x, y, width, height);
            self.body.push_str(&format!(
                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" opacity=\"{opacity}\" preserveAspectRatio=\"none\" href=\"{href}\"{clip}/>\n",
                fmt_num(rx), fmt_num(ry), fmt_num(rw), fmt_num(rh)
            ));
        } else {
            // Raster content can't be expressed as path `d` coordinates —
            // a `transform` attribute on THIS element is the one
            // documented exception to this backend's "bake transforms
            // into coordinates" convention (see lib.rs's own module doc).
            let base = self.transform.pre_translate(x, y).pre_scale(width / img_width as f64, height / img_height as f64);
            self.body.push_str(&format!(
                "<image x=\"0\" y=\"0\" width=\"{img_width}\" height=\"{img_height}\" opacity=\"{opacity}\" preserveAspectRatio=\"none\" transform=\"matrix({} {} {} {} {} {})\" href=\"{href}\"{clip}/>\n",
                fmt_num(base.sx), fmt_num(base.ky), fmt_num(base.kx), fmt_num(base.sy), fmt_num(base.tx), fmt_num(base.ty)
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// RenderContext
// ---------------------------------------------------------------------------

impl RenderContext for SvgRenderContext {
    fn dpr(&self) -> f64 {
        self.dpr
    }

    fn image_painter(&mut self) -> Option<&mut dyn ImagePainter> {
        Some(self)
    }

    // Offscreen render targets are not supported — SVG has no obvious
    // "compositable raster surface" primitive to back one with; every
    // `RenderContext` default (`supports_offscreen_targets` -> `false`,
    // push/pop/draw_cached/resize/free -> no-op) applies.
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> SvgRenderContext {
        SvgRenderContext::new(100, 100, 1.0)
    }

    #[test]
    fn fill_rect_emits_a_native_rect_element() {
        let mut c = ctx();
        c.set_fill_color("#ff0000");
        c.fill_rect(10.0, 20.0, 30.0, 40.0);
        let svg = c.finish();
        assert!(svg.contains("<rect"), "expected a native <rect> element, got: {svg}");
        assert!(svg.contains("x=\"10\""));
        assert!(svg.contains("y=\"20\""));
        assert!(svg.contains("width=\"30\""));
        assert!(svg.contains("height=\"40\""));
        assert!(svg.contains("fill=\"#ff0000\""));
    }

    #[test]
    fn a_circle_drawn_via_arc_and_fill_emits_a_closed_bezier_path() {
        let mut c = ctx();
        c.set_fill_color("#00ff00");
        c.begin_path();
        c.arc(50.0, 50.0, 20.0, 0.0, std::f64::consts::TAU);
        c.fill();
        let svg = c.finish();
        assert!(svg.contains("<path d=\"M"), "expected a <path> starting with M, got: {svg}");
        assert!(svg.contains(" C "), "a bezier-approximated circle must contain cubic C segments, got: {svg}");
    }

    #[test]
    fn a_generic_path_emits_a_single_moveto_lineto_close_path_element() {
        let mut c = ctx();
        c.set_fill_color("#0000ff");
        c.begin_path();
        c.move_to(0.0, 0.0);
        c.line_to(10.0, 0.0);
        c.line_to(10.0, 10.0);
        c.close_path();
        c.fill();
        let svg = c.finish();
        assert!(
            svg.contains("<path d=\"M 0 0 L 10 0 L 10 10 Z\""),
            "unexpected path d, got: {svg}"
        );
    }

    #[test]
    fn translate_and_scale_move_and_resize_a_rect() {
        let mut c = ctx();
        c.set_fill_color("#ffffff");
        c.translate(10.0, 20.0);
        c.scale(2.0, 3.0);
        c.fill_rect(0.0, 0.0, 5.0, 5.0);
        let svg = c.finish();
        // (0,0) -> (10,20); (5,5) -> (10 + 2*5, 20 + 3*5) = (20, 35) ->
        // width=10, height=15.
        assert!(svg.contains("x=\"10\""), "got: {svg}");
        assert!(svg.contains("y=\"20\""), "got: {svg}");
        assert!(svg.contains("width=\"10\""), "got: {svg}");
        assert!(svg.contains("height=\"15\""), "got: {svg}");
    }

    #[test]
    fn alpha_composes_from_color_alpha_times_global_alpha() {
        let mut c = ctx();
        c.set_fill_color("#ff000080"); // alpha 128/255
        c.set_global_alpha(0.5);
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        let svg = c.finish();
        let needle = "fill-opacity=\"";
        let start = svg.find(needle).expect("fill-opacity attribute present") + needle.len();
        let end = svg[start..].find('"').expect("closing quote") + start;
        let value: f64 = svg[start..end].parse().expect("numeric opacity");
        let expected = (128.0 / 255.0) * 0.5;
        assert!((value - expected).abs() < 0.001, "expected ~{expected}, got {value}");
    }

    #[test]
    fn xml_escaping_applies_to_fill_text_label_content() {
        let mut c = ctx();
        c.set_fill_color("#000000");
        c.set_font("16px sans-serif");
        c.fill_text("<a&b>", 5.0, 5.0);
        let svg = c.finish();
        assert!(svg.contains("&lt;a&amp;b&gt;"), "expected escaped label text, got: {svg}");
        assert!(!svg.contains("<a&b>"), "raw unescaped text must never appear in the document");
    }

    #[test]
    fn linear_gradient_fill_emits_a_gradient_def_and_a_referencing_path() {
        let mut c = ctx();
        c.begin_path();
        c.rect(0.0, 0.0, 10.0, 10.0);
        c.fill_linear_gradient(&[(0.0, "#ff0000"), (1.0, "#0000ff")], 0.0, 0.0, 10.0, 0.0);
        let svg = c.finish();
        assert!(svg.contains("<linearGradient"), "got: {svg}");
        assert!(svg.contains("fill=\"url(#"), "got: {svg}");
    }

    #[test]
    fn draw_image_rgba_embeds_a_base64_png_data_uri() {
        let mut c = ctx();
        #[rustfmt::skip]
        let rgba: [u8; 16] = [
            255, 0, 0, 255,    0, 255, 0, 255,
              0, 0, 255, 255,  255, 255, 0, 255,
        ];
        c.draw_image_rgba(&rgba, 2, 2, 0.0, 0.0, 20.0, 20.0);
        let svg = c.finish();
        assert!(svg.contains("<image"), "got: {svg}");
        assert!(svg.contains("data:image/png;base64,"), "got: {svg}");
    }

    #[test]
    fn image_painter_accessor_returns_some() {
        let mut c = ctx();
        let dyn_ctx: &mut dyn RenderContext = &mut c;
        assert!(dyn_ctx.image_painter().is_some());
    }

    #[test]
    fn finish_produces_a_single_well_formed_svg_root() {
        let mut c = ctx();
        c.set_fill_color("#123456");
        c.fill_rect(0.0, 0.0, 1.0, 1.0);
        let svg = c.finish();
        assert_well_formed(&svg);
    }

    /// Tiny stack-scan XML sanity check (not a validating parser — `usvg`
    /// is heavy for a unit test) — every open tag has a matching close (or
    /// is self-closing), and the document has exactly one root.
    fn assert_well_formed(svg: &str) {
        let trimmed = svg.trim();
        assert!(trimmed.starts_with("<svg"));
        assert!(trimmed.ends_with("</svg>"));
        let mut stack: Vec<&str> = Vec::new();
        let mut idx = 0usize;
        while let Some(rel_start) = svg[idx..].find('<') {
            let start = idx + rel_start;
            let Some(rel_end) = svg[start..].find('>') else { break };
            let end = start + rel_end;
            let tag = &svg[start + 1..end];
            idx = end + 1;
            if let Some(name) = tag.strip_prefix('/') {
                let name = name.split_whitespace().next().unwrap_or("");
                let top = stack.pop().unwrap_or("");
                assert_eq!(top, name, "mismatched closing tag </{name}>");
            } else if !tag.ends_with('/') {
                let name = tag.split_whitespace().next().unwrap_or("");
                stack.push(name);
            }
        }
        assert!(stack.is_empty(), "unbalanced tags left open: {stack:?}");
    }
}
