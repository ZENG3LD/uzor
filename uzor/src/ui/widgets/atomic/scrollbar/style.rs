//! Scrollbar geometry presets.
//!
//! Three presets matching the three inline render variants found in mlc:
//! - `StandardScrollbarStyle`  — 8 px column, 30 px min thumb, r=4 (sidebar, modals)
//! - `CompactScrollbarStyle`   — 4 px column, 24 px min thumb, r=2 (profile-manager)
//! - `SignalScrollbarStyle`    — 6 px column, 16 px min thumb, r=0, draws track bg

/// Geometry contract every scrollbar style must satisfy.
pub trait ScrollbarStyle {
    /// Width of the scrollbar column allocated in the layout (pixels).
    fn track_thickness(&self) -> f64;
    /// Minimum rendered thumb length (pixels).
    fn thumb_min_length(&self) -> f64;
    /// Corner radius applied to the thumb rect.  0 = flat rect.
    fn thumb_radius(&self) -> f64;
    /// Inset from the outer rect edges to the rendered track area.
    fn track_padding(&self) -> f64;
    /// Whether to draw a semi-transparent track background before the thumb.
    /// Only `SignalScrollbarStyle` returns `true`.
    fn draw_track_bg(&self) -> bool;
}

// ── Standard (sidebar, indicator-settings, user-settings, scrollable-container) ──

/// 8 px / 30 px min / r=4 / no track bg.  Opacity-gated by `ScrollbarState`.
pub struct StandardScrollbarStyle;

impl Default for StandardScrollbarStyle {
    fn default() -> Self {
        Self
    }
}

impl ScrollbarStyle for StandardScrollbarStyle {
    fn track_thickness(&self)  -> f64 { 8.0 }
    fn thumb_min_length(&self) -> f64 { 30.0 }
    fn thumb_radius(&self)     -> f64 { 4.0 }
    fn track_padding(&self)    -> f64 { 2.0 }
    fn draw_track_bg(&self)    -> bool { false }
}

// ── Compact (profile-manager) ─────────────────────────────────────────────────

/// 4 px / 24 px min / r=2 / no opacity gating / no track bg.
pub struct CompactScrollbarStyle;

impl Default for CompactScrollbarStyle {
    fn default() -> Self {
        Self
    }
}

impl ScrollbarStyle for CompactScrollbarStyle {
    fn track_thickness(&self)  -> f64 { 4.0 }
    fn thumb_min_length(&self) -> f64 { 24.0 }
    fn thumb_radius(&self)     -> f64 { 2.0 }
    fn track_padding(&self)    -> f64 { 0.0 }
    fn draw_track_bg(&self)    -> bool { false }
}

// ── Signal (signal-group sidebar panel) ──────────────────────────────────────

/// 6 px / 16 px min / r=0 (flat rect) / draws track background.
pub struct SignalScrollbarStyle;

impl Default for SignalScrollbarStyle {
    fn default() -> Self {
        Self
    }
}

impl ScrollbarStyle for SignalScrollbarStyle {
    fn track_thickness(&self)  -> f64 { 6.0 }
    fn thumb_min_length(&self) -> f64 { 16.0 }
    fn thumb_radius(&self)     -> f64 { 0.0 }
    fn track_padding(&self)    -> f64 { 0.0 }
    fn draw_track_bg(&self)    -> bool { true }
}

// ── Backward-compatible alias ─────────────────────────────────────────────────

/// Default style — same as `StandardScrollbarStyle`.
pub type DefaultScrollbarStyle = StandardScrollbarStyle;
