//! Shared rendering state for vello backends.
//!
//! [`VelloContextCore`] holds all mutable drawing state (colors, stroke
//! settings, font, transform stack, etc.) that every vello-family backend
//! needs.  Backends embed this struct and delegate the pure state-update
//! methods to it; they then read the state from it when they need to emit
//! actual vello/scene calls.

use vello::kurbo::{Affine, Cap, Join};
use vello::peniko::color::palette;

use uzor_core::render::{TextAlign, TextBaseline};

use crate::color::{parse_color, Color};
use crate::font::{parse_css_font, FontInfo};

// ---------------------------------------------------------------------------
// SavedState
// ---------------------------------------------------------------------------

/// A snapshot of the full rendering state, pushed onto the state stack by
/// [`VelloContextCore::save`] and popped by [`VelloContextCore::restore`].
#[derive(Clone)]
pub struct SavedState {
    pub transform: Affine,
    pub stroke_color: Color,
    pub stroke_width: f64,
    pub fill_color: Color,
    pub line_dash: Vec<f64>,
    pub line_cap: Cap,
    pub line_join: Join,
    pub global_alpha: f64,
    pub font_info: FontInfo,
    pub text_align: TextAlign,
    pub text_baseline: TextBaseline,
    /// Whether a clip layer was pushed while this save level was active.
    pub has_clip: bool,
}

// ---------------------------------------------------------------------------
// VelloContextCore
// ---------------------------------------------------------------------------

/// The shared rendering state embedded in every vello backend context.
///
/// All methods are pure state updates — they do not touch any vello `Scene`.
/// Backends read the fields directly (or via the accessor methods) to feed
/// values into scene calls.
pub struct VelloContextCore {
    // Transform
    pub transform: Affine,

    // Stroke styling
    pub stroke_color: Color,
    pub stroke_width: f64,
    pub line_dash: Vec<f64>,
    pub line_cap: Cap,
    pub line_join: Join,

    // Fill styling
    pub fill_color: Color,

    // Transparency
    pub global_alpha: f64,

    // Font / text
    pub font_info: FontInfo,
    pub text_align: TextAlign,
    pub text_baseline: TextBaseline,

    // State stack
    state_stack: Vec<SavedState>,
}

impl VelloContextCore {
    /// Create a new `VelloContextCore` with default state.
    ///
    /// `offset_x` and `offset_y` set an initial translation (typically the
    /// chart rect origin).
    pub fn new(offset_x: f64, offset_y: f64) -> Self {
        Self {
            transform: Affine::translate((offset_x, offset_y)),
            stroke_color: palette::css::WHITE,
            stroke_width: 1.0,
            line_dash: Vec::new(),
            line_cap: Cap::Butt,
            line_join: Join::Miter,
            fill_color: palette::css::TRANSPARENT,
            global_alpha: 1.0,
            font_info: FontInfo::default(),
            text_align: TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            state_stack: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Stroke / fill state setters (mirror uzor-render RenderContext trait)
    // -----------------------------------------------------------------------

    /// Set the stroke colour from a CSS color string.
    pub fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = parse_color(color);
    }

    /// Set the stroke width in logical pixels.
    pub fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width;
    }

    /// Set the dash pattern for stroked paths.
    pub fn set_line_dash(&mut self, pattern: &[f64]) {
        self.line_dash = pattern.to_vec();
    }

    /// Set the line cap style (`"round"`, `"square"`, or butt by default).
    pub fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round" => Cap::Round,
            "square" => Cap::Square,
            _ => Cap::Butt,
        };
    }

    /// Set the line join style (`"round"`, `"bevel"`, or miter by default).
    pub fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _ => Join::Miter,
        };
    }

    /// Set the fill colour from a CSS color string.
    pub fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }

    /// Set the global alpha multiplier (clamped to `[0.0, 1.0]`).
    pub fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    // -----------------------------------------------------------------------
    // Font / text setters
    // -----------------------------------------------------------------------

    /// Parse a CSS font shorthand string and update the active font state.
    ///
    /// Example: `"bold 14px Roboto"`.
    pub fn set_font(&mut self, font: &str) {
        self.font_info = parse_css_font(font);
    }

    /// Set the text alignment.
    pub fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    /// Set the text baseline.
    pub fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    // -----------------------------------------------------------------------
    // Transform helpers
    // -----------------------------------------------------------------------

    /// Translate the current transform by `(x, y)`.
    pub fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_translate((x, y).into());
    }

    /// Rotate the current transform by `angle` radians (counter-clockwise).
    pub fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.then_rotate(angle);
    }

    /// Scale the current transform non-uniformly.
    pub fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_scale_non_uniform(x, y);
    }

    // -----------------------------------------------------------------------
    // Effective color helpers
    // -----------------------------------------------------------------------

    /// Return the stroke color with `global_alpha` applied.
    pub fn effective_stroke_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.stroke_color.with_alpha(self.global_alpha as f32)
        } else {
            self.stroke_color
        }
    }

    /// Return the fill color with `global_alpha` applied.
    pub fn effective_fill_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.fill_color.with_alpha(self.global_alpha as f32)
        } else {
            self.fill_color
        }
    }

    // -----------------------------------------------------------------------
    // Save / restore
    // -----------------------------------------------------------------------

    /// Push the current state onto the save stack.
    ///
    /// `has_clip` should be passed as `true` if a clip layer is currently
    /// active in the caller's scene (i.e. `clip()` was called since the last
    /// `save()`).  Pass `false` if no clip is active.
    pub fn save(&mut self, has_clip: bool) {
        self.state_stack.push(SavedState {
            transform: self.transform,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            fill_color: self.fill_color,
            line_dash: self.line_dash.clone(),
            line_cap: self.line_cap,
            line_join: self.line_join,
            global_alpha: self.global_alpha,
            font_info: self.font_info.clone(),
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            has_clip,
        });
    }

    /// Pop the most recent saved state.
    ///
    /// Returns the popped [`SavedState`] so the caller can inspect `has_clip`
    /// and pop any clip layers from the scene accordingly.
    ///
    /// Returns `None` if the stack is empty.
    pub fn restore(&mut self) -> Option<SavedState> {
        let state = self.state_stack.pop()?;
        self.transform = state.transform;
        self.stroke_color = state.stroke_color;
        self.stroke_width = state.stroke_width;
        self.fill_color = state.fill_color;
        self.line_dash = state.line_dash.clone();
        self.line_cap = state.line_cap;
        self.line_join = state.line_join;
        self.global_alpha = state.global_alpha;
        self.font_info = state.font_info.clone();
        self.text_align = state.text_align;
        self.text_baseline = state.text_baseline;
        Some(state)
    }

    /// How many save levels are currently on the stack.
    pub fn save_depth(&self) -> usize {
        self.state_stack.len()
    }
}
