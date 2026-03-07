//! `RenderContext` implementation for the instanced wgpu backend.
//!
//! `InstancedRenderContext` accumulates draw calls into `QuadInstance` /
//! `LineInstance` / `TextAreaData` vectors during a frame.  At the end of the
//! frame the caller passes these vectors to `InstancedRenderer::render()`.

use std::sync::OnceLock;

use skrifa::MetadataProvider;
use uzor_render::{RenderContext, RenderContextExt, TextAlign, TextBaseline};

use crate::instances::{LineInstance, QuadInstance};
use crate::text::TextAreaData;

// ── Embedded Roboto fonts ──────────────────────────────────────────────────
static ROBOTO_REGULAR: &[u8]     = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8]        = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8]      = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

/// Lazily-loaded skrifa font data (regular, bold, italic, bold-italic).
static FONT_REGULAR:     OnceLock<skrifa::FontRef<'static>> = OnceLock::new();
static FONT_BOLD:        OnceLock<skrifa::FontRef<'static>> = OnceLock::new();
static FONT_ITALIC:      OnceLock<skrifa::FontRef<'static>> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<skrifa::FontRef<'static>> = OnceLock::new();

fn get_font_ref(bold: bool, italic: bool) -> Option<&'static skrifa::FontRef<'static>> {
    use skrifa::raw::FileRef;
    let make = |data: &'static [u8]| -> Option<skrifa::FontRef<'static>> {
        match FileRef::new(data).ok()? {
            FileRef::Font(f) => Some(f),
            FileRef::Collection(c) => c.get(0).ok(),
        }
    };
    match (bold, italic) {
        (true, true)   => FONT_BOLD_ITALIC.get_or_init(|| make(ROBOTO_BOLD_ITALIC).unwrap()).into(),
        (true, false)  => FONT_BOLD.get_or_init(|| make(ROBOTO_BOLD).unwrap()).into(),
        (false, true)  => FONT_ITALIC.get_or_init(|| make(ROBOTO_ITALIC).unwrap()).into(),
        (false, false) => FONT_REGULAR.get_or_init(|| make(ROBOTO_REGULAR).unwrap()).into(),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Parse a CSS color string to `[f32; 4]` RGBA.
fn parse_color(color: &str) -> [f32; 4] {
    let color = color.trim();

    // rgba(r,g,b,a)
    if let Some(inner) = color.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let p: Vec<&str> = inner.split(',').collect();
        if p.len() == 4 {
            let r = p[0].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let g = p[1].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let b = p[2].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let a = p[3].trim().parse::<f32>().unwrap_or(1.0);
            return [r, g, b, a.min(1.0)];
        }
    }

    // rgb(r,g,b)
    if let Some(inner) = color.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let p: Vec<&str> = inner.split(',').collect();
        if p.len() == 3 {
            let r = p[0].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let g = p[1].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let b = p[2].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            return [r, g, b, 1.0];
        }
    }

    // Hex
    let hex = color.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
        }
        _ => match color {
            "transparent" => [0.0, 0.0, 0.0, 0.0],
            "white"       => [1.0, 1.0, 1.0, 1.0],
            "black"       => [0.0, 0.0, 0.0, 1.0],
            "red"         => [1.0, 0.0, 0.0, 1.0],
            "green"       => [0.0, 0.5, 0.0, 1.0],
            "blue"        => [0.0, 0.0, 1.0, 1.0],
            "yellow"      => [1.0, 1.0, 0.0, 1.0],
            "orange"      => [1.0, 0.647, 0.0, 1.0],
            "gray" | "grey" => [0.502, 0.502, 0.502, 1.0],
            _             => [0.0, 0.0, 0.0, 1.0],
        },
    }
}

// ── 2D affine transform helpers ────────────────────────────────────────────
// `transform` is stored as [a, b, c, d, tx, ty] corresponding to the matrix:
//   | a  b  tx |
//   | c  d  ty |
//   | 0  0   1 |

