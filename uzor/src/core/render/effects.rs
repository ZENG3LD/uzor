//! [`Effects`] — shadow and blend mode rendering effects.
//!
//! Also owns the [`BlendMode`] enum (moved here from `context.rs` per charter §4).

// =========================================================================
// BlendMode enum
// =========================================================================

/// Compositing blend mode for subsequent fill/stroke operations.
///
/// Matches CSS `mix-blend-mode` and Canvas2D `globalCompositeOperation`
/// semantics. Backends that cannot apply a mode natively fall back to
/// `Normal` (standard alpha compositing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// Standard alpha compositing (source-over). Default.
    #[default]
    Normal,
    /// Multiply the source and destination colours.
    Multiply,
    /// Invert both colours, multiply them, then invert the result.
    Screen,
    /// Multiply or screen depending on which is darker.
    Overlay,
    /// Keep the darkest of source and destination.
    Darken,
    /// Keep the lightest of source and destination.
    Lighten,
    /// Brighten the destination to reflect the source.
    ColorDodge,
    /// Darken the destination to reflect the source.
    ColorBurn,
    /// Multiply or screen depending on the source value.
    HardLight,
    /// Like `HardLight` but softer.
    SoftLight,
    /// Subtract the darker of the two colours from the lighter.
    Difference,
    /// Difference but with lower contrast.
    Exclusion,
    /// Add source and destination (clamped to 1.0).
    Plus,
}

// =========================================================================
// Effects trait
// =========================================================================

/// Rendering effects — shadow and blend mode.
///
/// Backends without native support provide default no-ops.
/// These are NOT opt-in traits because their absence is visually tolerable
/// (unlike [`BackdropBlur`](super::BackdropBlur) which silently corrupts UX).
pub trait Effects {
    /// Set a drop shadow that applies to subsequent fill/stroke calls.
    ///
    /// `dx`/`dy` — shadow offset in canvas-space pixels.
    /// `blur` — Gaussian blur radius (0 = hard shadow).
    /// `color` — CSS color string (typically with alpha, e.g. `"#000000aa"`).
    ///
    /// Backends without native shadow support provide a no-op default.
    fn set_shadow(&mut self, dx: f64, dy: f64, blur: f64, color: &str) {
        let _ = (dx, dy, blur, color);
    }

    /// Clear the active shadow, restoring plain (no-shadow) rendering.
    ///
    /// No-op default for backends without native shadow support.
    fn clear_shadow(&mut self) {}

    /// Set the blend mode for subsequent fill/stroke calls.
    ///
    /// The mode is applied until another `set_blend_mode` call or a
    /// `restore` that crosses a `save` boundary (backend-dependent;
    /// wrap in `save`/`restore` for scoped blending).
    ///
    /// Backends without native blend mode support silently ignore the call
    /// (equivalent to keeping [`BlendMode::Normal`]).
    fn set_blend_mode(&mut self, mode: BlendMode) {
        let _ = mode;
    }
}
