//! Platform-agnostic rendering context trait
//!
//! This module provides the `RenderContext` trait that abstracts away
//! platform-specific rendering.
//!
//! Applications implement this trait to provide rendering capabilities.

use super::types::{TextAlign, TextBaseline};

/// Platform-agnostic rendering context
///
/// Applications implement this trait to provide rendering capabilities
/// to primitives, charts, and widgets.
pub trait RenderContext {
    // =========================================================================
    // Dimensions
    // =========================================================================

    /// Device pixel ratio for crisp rendering
    fn dpr(&self) -> f64;

    // =========================================================================
    // Stroke Style
    // =========================================================================

    /// Set stroke color (hex string like "#RRGGBB" or "#RRGGBBAA")
    fn set_stroke_color(&mut self, color: &str);

    /// Set stroke width in pixels
    fn set_stroke_width(&mut self, width: f64);

    /// Set line dash pattern (empty for solid)
    fn set_line_dash(&mut self, pattern: &[f64]);

    /// Set line cap style ("butt", "round", "square")
    fn set_line_cap(&mut self, cap: &str);

    /// Set line join style ("miter", "round", "bevel")
    fn set_line_join(&mut self, join: &str);

    // =========================================================================
    // Fill Style
    // =========================================================================

    /// Set fill color (hex string)
    fn set_fill_color(&mut self, color: &str);