/// Apply transform to a point.
#[inline]
fn apply(t: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    (t[0] * x + t[1] * y + t[4], t[2] * x + t[3] * y + t[5])
}

/// Multiply two transforms (left * right).
#[inline]
fn mul(a: &[f32; 6], b: &[f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[0] * b[4] + a[1] * b[5] + a[4],
        a[2] * b[4] + a[3] * b[5] + a[5],
    ]
}

fn translate_transform(t: &[f32; 6], x: f32, y: f32) -> [f32; 6] {
    mul(t, &[1.0, 0.0, 0.0, 1.0, x, y])
}

fn rotate_transform(t: &[f32; 6], angle: f32) -> [f32; 6] {
    let (s, c) = angle.sin_cos();
    mul(t, &[c, -s, s, c, 0.0, 0.0])
}

fn scale_transform(t: &[f32; 6], sx: f32, sy: f32) -> [f32; 6] {
    mul(t, &[sx, 0.0, 0.0, sy, 0.0, 0.0])
}

// ── Path building ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    Close,
    Rect(f32, f32, f32, f32),
    Arc { cx: f32, cy: f32, r: f32, start: f32, end: f32 },
}

// ── Saved state for save/restore ───────────────────────────────────────────

#[derive(Clone)]
struct SavedState {
    transform: [f32; 6],
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
    stroke_width: f32,
    global_alpha: f64,
    font_size: f32,
    font_bold: bool,
    font_italic: bool,
    text_align: TextAlign,
    text_baseline: TextBaseline,
    clip_depth: usize,
}

// ── InstancedRenderContext ─────────────────────────────────────────────────

/// A `RenderContext` that batches draw calls into instance lists.
///
/// At the end of a frame, pass `quads`, `lines`, and `text_areas` to
/// `InstancedRenderer::render()`.
pub struct InstancedRenderContext {
    // Output buffers
    pub quads: Vec<QuadInstance>,
    pub lines: Vec<LineInstance>,
    pub text_areas: Vec<TextAreaData>,

    // Current transform (2D affine)
    transform: [f32; 6],

    // Style state
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
    stroke_width: f32,
    global_alpha: f64,
    font_size: f32,
    font_bold: bool,
    font_italic: bool,
    text_align: TextAlign,
    text_baseline: TextBaseline,

    // Clip stack: each entry is [x,y,w,h] in screen-space pixels
    clip_stack: Vec<[f32; 4]>,

    // Save/restore state stack
    state_stack: Vec<SavedState>,

    // Path builder
    path: Vec<PathCmd>,

    // Screen size for the "infinite" default clip rect
    screen_w: f32,
    screen_h: f32,
}

impl InstancedRenderContext {
    /// Create a new context for a frame.
    ///
    /// `offset_x/y` are applied as an initial translate transform (analogous to
    /// the chart rect offset used in the vello backend).
    pub fn new(screen_w: f32, screen_h: f32, offset_x: f32, offset_y: f32) -> Self {
        let transform = [1.0, 0.0, 0.0, 1.0, offset_x, offset_y];
        let root_clip = [0.0, 0.0, screen_w, screen_h];
        Self {
            quads: Vec::new(),
            lines: Vec::new(),
            text_areas: Vec::new(),
            transform,
            fill_color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: [1.0, 1.0, 1.0, 1.0],
            stroke_width: 1.0,
            global_alpha: 1.0,
            font_size: 12.0,
            font_bold: false,
            font_italic: false,
            text_align: TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            clip_stack: vec![root_clip],
            state_stack: Vec::new(),
            path: Vec::new(),
            screen_w,
            screen_h,
        }
    }

    /// Clear all accumulated draw calls (call at the start of each frame).
    pub fn clear(&mut self) {
        self.quads.clear();
        self.lines.clear();
        self.text_areas.clear();
    }

    // ── Internals ──────────────────────────────────────────────────────────

