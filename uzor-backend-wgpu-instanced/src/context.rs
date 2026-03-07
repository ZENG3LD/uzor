//! `RenderContext` implementation for the instanced wgpu backend.
//!
//! `InstancedRenderContext` accumulates draw calls into `QuadInstance` /
//! `TriangleInstance` / `TextAreaData` vectors during a frame.  At the end of the
//! frame the caller passes these vectors to `InstancedRenderer::render()`.

use std::sync::OnceLock;

use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    StrokeTessellator, StrokeOptions, StrokeVertex, LineCap, LineJoin,
};
use lyon_path::math::point;
use skrifa::MetadataProvider;
use uzor_render::{RenderContext, RenderContextExt, TextAlign, TextBaseline};

use crate::instances::{DrawCmd, QuadInstance, TriangleInstance};
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

/// Approximate float equality (within 0.5px tolerance for rect detection).
#[inline]
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

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
    line_cap: String,
    line_join: String,
}

// ── InstancedRenderContext ─────────────────────────────────────────────────

/// A `RenderContext` that batches draw calls into a unified draw command list.
///
/// At the end of a frame, pass `draw_commands` to `InstancedRenderer::render()`.
/// The renderer processes them in order, preserving painter's z-order while
/// batching consecutive same-type commands into efficient GPU draw calls.
pub struct InstancedRenderContext {
    // Unified draw command list — preserves painter's order.
    pub draw_commands: Vec<DrawCmd>,

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
    line_cap: String,
    line_join: String,

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
            draw_commands: Vec::new(),
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
            line_cap: String::new(),
            line_join: String::new(),
            clip_stack: vec![root_clip],
            state_stack: Vec::new(),
            path: Vec::new(),
            screen_w,
            screen_h,
        }
    }

    /// Clear all accumulated draw calls (call at the start of each frame).
    pub fn clear(&mut self) {
        self.draw_commands.clear();
    }

    // ── Internals ──────────────────────────────────────────────────────────

    fn current_clip(&self) -> [f32; 4] {
        self.clip_stack.last().copied().unwrap_or([0.0, 0.0, self.screen_w, self.screen_h])
    }

    /// Get the last point in the current path (for bezier tessellation).
    fn last_path_point(&self) -> (f32, f32) {
        for cmd in self.path.iter().rev() {
            match *cmd {
                PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => return (x, y),
                PathCmd::Rect(x, y, w, h) => return (x + w, y + h),
                PathCmd::Arc { cx, cy, r, end, .. } => return (cx + r * end.cos(), cy + r * end.sin()),
                PathCmd::Close => continue,
            }
        }
        (0.0, 0.0)
    }

    /// Apply global alpha to a color.
    fn apply_alpha(&self, color: [f32; 4]) -> [f32; 4] {
        let a = (color[3] * self.global_alpha as f32).clamp(0.0, 1.0);
        [color[0], color[1], color[2], a]
    }

    /// Tessellate the current path into triangle instances (for `stroke()`).
    ///
    /// Uses lyon `StrokeTessellator` to produce proper stroke geometry with
    /// correct joins and caps — identical in principle to how Vello handles strokes.
    /// Open subpaths remain open; only `PathCmd::Close` closes a subpath.
    fn tessellate_stroke(&mut self) {
        let path = std::mem::take(&mut self.path);
        let color = self.apply_alpha(self.stroke_color);
        let clip = self.current_clip();

        // Map line cap / join string settings to lyon enums.
        let cap = match self.line_cap.as_str() {
            "round"  => LineCap::Round,
            "square" => LineCap::Square,
            _        => LineCap::Butt,
        };
        let join = match self.line_join.as_str() {
            "round" => LineJoin::Round,
            "bevel" => LineJoin::Bevel,
            _       => LineJoin::Miter,
        };

        // Build a lyon Path from PathCmd list.
        // Open subpaths end with builder.end(false); only PathCmd::Close calls builder.close().
        let mut builder = lyon_path::Path::builder();
        let transform = self.transform;
        let mut in_subpath = false;

        for cmd in &path {
            match *cmd {
                PathCmd::MoveTo(x, y) => {
                    if in_subpath {
                        builder.end(false);
                    }
                    let (px, py) = apply(&transform, x, y);
                    builder.begin(point(px, py));
                    in_subpath = true;
                }
                PathCmd::LineTo(x, y) => {
                    if !in_subpath {
                        let (px, py) = apply(&transform, x, y);
                        builder.begin(point(px, py));
                        in_subpath = true;
                    } else {
                        let (px, py) = apply(&transform, x, y);
                        builder.line_to(point(px, py));
                    }
                }
                PathCmd::Close => {
                    if in_subpath {
                        builder.close();
                        in_subpath = false;
                    }
                }
                PathCmd::Rect(x, y, w, h) => {
                    // Rect is a closed subpath.
                    if in_subpath {
                        builder.end(false);
                    }
                    let tl = apply(&transform, x,     y);
                    let tr = apply(&transform, x + w, y);
                    let br = apply(&transform, x + w, y + h);
                    let bl = apply(&transform, x,     y + h);
                    builder.begin(point(tl.0, tl.1));
                    builder.line_to(point(tr.0, tr.1));
                    builder.line_to(point(br.0, br.1));
                    builder.line_to(point(bl.0, bl.1));
                    builder.close();
                    in_subpath = false;
                }
                PathCmd::Arc { cx, cy, r, start, end } => {
                    let scale = ((transform[0].abs() + transform[3].abs()) * 0.5).max(1.0);
                    let screen_r = r * scale;

                    if screen_r < 0.5 {
                        // Zero-radius arc — advance cursor to center.
                        let center = apply(&transform, cx, cy);
                        if in_subpath {
                            builder.line_to(point(center.0, center.1));
                        } else {
                            builder.begin(point(center.0, center.1));
                            in_subpath = true;
                        }
                        continue;
                    }

                    let sweep = end - start;
                    let steps = ((sweep.abs() * screen_r).max(sweep.abs() * 5.0)).max(8.0) as usize;
                    let step_angle = sweep / steps as f32;
                    let first_pt = apply(&transform, cx + r * start.cos(), cy + r * start.sin());

                    if in_subpath {
                        builder.line_to(point(first_pt.0, first_pt.1));
                    } else {
                        builder.begin(point(first_pt.0, first_pt.1));
                        in_subpath = true;
                    }

                    for i in 1..=steps {
                        let angle = start + step_angle * i as f32;
                        let p = apply(&transform, cx + r * angle.cos(), cy + r * angle.sin());
                        builder.line_to(point(p.0, p.1));
                    }
                }
            }
        }

        // Close any remaining open subpath (end without closing).
        if in_subpath {
            builder.end(false);
        }

        let lyon_path = builder.build();

        // Tessellate the stroke using lyon StrokeTessellator.
        // The path coordinates are already in screen space (transformed via apply()),
        // so stroke_width is used as-is (logical pixels match screen pixels at scale 1,
        // and callers that pre-scale their stroke_width — e.g. svg.rs — pass screen pixels).
        let mut geometry: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
        let mut tessellator = StrokeTessellator::new();
        let options = StrokeOptions::tolerance(0.5)
            .with_line_width(self.stroke_width)
            .with_line_cap(cap)
            .with_line_join(join);

        let result = tessellator.tessellate_path(
            &lyon_path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                vertex.position().to_array()
            }),
        );

        if result.is_ok() && !geometry.indices.is_empty() {
            for tri in geometry.indices.chunks(3) {
                if tri.len() == 3 {
                    let v0 = geometry.vertices[tri[0] as usize];
                    let v1 = geometry.vertices[tri[1] as usize];
                    let v2 = geometry.vertices[tri[2] as usize];
                    self.draw_commands.push(DrawCmd::Triangle(TriangleInstance {
                        v0,
                        v1,
                        v2,
                        _pad0: [0.0; 2],
                        color,
                        clip_rect: clip,
                    }));
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
                self.draw_commands.push(DrawCmd::Quad(QuadInstance {
                    pos: [px.min(px2), py.min(py2)],
                    size: [(px2 - px).abs(), (py2 - py).abs()],
                    color,
                    corner_radius: 0.0,
                    border_width: 0.0,
                    _pad0: [0.0; 2],
                    border_color: [0.0; 4],
                    clip_rect: clip,
                }));
                return;
            }
        }

        // Detect MoveTo + 3 LineTo + Close  (axis-aligned rectangle pattern)
        if path.len() == 5 {
            let is_move  = matches!(path[0], PathCmd::MoveTo(..));
            let is_close = matches!(path[4], PathCmd::Close);
            let all_lines = matches!(path[1], PathCmd::LineTo(..))
                && matches!(path[2], PathCmd::LineTo(..))
                && matches!(path[3], PathCmd::LineTo(..));
            if is_move && is_close && all_lines {
                let (x0, y0) = if let PathCmd::MoveTo(a, b) = path[0] { (a, b) } else { unreachable!() };
                let (x1, y1) = if let PathCmd::LineTo(a, b) = path[1] { (a, b) } else { unreachable!() };
                let (x2, y2) = if let PathCmd::LineTo(a, b) = path[2] { (a, b) } else { unreachable!() };
                let (x3, y3) = if let PathCmd::LineTo(a, b) = path[3] { (a, b) } else { unreachable!() };

                // Only use the quad fast-path if the 4 vertices form an axis-aligned rectangle.
                // Trapezoids and other 4-point polygons must fall through to lyon tessellation.
                let is_aabb = (approx_eq(x0, x3) && approx_eq(x1, x2) && approx_eq(y0, y1) && approx_eq(y2, y3))
                           || (approx_eq(y0, y1) && approx_eq(y2, y3) && approx_eq(x0, x3) && approx_eq(x1, x2))
                           || (approx_eq(x0, x1) && approx_eq(x2, x3) && approx_eq(y1, y2) && approx_eq(y0, y3))
                           || (approx_eq(y0, y3) && approx_eq(y1, y2) && approx_eq(x0, x1) && approx_eq(x2, x3));

                if is_aabb {
                    let min_x = x0.min(x1).min(x2).min(x3);
                    let min_y = y0.min(y1).min(y2).min(y3);
                    let max_x = x0.max(x1).max(x2).max(x3);
                    let max_y = y0.max(y1).max(y2).max(y3);
                    let (px1, py1) = apply(&self.transform, min_x, min_y);
                    let (px2, py2) = apply(&self.transform, max_x, max_y);
                    self.draw_commands.push(DrawCmd::Quad(QuadInstance {
                        pos: [px1.min(px2), py1.min(py2)],
                        size: [(px2 - px1).abs(), (py2 - py1).abs()],
                        color,
                        corner_radius: 0.0,
                        border_width: 0.0,
                        _pad0: [0.0; 2],
                        border_color: [0.0; 4],
                        clip_rect: clip,
                    }));
                    return;
                }
                // Non-rect 4-point polygon: fall through to lyon tessellation below.
            }
        }

        // Fallback: fill for arbitrary closed paths using lyon tessellation.
        // This correctly fills any path — concave polygons, SVG shapes, area fills, circles, arcs.

        let mut builder = lyon_path::Path::builder();
        let transform = &self.transform;
        let mut in_subpath = false;

        for cmd in &path {
            match *cmd {
                PathCmd::MoveTo(x, y) => {
                    if in_subpath {
                        builder.end(false);
                    }
                    let (px, py) = apply(transform, x, y);
                    builder.begin(point(px, py));
                    in_subpath = true;
                }
                PathCmd::LineTo(x, y) => {
                    if !in_subpath {
                        let (px, py) = apply(transform, x, y);
                        builder.begin(point(px, py));
                        in_subpath = true;
                    } else {
                        let (px, py) = apply(transform, x, y);
                        builder.line_to(point(px, py));
                    }
                }
                PathCmd::Close => {
                    if in_subpath {
                        builder.close();
                        in_subpath = false;
                    }
                }
                PathCmd::Rect(x, y, w, h) => {
                    if in_subpath {
                        builder.end(false);
                    }
                    let tl = apply(transform, x, y);
                    let tr = apply(transform, x + w, y);
                    let br = apply(transform, x + w, y + h);
                    let bl = apply(transform, x, y + h);
                    builder.begin(point(tl.0, tl.1));
                    builder.line_to(point(tr.0, tr.1));
                    builder.line_to(point(br.0, br.1));
                    builder.line_to(point(bl.0, bl.1));
                    builder.close();
                    in_subpath = false;
                }
                PathCmd::Arc { cx, cy, r, start, end } => {
                    let sweep = end - start;
                    // Use screen-space radius for step count
                    let scale = ((transform[0].abs() + transform[3].abs()) * 0.5).max(1.0);
                    let screen_r = r * scale;

                    // Bug 1 fix: skip zero-radius arcs to avoid degenerate zero-length
                    // line segments that produce rendering artifacts.
                    if screen_r < 0.5 {
                        // Advance the subpath cursor to the center point
                        let center = apply(transform, cx, cy);
                        if in_subpath {
                            builder.line_to(point(center.0, center.1));
                        } else {
                            builder.begin(point(center.0, center.1));
                            in_subpath = true;
                        }
                        continue;
                    }

                    let steps = ((sweep.abs() * screen_r).max(sweep.abs() * 5.0)).max(8.0) as usize;
                    let step_angle = sweep / steps as f32;
                    let first_pt = apply(transform, cx + r * start.cos(), cy + r * start.sin());
                    if in_subpath {
                        builder.line_to(point(first_pt.0, first_pt.1));
                    } else {
                        builder.begin(point(first_pt.0, first_pt.1));
                        in_subpath = true;
                    }
                    for i in 1..=steps {
                        let angle = start + step_angle * i as f32;
                        let p = apply(transform, cx + r * angle.cos(), cy + r * angle.sin());
                        builder.line_to(point(p.0, p.1));
                    }
                }
            }
        }

        // Close any remaining open subpath
        if in_subpath {
            builder.close();
        }

        let lyon_path = builder.build();

        // Tessellate
        let mut geometry: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
        let mut tessellator = FillTessellator::new();
        let result = tessellator.tessellate_path(
            &lyon_path,
            &FillOptions::tolerance(0.5),
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                vertex.position().to_array()
            }),
        );

        if result.is_ok() && !geometry.indices.is_empty() {
            for tri in geometry.indices.chunks(3) {
                if tri.len() == 3 {
                    let v0 = geometry.vertices[tri[0] as usize];
                    let v1 = geometry.vertices[tri[1] as usize];
                    let v2 = geometry.vertices[tri[2] as usize];
                    self.draw_commands.push(DrawCmd::Triangle(TriangleInstance {
                        v0,
                        v1,
                        v2,
                        _pad0: [0.0; 2],
                        color,
                        clip_rect: clip,
                    }));
                }
            }
        }
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
    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = cap.to_owned();
    }
    fn set_line_join(&mut self, join: &str) {
        self.line_join = join.to_owned();
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
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rotation: f64, start: f64, end: f64) {
        if (rx - ry).abs() < 0.001 && rotation.abs() < 0.001 {
            // Circle — delegate to arc
            self.arc(cx, cy, rx, start, end);
            return;
        }
        // Tessellate ellipse into line segments using parametric form.
        // First point uses MoveTo (matches arc() behavior via PathCmd::Arc),
        // rest use LineTo.
        let sweep = end - start;
        let steps = ((sweep.abs() * rx.max(ry)).max(sweep.abs() * 5.0)).max(16.0) as usize;
        let (sin_rot, cos_rot) = rotation.sin_cos();
        for i in 0..=steps {
            let t = start + sweep * (i as f64 / steps as f64);
            let ex = rx * t.cos();
            let ey = ry * t.sin();
            let px = cx + ex * cos_rot - ey * sin_rot;
            let py = cy + ex * sin_rot + ey * cos_rot;
            if i == 0 {
                // Use LineTo if we're already in a subpath, MoveTo otherwise
                if self.path.is_empty() || matches!(self.path.last(), Some(PathCmd::Close)) {
                    self.path.push(PathCmd::MoveTo(px as f32, py as f32));
                } else {
                    self.path.push(PathCmd::LineTo(px as f32, py as f32));
                }
            } else {
                self.path.push(PathCmd::LineTo(px as f32, py as f32));
            }
        }
    }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        // Tessellate quadratic bezier into line segments.
        let (p0x, p0y) = self.last_path_point();
        let (cpx, cpy, ex, ey) = (cpx as f32, cpy as f32, x as f32, y as f32);
        const N: usize = 8;
        for i in 1..=N {
            let t = i as f32 / N as f32;
            let u = 1.0 - t;
            let px = u * u * p0x + 2.0 * u * t * cpx + t * t * ex;
            let py = u * u * p0y + 2.0 * u * t * cpy + t * t * ey;
            self.path.push(PathCmd::LineTo(px, py));
        }
    }
    fn bezier_curve_to(
        &mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64,
    ) {
        // Tessellate cubic bezier into line segments.
        let (p0x, p0y) = self.last_path_point();
        let (c1x, c1y) = (cp1x as f32, cp1y as f32);
        let (c2x, c2y) = (cp2x as f32, cp2y as f32);
        let (ex, ey) = (x as f32, y as f32);
        const N: usize = 8;
        for i in 1..=N {
            let t = i as f32 / N as f32;
            let u = 1.0 - t;
            let px = u*u*u*p0x + 3.0*u*u*t*c1x + 3.0*u*t*t*c2x + t*t*t*ex;
            let py = u*u*u*p0y + 3.0*u*u*t*c1y + 3.0*u*t*t*c2y + t*t*t*ey;
            self.path.push(PathCmd::LineTo(px, py));
        }
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
        // Delegate to stroke_rounded_rect with r=0: emits a single SDF quad
        // with border_width, which has no corner-joint dot artifacts.
        self.stroke_rounded_rect(x, y, w, h, 0.0);
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let color = self.apply_alpha(self.fill_color);
        let clip = self.current_clip();
        let (px1, py1) = apply(&self.transform, x,     y);
        let (px2, py2) = apply(&self.transform, x + w, y + h);
        self.draw_commands.push(DrawCmd::Quad(QuadInstance {
            pos: [px1.min(px2), py1.min(py2)],
            size: [(px2 - px1).abs(), (py2 - py1).abs()],
            color,
            corner_radius: 0.0,
            border_width: 0.0,
            _pad0: [0.0; 2],
            border_color: [0.0; 4],
            clip_rect: clip,
        }));
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

        self.draw_commands.push(DrawCmd::Text(TextAreaData {
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
        }));
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
            line_cap: self.line_cap.clone(),
            line_join: self.line_join.clone(),
        };
        self.state_stack.push(state);
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.transform     = state.transform;
            self.fill_color    = state.fill_color;
            self.stroke_color  = state.stroke_color;
            self.stroke_width  = state.stroke_width;
            self.global_alpha  = state.global_alpha;
            self.font_size     = state.font_size;
            self.font_bold     = state.font_bold;
            self.font_italic   = state.font_italic;
            self.text_align    = state.text_align;
            self.text_baseline = state.text_baseline;
            self.line_cap      = state.line_cap;
            self.line_join     = state.line_join;
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
        self.draw_commands.push(DrawCmd::Quad(QuadInstance {
            pos: [px1.min(px2), py1.min(py2)],
            size: [(px2 - px1).abs(), (py2 - py1).abs()],
            color,
            corner_radius: r * scale,
            border_width: 0.0,
            _pad0: [0.0; 2],
            border_color: [0.0; 4],
            clip_rect: clip,
        }));
    }

    fn stroke_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        // Bug 3 fix: emit a direct QuadInstance with border_width instead of using
        // the path-based fallback. This avoids the zero-radius arc artifacts entirely
        // for the common case and is GPU-efficient (single SDF quad vs many line segments).
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let r = (radius as f32).min(w / 2.0).min(h / 2.0).max(0.0);
        let border_color = self.apply_alpha(self.stroke_color);
        let clip = self.current_clip();
        let (px1, py1) = apply(&self.transform, x,     y);
        let (px2, py2) = apply(&self.transform, x + w, y + h);
        // Scale logical radius and stroke width to screen pixels (assumes uniform scale).
        let scale = ((self.transform[0].abs() + self.transform[3].abs()) * 0.5).max(0.001);
        self.draw_commands.push(DrawCmd::Quad(QuadInstance {
            pos: [px1.min(px2), py1.min(py2)],
            size: [(px2 - px1).abs(), (py2 - py1).abs()],
            // Transparent fill — only the border is visible.
            color: [0.0, 0.0, 0.0, 0.0],
            corner_radius: r * scale,
            border_width: self.stroke_width,
            _pad0: [0.0; 2],
            border_color,
            clip_rect: clip,
        }));
    }
}

impl RenderContextExt for InstancedRenderContext {
    type BlurImage = ();
    // Blur is not supported in this backend — default no-ops are sufficient.
}
