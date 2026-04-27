//! Toast geometry constants — verbatim from mlc `render_toasts`.

/// Toast geometry constants, taken verbatim from mlc's `render_toasts`.
/// All values in logical pixels unless noted.
pub struct ToastGeometry;

impl ToastGeometry {
    /// Card width (px). mlc: 320.
    pub const TOAST_WIDTH: f64 = 320.0;

    /// Card height (px). mlc: 64.
    pub const TOAST_HEIGHT: f64 = 64.0;

    /// Text inset from the card edge (px). mlc: 12.
    pub const PADDING: f64 = 12.0;

    /// Gap between toasts AND between the stack and the window edge (px). mlc: 8.
    pub const MARGIN: f64 = 8.0;

    /// Border thickness (px). mlc: 1.
    pub const BORDER_THICKNESS: f64 = 1.0;

    /// Drop-shadow offset on both X and Y (px). mlc: 2.
    pub const SHADOW_OFFSET: f64 = 2.0;

    /// Y start of the first toast — accounts for the 32 px chrome strip + 8 px gap.
    /// mlc: `40.0 + margin = 48.0`. Stored as the raw `40.0` offset so callers can
    /// add MARGIN themselves if needed, but TOP_ANCHOR is the ready-to-use value.
    pub const CHROME_OFFSET: f64 = 40.0;

    /// Vertical position of the first toast top edge. mlc: 40.0 + 8.0 = 48.0.
    pub const TOP_ANCHOR: f64 = Self::CHROME_OFFSET + Self::MARGIN;

    /// Vertical distance between consecutive toast top edges (height + margin). mlc: 72.
    pub const STACK_PITCH: f64 = Self::TOAST_HEIGHT + Self::MARGIN;

    /// Title Y offset from toast top. mlc: padding + 6 = 18 px.
    pub const TITLE_Y_OFFSET: f64 = Self::PADDING + 6.0;

    /// Message Y offset from toast top. mlc: padding + 6 + 18 = 36 px.
    pub const MESSAGE_Y_OFFSET: f64 = Self::PADDING + 6.0 + 18.0;

    /// Fade starts when `remaining_fraction` drops below this value. mlc: 0.2.
    pub const FADE_THRESHOLD: f64 = 0.2;
}

// ─── Trait-based style interface (kept for callers that already use it) ───────

pub trait ToastStyle {
    fn padding(&self) -> f64;
    fn radius(&self) -> f64;
    fn icon_size(&self) -> f64;
    fn font_size(&self) -> f64;
    /// Kept for API compatibility; unused by the mlc-parity render path.
    fn fade_duration_ms(&self) -> u32;
}

pub struct DefaultToastStyle;

impl Default for DefaultToastStyle {
    fn default() -> Self {
        Self
    }
}

impl ToastStyle for DefaultToastStyle {
    fn padding(&self) -> f64 {
        ToastGeometry::PADDING
    }
    fn radius(&self) -> f64 {
        0.0
    }
    fn icon_size(&self) -> f64 {
        0.0
    }
    fn font_size(&self) -> f64 {
        11.0
    }
    /// Not used by the mlc-parity fade path. Kept for settings compatibility.
    fn fade_duration_ms(&self) -> u32 {
        0
    }
}
