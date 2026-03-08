use super::context::RenderContext;
use super::types::TextAlign;

/// Render instructions that can be serialized
#[derive(Clone, Debug)]
pub enum RenderOp {
    // Style
    SetStrokeColor(String),
    SetFillColor(String),
    SetLineWidth(f64),
    SetLineDash(Vec<f64>),

    // Path
    BeginPath,
    MoveTo(f64, f64),
    LineTo(f64, f64),
    QuadraticCurveTo(f64, f64, f64, f64),
    BezierCurveTo(f64, f64, f64, f64, f64, f64),
    Arc(f64, f64, f64, f64, f64),
    Ellipse(f64, f64, f64, f64, f64, f64, f64),
    ClosePath,

    // Draw
    Stroke,
    Fill,
    StrokeRect(f64, f64, f64, f64),
    FillRect(f64, f64, f64, f64),

    // Text
    SetFont(String),
    SetTextAlign(TextAlign),
    FillText(String, f64, f64),
    StrokeText(String, f64, f64),

    // State
    Save,
    Restore,
    Translate(f64, f64),
    Rotate(f64),
    Scale(f64, f64),
    Clip,
}

/// Collection of render operations
pub type RenderOps = Vec<RenderOp>;

/// Execute render operations on a context
pub fn execute_ops(ctx: &mut dyn RenderContext, ops: &[RenderOp]) {
    for op in ops {
        match op {
            RenderOp::SetStrokeColor(c) => ctx.set_stroke_color(c),
            RenderOp::SetFillColor(c) => ctx.set_fill_color(c),
            RenderOp::SetLineWidth(w) => ctx.set_stroke_width(*w),
            RenderOp::SetLineDash(p) => ctx.set_line_dash(p),
            RenderOp::BeginPath => ctx.begin_path(),
            RenderOp::MoveTo(x, y) => ctx.move_to(*x, *y),
            RenderOp::LineTo(x, y) => ctx.line_to(*x, *y),
            RenderOp::QuadraticCurveTo(cpx, cpy, x, y) => {
                ctx.quadratic_curve_to(*cpx, *cpy, *x, *y)
            }
            RenderOp::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                ctx.bezier_curve_to(*cp1x, *cp1y, *cp2x, *cp2y, *x, *y)
            }
            RenderOp::Arc(cx, cy, r, start, end) => ctx.arc(*cx, *cy, *r, *start, *end),
            RenderOp::Ellipse(cx, cy, rx, ry, rot, start, end) => {
                ctx.ellipse(*cx, *cy, *rx, *ry, *rot, *start, *end)
            }
            RenderOp::ClosePath => ctx.close_path(),
            RenderOp::Stroke => ctx.stroke(),
            RenderOp::Fill => ctx.fill(),
            RenderOp::StrokeRect(x, y, w, h) => ctx.stroke_rect(*x, *y, *w, *h),
            RenderOp::FillRect(x, y, w, h) => ctx.fill_rect(*x, *y, *w, *h),
            RenderOp::SetFont(f) => ctx.set_font(f),
            RenderOp::SetTextAlign(a) => ctx.set_text_align(*a),
            RenderOp::FillText(t, x, y) => ctx.fill_text(t, *x, *y),
            RenderOp::StrokeText(t, x, y) => ctx.stroke_text(t, *x, *y),
            RenderOp::Save => ctx.save(),
            RenderOp::Restore => ctx.restore(),
            RenderOp::Translate(x, y) => ctx.translate(*x, *y),
            RenderOp::Rotate(a) => ctx.rotate(*a),
            RenderOp::Scale(x, y) => ctx.scale(*x, *y),
            RenderOp::Clip => ctx.clip(),
        }
    }
}
