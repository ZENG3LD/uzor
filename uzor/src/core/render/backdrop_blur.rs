//! [`BackdropBlur`] — GPU-only opt-in backdrop blur capability.
//!
//! Backends that do not implement this trait are statically known to callers —
//! no silent no-op. Tessera widget paint functions that want blur check
//! capability via downcasting.

/// Backdrop blur — opt-in GPU capability.
///
/// Backends that do not implement this trait are statically known
/// to callers — no silent no-op.
///
/// [`VelloGpuRenderContext`] implements this; tiny-skia does NOT (yet).
pub trait BackdropBlur {
    /// Draw the blurred background texture clipped to the given rect.
    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64);

    /// Returns `true` when a blur image is loaded and ready for drawing.
    fn has_blur_background(&self) -> bool;

    /// Returns `true` when blur is active AND convex glass button style is enabled.
    fn use_convex_glass_buttons(&self) -> bool;

    /// Draw a 3D convex glass button.
    ///
    /// Default: blur background + [`fill_rounded_rect`](super::ShapeHelpers::fill_rounded_rect).
    /// [`VelloGpuRenderContext`] overrides with full specular/gradient/shadow 3D effect.
    #[allow(clippy::too_many_arguments)]
    fn draw_glass_button_3d(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        is_active: bool,
        color: &str,
    );
}
