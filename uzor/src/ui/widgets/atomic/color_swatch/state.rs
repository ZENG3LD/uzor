//! ColorSwatch persistent state.

/// Persistent state for a color swatch widget.
///
/// Most color swatches are stateless per-frame; the `selected` (picker open)
/// and `hovered` flags are passed directly in `ColorSwatchView` each frame.
/// This struct is reserved for future state (e.g. animation timers).
#[derive(Debug, Default, Clone)]
pub struct ColorSwatchState;