    fn current_clip(&self) -> [f32; 4] {
        self.clip_stack.last().copied().unwrap_or([0.0, 0.0, self.screen_w, self.screen_h])
    }

    /// Apply global alpha to a color.
    fn apply_alpha(&self, color: [f32; 4]) -> [f32; 4] {
        let a = (color[3] * self.global_alpha as f32).clamp(0.0, 1.0);
        [color[0], color[1], color[2], a]
    }

    /// Tessellate the current path into line instances (for `stroke()`).
    fn tessellate_stroke(&mut self) {
        let path = std::mem::take(&mut self.path);
        let color = self.apply_alpha(self.stroke_color);
        let clip = self.current_clip();
        let width = self.stroke_width;

        let mut cursor = (0.0f32, 0.0f32);
        let mut first  = (0.0f32, 0.0f32);
        let mut first_set = false;

        for cmd in &path {
            match *cmd {
                PathCmd::MoveTo(x, y) => {
                    let (px, py) = apply(&self.transform, x, y);
                    cursor = (px, py);
                    if !first_set { first = cursor; first_set = true; }
                }
                PathCmd::LineTo(x, y) => {
                    let (px, py) = apply(&self.transform, x, y);
                    self.lines.push(LineInstance {
                        start: [cursor.0, cursor.1],
                        end: [px, py],
                        color,
                        width,
                        _pad0: [0.0; 3],
                        clip_rect: clip,
                    });
                    cursor = (px, py);
                }
                PathCmd::Close => {
                    if first_set {
                        self.lines.push(LineInstance {
                            start: [cursor.0, cursor.1],
                            end: [first.0, first.1],
                            color,
                            width,
                            _pad0: [0.0; 3],
                            clip_rect: clip,
                        });
                        cursor = first;
                    }
                }
                PathCmd::Rect(x, y, w, h) => {
                    let corners = [
                        apply(&self.transform, x,     y),
                        apply(&self.transform, x + w, y),
                        apply(&self.transform, x + w, y + h),
                        apply(&self.transform, x,     y + h),
                    ];
                    for i in 0..4 {
                        let a = corners[i];
                        let b = corners[(i + 1) % 4];
                        self.lines.push(LineInstance {
                            start: [a.0, a.1],
                            end: [b.0, b.1],
                            color,
                            width,
                            _pad0: [0.0; 3],
                            clip_rect: clip,
                        });
                    }
                }
                PathCmd::Arc { cx, cy, r, start, end } => {
                    // Approximate arc with line segments
                    let steps = ((end - start).abs() * r).max(4.0) as usize;
                    let step = (end - start) / steps as f32;
                    let mut prev = apply(&self.transform, cx + r * start.cos(), cy + r * start.sin());
                    for i in 1..=steps {
                        let a = start + step * i as f32;
                        let next = apply(&self.transform, cx + r * a.cos(), cy + r * a.sin());
                        self.lines.push(LineInstance {
                            start: [prev.0, prev.1],
                            end: [next.0, next.1],
                            color,
                            width,
                            _pad0: [0.0; 3],
                            clip_rect: clip,
                        });
                        prev = next;
                    }
                }
            }
        }
    }

