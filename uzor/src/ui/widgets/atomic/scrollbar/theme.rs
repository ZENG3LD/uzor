//! Scrollbar colour palette.
//!
//! Two colour slots are scrollbar-specific:
//! - `thumb_normal`  / `thumb_hover`  / `thumb_active` — handle colours for each state
//! - `track_bg`      — semi-transparent track background drawn only when
//!   `ScrollbarStyle::draw_track_bg()` returns `true` (signal-group variant).
//!
//! Colour strings are `#rrggbb` hex.  Alpha is applied at draw time.

pub trait ScrollbarTheme {
    /// Handle colour when neither hovered nor dragging (Active state, 0.5 opacity).
    fn thumb_normal(&self) -> &str;
    /// Handle colour when the cursor is over the thumb (0.8 opacity).
    fn thumb_hover(&self) -> &str;
    /// Handle colour while dragging (0.8 opacity).
    fn thumb_active(&self) -> &str;
    /// Track background colour — used only when `ScrollbarStyle::draw_track_bg()`.
    /// Signal-group applies this at ~12 % opacity (`hex + "20"`).
    fn track_bg(&self) -> &str;
}

// ── Dark theme ────────────────────────────────────────────────────────────────

/// Default dark-theme scrollbar palette matching mlc `WidgetTheme::default()`.
///
/// - `thumb_normal`  → `text_disabled`  `#6a6d78`
/// - `thumb_hover`   → `text_normal`    `#d1d4dc`
/// - `thumb_active`  → `text_normal`    `#d1d4dc`
/// - `track_bg`      → separator        `#363a45`
#[derive(Default)]
pub struct DefaultScrollbarTheme;

impl ScrollbarTheme for DefaultScrollbarTheme {
    fn thumb_normal(&self) -> &str { "#6a6d78" }
    fn thumb_hover(&self)  -> &str { "#d1d4dc" }
    fn thumb_active(&self) -> &str { "#d1d4dc" }
    fn track_bg(&self)     -> &str { "#363a45" }
}

// ── Light theme ───────────────────────────────────────────────────────────────

/// Light-theme scrollbar palette matching mlc `WidgetTheme::light()`.
///
/// - `thumb_normal`  → `text_disabled`  `#9598a1`
/// - `thumb_hover`   → `text_normal`    `#131722`
/// - `thumb_active`  → `text_normal`    `#131722`
/// - `track_bg`      → separator light  `#c8cad0`
#[derive(Default)]
pub struct LightScrollbarTheme;

impl ScrollbarTheme for LightScrollbarTheme {
    fn thumb_normal(&self) -> &str { "#9598a1" }
    fn thumb_hover(&self)  -> &str { "#131722" }
    fn thumb_active(&self) -> &str { "#131722" }
    fn track_bg(&self)     -> &str { "#c8cad0" }
}
