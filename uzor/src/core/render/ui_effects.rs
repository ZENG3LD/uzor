//! [`UiEffectHelpers`] — UI-effect convenience helpers (hover, active, glass).
//!
//! Composite methods for hover/active/glass patterns.
//! Default impls use solid colour only. Backends that support blur override
//! the blur-specific methods (`draw_blur_background`, `has_blur_background`,
//! `use_convex_glass_buttons`, `draw_glass_button_3d`) with real implementations.
//!
//! This trait lives in the `RenderContext` supertrait chain so all methods are
//! reachable via `&mut dyn RenderContext` — no downcasting required.

use super::shape_helpers::ShapeHelpers;

/// UI-effect convenience helpers — composite methods for hover/active/glass patterns.
///
/// All methods have default impls. Backends that support backdrop blur
/// override the blur-query and blur-draw methods.
///
/// Tessera widgets call these instead of branching on `has_blur_background()`.
pub trait UiEffectHelpers: ShapeHelpers {
    // =========================================================================
    // Blur queries + draw (no-op defaults; backends with real blur override)
    // =========================================================================

    /// Draw blurred background for UI elements (FrostedGlass/LiquidGlass effects).
    ///
    /// Default: no-op. Override in GPU backends that load a blur image.
    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let _ = (x, y, width, height);
    }

    /// Returns `true` when a blur image is loaded and ready for drawing.
    ///
    /// Default: `false`. Override in backends that support blur.
    fn has_blur_background(&self) -> bool {
        false
    }

    /// Returns `true` when blur is active AND convex glass button style is enabled.
    ///
    /// Default: `false`.
    fn use_convex_glass_buttons(&self) -> bool {
        false
    }

    /// Draw a 3D convex glass button effect.
    ///
    /// Default: blur background + fill_rounded_rect with `color`.
    /// GPU backends override with full specular/gradient/shadow 3D effect.
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
        self.draw_blur_background(x, y, width, height);
        self.set_fill_color(color);
        self.fill_rounded_rect(x, y, width, height, radius);
    }

    // =========================================================================
    // Hover / active states
    // =========================================================================

    /// Draw a hover state rectangle.
    ///
    /// Branches on blur/glass state: 3D glass → flat glass → solid.
    fn draw_hover_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: &str) {
        if self.use_convex_glass_buttons() {
            self.draw_glass_button_3d(x, y, width, height, 2.0, false, color);
        } else if self.has_blur_background() {
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        } else {
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        }
    }

    /// Draw an active/pressed state rectangle.
    fn draw_active_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: &str) {
        if self.use_convex_glass_buttons() {
            self.draw_glass_button_3d(x, y, width, height, 2.0, true, color);
        } else if self.has_blur_background() {
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        } else {
            self.set_fill_color(color);
            self.fill_rect(x, y, width, height);
        }
    }

    /// Draw a hover state rounded rectangle.
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
            self.draw_glass_button_3d(x, y, width, height, radius, false, color);
        } else if self.has_blur_background() {
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        } else {
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        }
    }

    /// Draw an active/pressed state rounded rectangle.
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
            self.draw_glass_button_3d(x, y, width, height, radius, true, color);
        } else if self.has_blur_background() {
            self.draw_blur_background(x, y, width, height);
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        } else {
            self.set_fill_color(color);
            self.fill_rounded_rect(x, y, width, height, radius);
        }
    }

    /// Draw a sidebar hover item with a vertical accent indicator on the left.
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
        self.set_fill_color(accent_color);
        self.fill_rect(x, y, indicator_width, height);
        self.draw_hover_rect(x + indicator_width, y, width - indicator_width, height, bg_color);
    }

    /// Draw a sidebar active item with a vertical accent indicator on the left.
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
        self.set_fill_color(accent_color);
        self.fill_rect(x, y, indicator_width, height);
        self.draw_active_rect(x + indicator_width, y, width - indicator_width, height, bg_color);
    }
}