    /// Tessellate the current path into quad/line instances (for `fill()`).
    ///
    /// Simple cases (rect path) emit a `QuadInstance`.
    /// Other paths are approximated as a series of line segments (fill is not
    /// implemented for arbitrary paths in this backend — they fall back to stroke).
    fn tessellate_fill(&mut self) {
        // Check for the common rect pattern: MoveTo + 3×LineTo + Close (or Rect)
        let path = std::mem::take(&mut self.path);
        let color = self.apply_alpha(self.fill_color);
        let clip = self.current_clip();

        // Single Rect command
        if path.len() == 1 {
            if let PathCmd::Rect(x, y, w, h) = path[0] {
                let (px, py) = apply(&self.transform, x, y);
                // Scale only — use transform scale components to compute pixel size
                // This is a simplified approach: transform x/y corners to get size
                let (px2, py2) = apply(&self.transform, x + w, y + h);
                self.quads.push(QuadInstance {
                    pos: [px.min(px2), py.min(py2)],
                    size: [(px2 - px).abs(), (py2 - py).abs()],
                    color,
                    corner_radius: 0.0,
                    border_width: 0.0,
                    _pad0: [0.0; 2],
                    border_color: [0.0; 4],
                    clip_rect: clip,
                });
                return;
            }
        }

        // Detect MoveTo + 3 LineTo + Close  (rectangle pattern)
        if path.len() == 5 {
            let is_move  = matches!(path[0], PathCmd::MoveTo(..));
            let is_close = matches!(path[4], PathCmd::Close);
            let all_lines = matches!(path[1], PathCmd::LineTo(..))
                && matches!(path[2], PathCmd::LineTo(..))
                && matches!(path[3], PathCmd::LineTo(..));
            if is_move && is_close && all_lines {
                let (x0, y0) = if let PathCmd::MoveTo(a, b) = path[0] { (a, b) } else { unreachable!() };
                let (x2, y2) = if let PathCmd::LineTo(a, b) = path[2] { (a, b) } else { unreachable!() };
                let min_x = x0.min(x2);
                let min_y = y0.min(y2);
                let max_x = x0.max(x2);
                let max_y = y0.max(y2);
                let (px1, py1) = apply(&self.transform, min_x, min_y);
                let (px2, py2) = apply(&self.transform, max_x, max_y);
                self.quads.push(QuadInstance {
                    pos: [px1.min(px2), py1.min(py2)],
                    size: [(px2 - px1).abs(), (py2 - py1).abs()],
                    color,
                    corner_radius: 0.0,
                    border_width: 0.0,
                    _pad0: [0.0; 2],
                    border_color: [0.0; 4],
                    clip_rect: clip,
                });
                return;
            }
        }

        // Fallback: emit as stroked lines using fill color (approximate)
        // This handles arc paths (rounded_rect), polygon paths, etc.
        let old_color = self.stroke_color;
        let old_width = self.stroke_width;
        self.stroke_color = self.fill_color;
        self.stroke_width = 1.0;
        self.path = path;
        self.tessellate_stroke();
        self.stroke_color = old_color;
        self.stroke_width = old_width;
    }

    /// Compute the bounding box of the current path in screen-space pixels.
    fn path_bounding_box(&self) -> [f32; 4] {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        let mut update = |x: f32, y: f32| {
            let (px, py) = apply(&self.transform, x, y);
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        };

        for cmd in &self.path {
            match *cmd {
                PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => update(x, y),
                PathCmd::Close => {}
                PathCmd::Rect(x, y, w, h) => {
                    update(x, y);
                    update(x + w, y + h);
                }
                PathCmd::Arc { cx, cy, r, .. } => {
                    update(cx - r, cy - r);
                    update(cx + r, cy + r);
                }
            }
        }

        if min_x > max_x {
            return self.current_clip();
        }
        [min_x, min_y, max_x - min_x, max_y - min_y]
    }

    /// Intersect a rectangle [x,y,w,h] with the current clip.
    fn intersect_clip(&self, r: [f32; 4]) -> [f32; 4] {
        let c = self.current_clip();
        let x1 = r[0].max(c[0]);
        let y1 = r[1].max(c[1]);
        let x2 = (r[0] + r[2]).min(c[0] + c[2]);
        let y2 = (r[1] + r[3]).min(c[1] + c[3]);
        if x2 <= x1 || y2 <= y1 {
            [0.0; 4]
        } else {
            [x1, y1, x2 - x1, y2 - y1]
        }
    }

