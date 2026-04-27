//! Container geometry presets — one struct per `ContainerType` variant.
//!
//! All numeric defaults are sourced directly from mlc call sites (see research).

// ---------------------------------------------------------------------------
// Shared trait
// ---------------------------------------------------------------------------

/// Geometry parameters shared across all container variants.
pub trait ContainerStyle {
    fn radius(&self) -> f64;
    fn padding(&self) -> f64;
    fn border_width(&self) -> f64;
    fn shadow_offset(&self) -> (f64, f64);
    fn shadow_alpha(&self) -> f64;
}

// ---------------------------------------------------------------------------
// Plain — fill_rect, no border, radius=0
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Plain`.
///
/// mlc trading panels: outermost `fill_rect`, zero radius, no border, no padding.
pub struct PlainContainerStyle;

impl Default for PlainContainerStyle {
    fn default() -> Self {
        Self
    }
}

impl ContainerStyle for PlainContainerStyle {
    fn radius(&self) -> f64 {
        0.0
    }
    fn padding(&self) -> f64 {
        0.0
    }
    fn border_width(&self) -> f64 {
        0.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Bordered — fill_rounded_rect + 1px stroke
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Bordered`.
///
/// mlc dropdowns: `radius=4.0`, `border_width=1.0`, no shadow.
pub struct BorderedContainerStyle {
    /// Corner radius. mlc dropdowns: `4.0`.
    pub radius: f64,
    /// Inner content padding.
    pub padding: f64,
}

impl Default for BorderedContainerStyle {
    fn default() -> Self {
        Self {
            radius: 4.0,
            padding: 0.0,
        }
    }
}

impl ContainerStyle for BorderedContainerStyle {
    fn radius(&self) -> f64 {
        self.radius
    }
    fn padding(&self) -> f64 {
        self.padding
    }
    fn border_width(&self) -> f64 {
        1.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Card — bg + drop shadow + rounded corners (no blur at atomic level)
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Card`.
///
/// Shadow is a semi-transparent filled rect drawn before the background.
/// mlc popup defaults: `radius=4.0`, shadow offset `(2.0, 4.0)`, padding `8.0`.
pub struct CardContainerStyle {
    /// Corner radius. mlc popup default: `4.0`; modal default: `8.0`.
    pub radius: f64,
    /// Inner content padding. mlc popup: `8.0`.
    pub padding: f64,
    /// Shadow rect offset `(dx, dy)`. mlc popup: `(2.0, 4.0)`.
    pub shadow_offset: (f64, f64),
    /// Shadow fill alpha. mlc uses `rgba(0,0,0,0.4)` — stored separately in theme.
    pub shadow_alpha: f64,
}

impl Default for CardContainerStyle {
    fn default() -> Self {
        Self {
            radius: 4.0,
            padding: 8.0,
            shadow_offset: (2.0, 4.0),
            shadow_alpha: 0.4,
        }
    }
}

impl ContainerStyle for CardContainerStyle {
    fn radius(&self) -> f64 {
        self.radius
    }
    fn padding(&self) -> f64 {
        self.padding
    }
    fn border_width(&self) -> f64 {
        1.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        self.shadow_offset
    }
    fn shadow_alpha(&self) -> f64 {
        self.shadow_alpha
    }
}

// ---------------------------------------------------------------------------
// Clipping — clip_rect wraps children
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Clip`.
///
/// mlc `ScrollableContainer`: no background drawn by container itself; clipping
/// region is `viewport.width - scrollbar_width` (8.0 default). This style carries
/// no geometry beyond what the caller passes as `rect`.
pub struct ClippingContainerStyle {
    /// When `true`, `draw_clipping_container` emits `ctx.save()` + `clip_rect`.
    /// Set to `false` to skip clipping (useful for layout-only nesting).
    pub clipping: bool,
}

impl Default for ClippingContainerStyle {
    fn default() -> Self {
        Self { clipping: true }
    }
}

impl ContainerStyle for ClippingContainerStyle {
    fn radius(&self) -> f64 {
        0.0
    }
    fn padding(&self) -> f64 {
        0.0
    }
    fn border_width(&self) -> f64 {
        0.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Section — header strip + body
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Section`.
///
/// mlc panels: header strip is a second `fill_rect` on top of `panel_bg`.
/// Heights vary per panel (named constants, not a global). Default `24.0`.
pub struct SectionContainerStyle {
    /// Height of the header strip. mlc uses panel-private constants; `24.0` is a
    /// reasonable default for compact panels.
    pub header_height: f64,
    /// Inner padding for body content below the header.
    pub body_padding: f64,
}

impl Default for SectionContainerStyle {
    fn default() -> Self {
        Self {
            header_height: 24.0,
            body_padding: 0.0,
        }
    }
}

impl ContainerStyle for SectionContainerStyle {
    fn radius(&self) -> f64 {
        0.0
    }
    fn padding(&self) -> f64 {
        self.body_padding
    }
    fn border_width(&self) -> f64 {
        1.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Panel — toolbar / sidebar / status-bar
// ---------------------------------------------------------------------------

/// Style for `ContainerType::Panel`.
///
/// mlc toolbar: full-width flat rect, no radius, no shadow.
/// mlc sidebar settings panels: no radius, optional separator border.
pub struct PanelContainerStyle {
    /// Inner padding. mlc modal default: `16.0`; toolbar: `0.0`.
    pub padding: f64,
}

impl Default for PanelContainerStyle {
    fn default() -> Self {
        Self { padding: 0.0 }
    }
}

impl ContainerStyle for PanelContainerStyle {
    fn radius(&self) -> f64 {
        0.0
    }
    fn padding(&self) -> f64 {
        self.padding
    }
    fn border_width(&self) -> f64 {
        1.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Legacy default (kept for callers that use DefaultContainerStyle by name)
// ---------------------------------------------------------------------------

/// Backward-compatible generic style.
///
/// Prefer one of the typed presets above for new call sites.
pub struct DefaultContainerStyle;

impl Default for DefaultContainerStyle {
    fn default() -> Self {
        Self
    }
}

impl ContainerStyle for DefaultContainerStyle {
    fn radius(&self) -> f64 {
        4.0
    }
    fn padding(&self) -> f64 {
        8.0
    }
    fn border_width(&self) -> f64 {
        1.0
    }
    fn shadow_offset(&self) -> (f64, f64) {
        (0.0, 2.0)
    }
    fn shadow_alpha(&self) -> f64 {
        0.25
    }
}