    /// Set fill color with alpha transparency
    /// Default implementation uses set_fill_color + set_global_alpha
    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        self.set_fill_color(color);
        self.set_global_alpha(alpha.clamp(0.0, 1.0));
    }

    /// Set global alpha (transparency)
    fn set_global_alpha(&mut self, alpha: f64);

    /// Reset global alpha to 1.0
    fn reset_alpha(&mut self) {
        self.set_global_alpha(1.0);
    }

    // =========================================================================
    // Path Operations
    // =========================================================================

    /// Begin a new path
    fn begin_path(&mut self);

    /// Move to point (without drawing)
    fn move_to(&mut self, x: f64, y: f64);

    /// Draw line to point
    fn line_to(&mut self, x: f64, y: f64);

    /// Close the current path
    fn close_path(&mut self);

    /// Add rectangle to current path (without stroking or filling)
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Draw arc (for partial circles)
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64);

    /// Draw ellipse (center, radii, rotation, start_angle, end_angle)
    #[allow(clippy::too_many_arguments)]
    fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
    );

    /// Quadratic bezier curve
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64);

    /// Cubic bezier curve
    fn bezier_curve_to(
        &mut self,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    );

    // =========================================================================
    // Stroke/Fill Operations
    // =========================================================================

    /// Stroke the current path
    fn stroke(&mut self);

    /// Fill the current path
    fn fill(&mut self);

    /// Clip to the current path
    fn clip(&mut self);

    /// Set a clipping rectangle. All subsequent drawing operations will be clipped to this rect.
    /// This is a convenience method that creates a rect path and applies it as a clip.
    /// Must be called within save/restore to limit the clip scope.
    fn clip_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.begin_path();
        self.rect(x, y, width, height);
        self.clip();
    }

    // =========================================================================
    // Shape Helpers (convenience methods)
    // =========================================================================

    /// Stroke a rectangle
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Fill a rectangle
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Fill a rounded rectangle (convenience method with default impl)
    fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        self.begin_path();
        self.rounded_rect(x, y, w, h, radius);
        self.fill();
    }

    /// Stroke a rounded rectangle
    fn stroke_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        self.begin_path();
        self.rounded_rect(x, y, w, h, radius);
        self.stroke();
    }

    /// Add rounded rectangle to path (default impl using arcs)
    fn rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        let r = r.min(w / 2.0).min(h / 2.0);
        self.move_to(x + r, y);
        self.line_to(x + w - r, y);
        self.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        self.line_to(x + w, y + h - r);
        self.arc(
            x + w - r,
            y + h - r,
            r,
            0.0,
            std::f64::consts::FRAC_PI_2,
        );
        self.line_to(x + r, y + h);
        self.arc(
            x + r,
            y + h - r,
            r,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
        );
        self.line_to(x, y + r);
        self.arc(
            x + r,
            y + r,
            r,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
        );
        // Close the path back to the start point so stroke/fill form a complete shape.
        self.close_path();
    }

    // =========================================================================
    // Text Rendering
    // =========================================================================

    /// Set font (CSS-style: "14px sans-serif" or "bold 16px monospace")
    fn set_font(&mut self, font: &str);

    /// Set text horizontal alignment
    fn set_text_align(&mut self, align: TextAlign);

    /// Set text vertical baseline
    fn set_text_baseline(&mut self, baseline: TextBaseline);

    /// Fill text at position
    fn fill_text(&mut self, text: &str, x: f64, y: f64);

    /// Stroke text at position
    fn stroke_text(&mut self, text: &str, x: f64, y: f64);

    /// Measure text width
    fn measure_text(&self, text: &str) -> f64;

    /// Fill text with rotation around the anchor point.
    /// Default implementation uses save/translate/rotate/fill_text/restore.
    fn fill_text_rotated(&mut self, text: &str, x: f64, y: f64, angle: f64) {
        if angle.abs() < 0.001 {
            self.fill_text(text, x, y);
        } else {
            self.save();
            self.translate(x, y);
            self.rotate(angle);
            self.fill_text(text, 0.0, 0.0);
            self.restore();
        }
    }

    /// Fill text centered at position.
    /// Default implementation sets alignment and baseline then calls fill_text.
    fn fill_text_centered(&mut self, text: &str, x: f64, y: f64) {
        self.set_text_align(TextAlign::Center);
        self.set_text_baseline(TextBaseline::Middle);
        self.fill_text(text, x, y);
    }

    // =========================================================================
    // Transform Operations
    // =========================================================================

    /// Save current state (transforms, styles)
    fn save(&mut self);

    /// Restore previously saved state
    fn restore(&mut self);

    /// Translate origin
    fn translate(&mut self, x: f64, y: f64);

    /// Rotate around origin
    fn rotate(&mut self, angle: f64);

    /// Scale from origin
    fn scale(&mut self, x: f64, y: f64);

    // =========================================================================
    // Images
    // =========================================================================

    /// Draw an image at the specified position
    ///
    /// # Arguments
    /// * `image_id` - Unique identifier for the cached image (URL or data URI)
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions to draw the image
    ///
    /// Returns true if the image was drawn, false if not yet loaded/cached.
    fn draw_image(&mut self, image_id: &str, x: f64, y: f64, width: f64, height: f64) -> bool {
        let _ = (image_id, x, y, width, height);
        false
    }

    /// Draw raw RGBA pixel data as an image
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, row-major, top-to-bottom)
    /// * `img_width`, `img_height` - Source image dimensions in pixels
    /// * `x`, `y` - Top-left corner position on canvas
    /// * `width`, `height` - Target dimensions to draw (stretches/shrinks to fit)
    ///
    /// Default implementation does nothing. Override in platform-specific contexts.
    #[allow(clippy::too_many_arguments)]
    fn draw_image_rgba(
        &mut self,
        data: &[u8],
        img_width: u32,
        img_height: u32,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        let _ = (data, img_width, img_height, x, y, width, height);
    }

    // =========================================================================
    // Blur Background (FrostedGlass/LiquidGlass effects)
    // =========================================================================

    /// Draw blurred background for UI elements (FrostedGlass/LiquidGlass effects)
    ///
    /// When a blur-enabled style is active, this draws a clipped portion of the
    /// blurred chart texture as background for UI elements like toolbars, sidebars, modals.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the blur region
    ///
    /// Default implementation does nothing. Override in platform-specific contexts
    /// that support blur effects (e.g., VelloGpuRenderContext).
    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let _ = (x, y, width, height);
    }

    /// Check if blur background is available
    ///
    /// Returns true if blur image is set and ready for drawing.
    /// Use this to conditionally draw blur backgrounds only when supported.
    fn has_blur_background(&self) -> bool {
        false
    }

    /// Check if 3D convex glass buttons should be used
    ///
    /// Returns true if blur is active AND convex glass button style is enabled.
    fn use_convex_glass_buttons(&self) -> bool {
        false
    }

    // =========================================================================
    // UI State Rectangles (Hover/Active)
    // =========================================================================

    /// Draw a hover state rectangle
    ///
    /// Centralized rendering for hover states on UI elements.
    /// For FrostedGlass/LiquidGlass with Convex3D style, uses 3D glass button effect.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the rectangle
    /// * `color` - Fill color (should be styled via theme.hover_bg_styled())
    fn draw_hover_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: &str) {
        if self.use_convex_glass_buttons() {
            // 3D convex glass button effect with theme color
            self.draw_glass_button_3d(x, y, width, height, 2.0, false, color);
        } else if self.has_blur_background() {
            // Flat glass: blur + color overlay
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        } else {
            // Solid: just color
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        }
    }

    /// Draw an active state rectangle
    ///
    /// Centralized rendering for active/pressed states on UI elements.
    /// For FrostedGlass/LiquidGlass with Convex3D style, uses 3D glass button effect.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the rectangle
    /// * `color` - Fill color (should be styled via theme.active_bg_styled())
    fn draw_active_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: &str) {
        if self.use_convex_glass_buttons() {
            // 3D convex glass button effect (pressed) with theme color
            self.draw_glass_button_3d(x, y, width, height, 2.0, true, color);
        } else if self.has_blur_background() {
            // Flat glass: blur + color overlay
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        } else {
            // Solid: just color
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        }
    }

    /// Draw a hover state rounded rectangle
    ///
    /// Centralized rendering for hover states on toolbar buttons.
    /// For FrostedGlass/LiquidGlass with Convex3D style, uses 3D glass button effect.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the rectangle
    /// * `radius` - Corner radius
    /// * `color` - Fill color (should be styled via theme.hover_bg_styled())
    fn draw_hover_rounded_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        color: &str,
    ) {
        if self.use_convex_glass_buttons() {
            // 3D convex glass button effect with theme color
            self.draw_glass_button_3d(x, y, width, height, radius, false, color);
        } else if self.has_blur_background() {
            // Flat glass: blur + color overlay
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        } else {
            // Solid: just color
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        }
    }

    /// Draw an active state rounded rectangle
    ///
    /// Centralized rendering for active/pressed states on toolbar buttons.
    /// For FrostedGlass/LiquidGlass with Convex3D style, uses 3D glass button effect.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the rectangle
    /// * `radius` - Corner radius
    /// * `color` - Fill color (should be styled via theme.active_bg_styled())
    fn draw_active_rounded_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        color: &str,
    ) {
        if self.use_convex_glass_buttons() {
            // 3D convex glass button effect (pressed) with theme color
            self.draw_glass_button_3d(x, y, width, height, radius, true, color);
        } else if self.has_blur_background() {
            // Flat glass: blur + color overlay
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        } else {
            // Solid: just color
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        }
    }

    /// Draw a sidebar hover item with vertical accent indicator
    ///
    /// Centralized rendering for hovered sidebar tabs/items.
    /// Draws a vertical accent bar on the left + hover background.
    /// Used for vertical toolbars in FrostedGlass/LiquidGlass styles.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the item
    /// * `accent_color` - Color for the left accent bar (theme.colors.accent)
    /// * `bg_color` - Background color (theme styled hover color)
    /// * `indicator_width` - Width of the accent bar (typically 3-4px)
    #[allow(clippy::too_many_arguments)]
    fn draw_sidebar_hover_item(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        accent_color: &str,
        bg_color: &str,
        indicator_width: f64,
    ) {
        // Draw accent indicator bar on the left
        self.set_fill_color(accent_color);
        self.fill_rect(x, y, indicator_width, height);

        // Draw hover background (with blur for Glass styles)
        self.draw_hover_rect(x + indicator_width, y, width - indicator_width, height, bg_color);
    }

    /// Draw a sidebar active item with vertical accent indicator
    ///
    /// Centralized rendering for active sidebar tabs/items.
    /// Draws a vertical accent bar on the left + active background.
    /// Used in modal sidebars (Chart Settings, Indicator Settings, Add Indicator, etc.)
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Dimensions of the item
    /// * `accent_color` - Color for the left accent bar (theme.colors.accent)
    /// * `bg_color` - Background color (theme styled active color)
    /// * `indicator_width` - Width of the accent bar (typically 3-4px)
    #[allow(clippy::too_many_arguments)]
    fn draw_sidebar_active_item(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        accent_color: &str,
        bg_color: &str,
        indicator_width: f64,
    ) {
        // Draw accent indicator bar on the left
        self.set_fill_color(accent_color);
        self.fill_rect(x, y, indicator_width, height);

        // Draw active background (with blur for Glass styles)
        self.draw_active_rect(x + indicator_width, y, width - indicator_width, height, bg_color);
    }

    // =========================================================================
    // 3D Glass Button Effects (FrostedGlass/LiquidGlass only)
    // =========================================================================

    /// Draw a 3D convex glass button effect
    ///
    /// Creates iOS-style raised glass button with:
    /// - Blur background (backdrop)
    /// - Theme color overlay (hover/active color from theme)
    /// - Convex bulge effect (lighter top, darker bottom)
    /// - Specular highlight (white stripe at top)
    /// - Inner shadow (depth at edges)
    /// - Fresnel rim lighting (subtle edge glow)
    ///
    /// Only has visual effect when blur background is available (Glass styles).
    /// Falls back to simple hover/active rendering otherwise.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner position
    /// * `width`, `height` - Button dimensions
    /// * `radius` - Corner radius
    /// * `is_active` - true for pressed state (flattened bulge), false for hover
    /// * `color` - Theme color for the button (hover_bg or active_bg)
    #[allow(clippy::too_many_arguments)]
    fn draw_glass_button_3d(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        _is_active: bool,
        color: &str,
    ) {
        // Default implementation: just draw blur background + color
        // VelloGpuRenderContext overrides this with full 3D effect
        self.draw_blur_background(x, y, width, height);
        self.set_fill_color(color);
        self.fill_rounded_rect(x, y, width, height, radius);
    }

}

/// Extension trait for platform-specific blur features
///
/// Provides type-safe blur image management for RenderContext implementations.
pub trait RenderContextExt: RenderContext {
    type BlurImage: Clone;
    fn set_blur_image(&mut self, _image: Option<Self::BlurImage>, _width: u32, _height: u32) {}
    fn set_use_convex_glass_buttons(&mut self, _use_convex: bool) {}
}