    /// Measure the pixel width of `text` using skrifa metrics.
    fn measure_text_internal(&self, text: &str) -> f32 {
        let Some(font_ref) = get_font_ref(self.font_bold, self.font_italic) else {
            return text.len() as f32 * self.font_size * 0.6;
        };
        let size = skrifa::instance::Size::new(self.font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let charmap = font_ref.charmap();
        let glyph_metrics = font_ref.glyph_metrics(size, var_loc);

        text.chars()
            .map(|ch| {
                let gid = charmap.map(ch).unwrap_or_default();
                glyph_metrics.advance_width(gid).unwrap_or_default()
            })
            .sum()
    }
}

// ── RenderContext impl ─────────────────────────────────────────────────────

impl RenderContext for InstancedRenderContext {
    fn dpr(&self) -> f64 { 1.0 }

    // ── Stroke style ────────────────────────────────────────────────────
    fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = parse_color(color);
    }
    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width as f32;
    }
    fn set_line_dash(&mut self, _pattern: &[f64]) {
        // Dash patterns are not supported in this backend (SDF lines are solid).
    }
    fn set_line_cap(&mut self, _cap: &str) {
        // Capsule SDF always has round caps.
    }
    fn set_line_join(&mut self, _join: &str) {
        // Not applicable for the SDF line primitive.
    }

    // ── Fill style ──────────────────────────────────────────────────────
    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }
    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    // ── Path operations ─────────────────────────────────────────────────
    fn begin_path(&mut self) {
        self.path.clear();
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.path.push(PathCmd::MoveTo(x as f32, y as f32));
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.path.push(PathCmd::LineTo(x as f32, y as f32));
    }
    fn close_path(&mut self) {
        self.path.push(PathCmd::Close);
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.path.push(PathCmd::Rect(x as f32, y as f32, w as f32, h as f32));
    }
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        self.path.push(PathCmd::Arc {
            cx: cx as f32,
            cy: cy as f32,
            r: radius as f32,
            start: start_angle as f32,
            end: end_angle as f32,
        });
    }
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, _ry: f64, _rotation: f64, start: f64, end: f64) {
        // Approximate as circle using rx
        self.arc(cx, cy, rx, start, end);
    }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        // Approximate with a single line segment
        let _ = (cpx, cpy);
        self.path.push(PathCmd::LineTo(x as f32, y as f32));
    }
    fn bezier_curve_to(
        &mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, x: f64, y: f64,
    ) {
        // Approximate with a single line segment
        self.path.push(PathCmd::LineTo(x as f32, y as f32));
    }

    // ── Stroke / fill / clip ────────────────────────────────────────────
    fn stroke(&mut self) {
        self.tessellate_stroke();
    }
    fn fill(&mut self) {
        self.tessellate_fill();
    }
    fn clip(&mut self) {
        let bb = self.path_bounding_box();
        let new_clip = self.intersect_clip(bb);
        self.clip_stack.push(new_clip);
        self.path.clear();
    }

    // ── Shape helpers ───────────────────────────────────────────────────
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let color = self.apply_alpha(self.stroke_color);
        let clip = self.current_clip();
        let width = self.stroke_width;
        let corners = [
            apply(&self.transform, x,     y),
            apply(&self.transform, x + w, y),
            apply(&self.transform, x + w, y + h),
            apply(&self.transform, x,     y + h),
        ];
        for i in 0..4 {
            let a = corners[i];
            let b = corners[(i + 1) % 4];
            self.lines.push(LineInstance {
                start: [a.0, a.1],
                end: [b.0, b.1],
                color,
                width,
                _pad0: [0.0; 3],
                clip_rect: clip,
            });
        }
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let color = self.apply_alpha(self.fill_color);
        let clip = self.current_clip();
        let (px1, py1) = apply(&self.transform, x,     y);
        let (px2, py2) = apply(&self.transform, x + w, y + h);
        self.quads.push(QuadInstance {
            pos: [px1.min(px2), py1.min(py2)],
            size: [(px2 - px1).abs(), (py2 - py1).abs()],
            color,
            corner_radius: 0.0,
            border_width: 0.0,
            _pad0: [0.0; 2],
            border_color: [0.0; 4],
            clip_rect: clip,
        });
    }

    // ── Text ─────────────────────────────────────────────────────────────
    fn set_font(&mut self, font: &str) {
        self.font_bold   = false;
        self.font_italic = false;
        for part in font.to_lowercase().split_whitespace() {
            if part.ends_with("px") {
                if let Ok(sz) = part.trim_end_matches("px").parse::<f32>() {
                    self.font_size = sz;
                }
            } else if part == "bold" {
                self.font_bold = true;
            } else if part == "italic" {
                self.font_italic = true;
            }
        }
    }
    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }
    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() { return; }
        let (px, py) = apply(&self.transform, x as f32, y as f32);
        let estimated_width  = self.measure_text_internal(text);
        let estimated_height = self.font_size;
        let clip = self.current_clip();
        let color = self.apply_alpha(self.fill_color);

        self.text_areas.push(TextAreaData {
            text: text.to_owned(),
            x: px,
            y: py,
            font_size: self.font_size,
            color,
            bold: self.font_bold,
            italic: self.font_italic,
            align: self.text_align,
            baseline: self.text_baseline,
            clip,
            estimated_width,
            estimated_height,
        });
    }

    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {
        // Not supported; text stroking is rarely needed.
    }

    fn measure_text(&self, text: &str) -> f64 {
        self.measure_text_internal(text) as f64
    }

    // ── Transform ────────────────────────────────────────────────────────
    fn save(&mut self) {
        let state = SavedState {
            transform: self.transform,
            fill_color: self.fill_color,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            global_alpha: self.global_alpha,
            font_size: self.font_size,
            font_bold: self.font_bold,
            font_italic: self.font_italic,
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            clip_depth: self.clip_stack.len(),
        };
        self.state_stack.push(state);
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.transform    = state.transform;
            self.fill_color   = state.fill_color;
            self.stroke_color = state.stroke_color;
            self.stroke_width = state.stroke_width;
            self.global_alpha = state.global_alpha;
            self.font_size    = state.font_size;
            self.font_bold    = state.font_bold;
            self.font_italic  = state.font_italic;
            self.text_align   = state.text_align;
            self.text_baseline = state.text_baseline;
            // Restore clip stack to the depth at save time
            self.clip_stack.truncate(state.clip_depth);
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = translate_transform(&self.transform, x as f32, y as f32);
    }
    fn rotate(&mut self, angle: f64) {
        self.transform = rotate_transform(&self.transform, angle as f32);
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.transform = scale_transform(&self.transform, x as f32, y as f32);
    }

    // ── Rounded rect fill (direct quad with corner_radius) ───────────────
    fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let r = (radius as f32).min(w / 2.0).min(h / 2.0).max(0.0);
        let color = self.apply_alpha(self.fill_color);
        let clip = self.current_clip();
        let (px1, py1) = apply(&self.transform, x,     y);
        let (px2, py2) = apply(&self.transform, x + w, y + h);
        // Corner radius in screen space (assumes uniform scale)
        let scale = ((self.transform[0].abs() + self.transform[3].abs()) * 0.5).max(0.001);
        self.quads.push(QuadInstance {
            pos: [px1.min(px2), py1.min(py2)],
            size: [(px2 - px1).abs(), (py2 - py1).abs()],
            color,
            corner_radius: r * scale,
            border_width: 0.0,
            _pad0: [0.0; 2],
            border_color: [0.0; 4],
            clip_rect: clip,
        });
    }

    fn stroke_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        // Approximate with the path-based default implementation
        self.begin_path();
        self.rounded_rect(x, y, w, h, radius);
        self.stroke();
    }
}

impl RenderContextExt for InstancedRenderContext {
    type BlurImage = ();
    // Blur is not supported in this backend — default no-ops are sufficient.
}
