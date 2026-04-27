//! Toggle geometry traits and presets.
//!
//! `ToggleSwitchStyle` controls track/thumb dimensions.
//! The two concrete presets match mlc sections 25 and 26.

/// Geometry parameters for toggle switch rendering.
///
/// The track is a pill (rounded rect with `border-radius = height / 2`).
/// The thumb is a filled circle positioned inside the track.
pub trait ToggleSwitchStyle {
    /// Track width in pixels.
    fn track_width(&self) -> f64;
    /// Track height in pixels. `border-radius = height / 2`.
    fn track_height(&self) -> f64;
    /// Thumb radius.
    fn thumb_radius(&self) -> f64;
    /// Gap between thumb edge and the nearest track edge (inner padding).
    fn thumb_padding(&self) -> f64;
    /// Gap between the toggle track right edge and an optional label.
    fn label_gap(&self) -> f64 {
        8.0
    }
}

/// Indicator-param toggle (section 25 — `indicator_settings.rs` Bool param).
///
/// mlc geometry: track 44×22, thumb radius 8.0, padding 4.0.
pub struct IndicatorToggleStyle;

impl ToggleSwitchStyle for IndicatorToggleStyle {
    fn track_width(&self)   -> f64 { 44.0 }
    fn track_height(&self)  -> f64 { 22.0 }
    fn thumb_radius(&self)  -> f64 { 8.0 }
    fn thumb_padding(&self) -> f64 { 4.0 }
}

/// Signals-enable toggle (section 26 — signals tab in `indicator_settings.rs`).
///
/// mlc geometry: track 44×22, thumb radius 9.0 (`height/2 - 2`), padding 2.0.
pub struct SignalsToggleStyle;

impl ToggleSwitchStyle for SignalsToggleStyle {
    fn track_width(&self)   -> f64 { 44.0 }
    fn track_height(&self)  -> f64 { 22.0 }
    fn thumb_radius(&self)  -> f64 { 9.0 }
    fn thumb_padding(&self) -> f64 { 2.0 }
}

/// Icon-swap toggle geometry — controls the icon bounding square.
pub trait ToggleIconStyle {
    /// Icon side length. mlc default: 16.0.
    fn icon_size(&self) -> f64;
}

/// Default icon-swap style matching mlc toolbar defaults.
pub struct DefaultToggleIconStyle;

impl ToggleIconStyle for DefaultToggleIconStyle {
    fn icon_size(&self) -> f64 { 16.0 }
}
