//! Shared scene encoding. All URX backends (wgpu, cpu, hybrid) consume
//! the same [`Scene`] of [`DrawCommand`]s in painter's order.
//!
//! Minimal vocabulary to start — YAGNI'd from research-04 §7. Add new
//! variants only when a real consumer needs them. New variants MUST be
//! implementable across all 3 backends or the variant doesn't ship.

use crate::math::{Affine, BezPath, Brush, Color, Rect, RoundedRect, Vec2};

/// Stroke parameters for line/path stroking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub width:     f32,
    pub miter_limit: f32,
    pub join:      LineJoin,
    pub cap:       LineCap,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            width: 1.0,
            miter_limit: 10.0,
            join: LineJoin::Miter,
            cap: LineCap::Butt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin { Miter, Round, Bevel }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap { Butt, Round, Square }

/// Opaque handle to a registered image. The URX engine maintains the
/// mapping from `ImageId` → backend texture (atlas slot on WGPU,
/// `Vec<u8>` pixmap on CPU, both on Hybrid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

/// One positioned glyph in a glyph run. Pre-shaped by cosmic-text on
/// the consumer side; the backend just rasterises + composites.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub glyph_id: u32,
    pub x:        f32,
    pub y:        f32,
}

/// Opaque handle to a registered font face. The consumer (or a higher
/// layer above URX) keeps the actual `skrifa::FontRef` / `cosmic_text::Font`
/// alive; URX just routes glyph rasterisation by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub u64);

/// Path winding-rule for fill operations. `NonZero` matches SVG/Canvas
/// default; `EvenOdd` flips fill state at each edge crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// Painter's-order draw command. Every backend walks `Scene::commands`
/// in this order and produces pixels.
///
/// **Vocabulary policy** (research-04 §7 + research-06 §"D6"):
/// minimal start, add primitives ONLY when a real consumer requires
/// them across all 3 backends. No "future-proof" enum variants.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Filled axis-aligned (or rotated via transform) rect, optional
    /// corner radii (per-corner). Brush = Solid / LinearGradient /
    /// RadialGradient (via `Brush` from peniko).
    FillRect {
        rect:    Rect,
        radii:   Option<[f32; 4]>,
        brush:   Brush,
        transform: Affine,
    },
    /// Stroked axis-aligned rect (matches FillRect parameters).
    StrokeRect {
        rect:    Rect,
        radii:   Option<[f32; 4]>,
        stroke:  Stroke,
        brush:   Brush,
        transform: Affine,
    },
    /// Single line segment with capsule SDF AA.
    Line {
        from:    Vec2,
        to:      Vec2,
        stroke:  Stroke,
        brush:   Brush,
        transform: Affine,
    },
    /// Filled arbitrary path (curves flattened on CPU per scanline,
    /// or via GPU tessellation on URX-WGPU). NonZero / EvenOdd
    /// winding rule.
    FillPath {
        path:      BezPath,
        rule:      FillRule,
        brush:     Brush,
        transform: Affine,
    },
    /// Stroked arbitrary path. Stroke width centered on the path;
    /// joins / caps from `Stroke`. Backend tessellates internally.
    StrokePath {
        path:      BezPath,
        stroke:    Stroke,
        brush:     Brush,
        transform: Affine,
    },
    /// Pre-shaped glyph run. Position is the run's origin; per-glyph
    /// (x, y) are relative offsets.
    GlyphRun {
        glyphs:    Vec<Glyph>,
        font:      FontId,
        font_size: f32,
        brush:     Brush,
        transform: Affine,
    },
    /// Image (texture) into destination rect with optional source crop.
    Image {
        src:       ImageId,
        src_rect:  Option<Rect>,
        dest:      Rect,
        transform: Affine,
    },
    /// Push an axis-aligned clip onto the stack. POP via `PopClip`.
    /// (Rectangular clip via scissor on WGPU, fast and free.)
    PushClipRect {
        rect:      Rect,
        transform: Affine,
    },
    /// Push a rounded-rect clip — implementable on WGPU via stencil
    /// pass or fragment discard; on CPU via path clip mask.
    PushClipRoundedRect {
        rect:      RoundedRect,
        transform: Affine,
    },
    PopClip,
}

/// A complete scene to render. Drained per-frame by the backend.
#[derive(Debug, Default, Clone)]
pub struct Scene {
    pub commands: Vec<DrawCommand>,
}

impl Scene {
    pub fn new() -> Self { Self::default() }

    pub fn reset(&mut self) {
        // Keep Vec capacity — backend hot path reuses allocation.
        self.commands.clear();
    }

    pub fn push(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    pub fn len(&self) -> usize { self.commands.len() }
    pub fn is_empty(&self) -> bool { self.commands.is_empty() }

    /// Convenience: solid-color FillRect with identity transform.
    pub fn fill_rect_solid(&mut self, r: Rect, color: Color) {
        self.commands.push(DrawCommand::FillRect {
            rect: r, radii: None,
            brush: Brush::Solid(color),
            transform: Affine::IDENTITY,
        });
    }

    /// Convenience: solid-color Line with identity transform.
    pub fn line_solid(&mut self, from: Vec2, to: Vec2, width: f32, color: Color) {
        let stroke = Stroke { width, ..Stroke::default() };
        self.commands.push(DrawCommand::Line {
            from, to, stroke,
            brush: Brush::Solid(color),
            transform: Affine::IDENTITY,
        });
    }

}
