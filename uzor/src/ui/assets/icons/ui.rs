//! UI icons — Feather/Lucide style, stroke-based.
//! Copied from mylittlechart icon collection.
//! viewBox: 0 0 24 24, fill="none", stroke="currentColor"

// =============================================================================
// Navigation Icons (Feather standard)
// =============================================================================

/// Home icon (Feather)
pub const ICON_HOME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>"##;

/// Server icon (Feather)
pub const ICON_SERVER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"/><rect x="2" y="14" width="20" height="8" rx="2" ry="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>"##;

/// File text / logs icon (Feather)
pub const ICON_FILE_TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>"##;

// =============================================================================
// Chart Type Icons
// =============================================================================

pub const ICON_CANDLESTICK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M7 4v3"/>
  <path d="M7 17v3"/>
  <rect x="5" y="7" width="4" height="10" rx="1"/>
  <path d="M17 6v2"/>
  <path d="M17 16v2"/>
  <rect x="15" y="8" width="4" height="8" rx="1" fill="currentColor"/>
</svg>"##;

pub const ICON_HOLLOW_CANDLES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M7 4v3"/>
  <path d="M7 17v3"/>
  <rect x="5" y="7" width="4" height="10" rx="1"/>
  <path d="M17 6v2"/>
  <path d="M17 16v2"/>
  <rect x="15" y="8" width="4" height="8" rx="1"/>
</svg>"##;

pub const ICON_HEIKIN_ASHI: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M5 4v4"/>
  <path d="M5 16v4"/>
  <rect x="3" y="8" width="4" height="8" rx="1" fill="currentColor"/>
  <path d="M12 3v3"/>
  <path d="M12 15v6"/>
  <rect x="10" y="6" width="4" height="9" rx="1" fill="currentColor"/>
  <path d="M19 5v5"/>
  <path d="M19 17v2"/>
  <rect x="17" y="10" width="4" height="7" rx="1" fill="currentColor"/>
</svg>"##;

pub const ICON_LINE_CHART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 17l6-6 4 4 8-8"/>
</svg>"##;

pub const ICON_AREA_CHART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 17l6-6 4 4 8-8v11H3z" fill="currentColor" fill-opacity="0.3"/>
  <path d="M3 17l6-6 4 4 8-8"/>
</svg>"##;

pub const ICON_BAR_CHART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M6 4v16"/>
  <path d="M4 8h2"/>
  <path d="M6 14h2"/>
  <path d="M12 6v12"/>
  <path d="M10 10h2"/>
  <path d="M12 14h2"/>
  <path d="M18 5v14"/>
  <path d="M16 8h2"/>
  <path d="M18 15h2"/>
</svg>"##;

pub const ICON_HISTOGRAM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="12" width="4" height="8" fill="currentColor"/>
  <rect x="10" y="8" width="4" height="12" fill="currentColor"/>
  <rect x="17" y="4" width="4" height="16" fill="currentColor"/>
</svg>"##;

pub const ICON_BASELINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12h20"/>
  <path d="M3 8l5 8 4-10 5 6 5-4"/>
  <path d="M3 12L3 8l5 8V12" fill="currentColor" fill-opacity="0.3" stroke="none"/>
  <path d="M13 12v-2l5 6v-4" fill="currentColor" fill-opacity="0.3" stroke="none"/>
</svg>"##;

pub const ICON_STEP_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 18h4v-4h4v-2h4v-4h4v-2h2"/>
</svg>"##;

pub const ICON_LINE_MARKERS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 17l6-6 4 4 8-8"/>
  <circle cx="3" cy="17" r="2" fill="currentColor"/>
  <circle cx="9" cy="11" r="2" fill="currentColor"/>
  <circle cx="13" cy="15" r="2" fill="currentColor"/>
  <circle cx="21" cy="7" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_HLC_AREA: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 8l5 2 5-4 5 3 3-2v10l-3 2-5-3-5 4-5-2z" fill="currentColor" fill-opacity="0.3"/>
  <path d="M3 12l5 1 5-2 5 2 3-1"/>
</svg>"##;

pub const ICON_COLUMNS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="10" width="3" height="10" fill="currentColor"/>
  <rect x="8" y="6" width="3" height="14" fill="currentColor"/>
  <rect x="13" y="14" width="3" height="6" fill="currentColor"/>
  <rect x="18" y="8" width="3" height="12" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Drawing Tool Icons
// =============================================================================

pub const ICON_TREND_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L20 4"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
  <circle cx="20" cy="4" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_HORIZONTAL_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 12h18"/>
  <circle cx="3" cy="12" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_VERTICAL_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 3v18"/>
  <circle cx="12" cy="3" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_RAY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 16L20 8"/>
  <circle cx="4" cy="16" r="2" fill="currentColor"/>
  <path d="M18 6l2 2-2 2"/>
</svg>"##;

pub const ICON_EXTENDED_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 18L22 6"/>
  <circle cx="8" cy="14" r="2" fill="currentColor"/>
  <circle cx="16" cy="10" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_PARALLEL_CHANNEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L20 10"/>
  <path d="M4 10L20 2"/>
  <circle cx="4" cy="18" r="1.5" fill="currentColor"/>
  <circle cx="4" cy="10" r="1.5" fill="currentColor"/>
</svg>"##;

pub const ICON_HORIZONTAL_RAY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 12h17"/>
  <circle cx="4" cy="12" r="2" fill="currentColor"/>
  <path d="M19 9l3 3-3 3"/>
</svg>"##;

pub const ICON_CROSS_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 3v18"/>
  <path d="M3 12h18"/>
  <circle cx="12" cy="12" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_INFO_LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L20 6"/>
  <circle cx="4" cy="18" r="2" fill="currentColor"/>
  <rect x="14" y="4" width="8" height="5" rx="1"/>
</svg>"##;

pub const ICON_TREND_ANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L18 6"/>
  <path d="M4 18h14"/>
  <path d="M8 18a4 4 0 0 0 2.5-3.5"/>
  <circle cx="4" cy="18" r="2" fill="currentColor"/>
  <circle cx="18" cy="6" r="2" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Channel Icons
// =============================================================================

pub const ICON_REGRESSION_TREND: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 18L21 6"/><path d="M3 14L21 2" stroke-dasharray="2 2"/><path d="M3 22L21 10" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_FLAT_TOP_BOTTOM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 6h18"/><path d="M3 18h18"/><path d="M3 6L10 12L3 18"/>
</svg>"##;

pub const ICON_DISJOINT_CHANNEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 18L12 10"/><path d="M12 14L21 6"/><path d="M3 10L12 2"/><path d="M12 6L21 14"/>
</svg>"##;

// =============================================================================
// Pitchfork Icons
// =============================================================================

pub const ICON_PITCHFORK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 20V8"/>
  <path d="M6 2v6c0 2 2 3 6 3s6-1 6-3V2"/>
  <path d="M6 2v4"/>
  <path d="M12 2v4"/>
  <path d="M18 2v4"/>
</svg>"##;

// =============================================================================
// Fibonacci Icons
// =============================================================================

pub const ICON_FIB_RETRACEMENT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 4h16"/>
  <path d="M4 9h16" stroke-dasharray="4 2"/>
  <path d="M4 14h16" stroke-dasharray="4 2"/>
  <path d="M4 20h16"/>
  <path d="M4 4v16"/>
</svg>"##;

pub const ICON_FIB_EXTENSION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L12 4L20 12"/>
  <path d="M4 8h16" stroke-dasharray="2 2"/>
  <path d="M4 14h16" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_FIB_CHANNEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 20L22 8"/>
  <path d="M2 14L22 2"/>
  <path d="M6 12L22 5" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_FIB_CIRCLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="9"/>
  <circle cx="12" cy="12" r="6" stroke-dasharray="2 2"/>
  <circle cx="12" cy="12" r="3" stroke-dasharray="2 2"/>
  <circle cx="12" cy="12" r="1" fill="currentColor"/>
</svg>"##;

pub const ICON_FIB_SPIRAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 12a5 5 0 0 1 5 5"/>
  <path d="M12 12a8 8 0 0 0-8 8"/>
  <path d="M12 12a3 3 0 0 1-3-3"/>
  <path d="M12 12a2 2 0 0 0 2-2"/>
  <circle cx="12" cy="12" r="1" fill="currentColor"/>
</svg>"##;

pub const ICON_FIB_TIME_ZONES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 4v16"/><path d="M8 4v16"/><path d="M13 4v16" stroke-dasharray="2 2"/><path d="M20 4v16" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_FIB_SPEED_RESISTANCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L20 4"/><path d="M4 20L20 10" stroke-dasharray="2 2"/><path d="M4 20L20 16" stroke-dasharray="2 2"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_FIB_TREND_TIME: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L12 6L20 14"/><path d="M4 18v-14"/><path d="M12 6v12" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_FIB_ARCS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20Q4 8 20 4"/><path d="M4 20Q4 12 14 8" stroke-dasharray="2 2"/><path d="M4 20Q4 16 10 14" stroke-dasharray="2 2"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_FIB_WEDGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L20 4"/><path d="M4 20L20 12"/><path d="M4 20L20 8" stroke-dasharray="2 2"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_FIB_FAN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L20 4"/><path d="M4 20L20 8" stroke-dasharray="2 2"/><path d="M4 20L20 12" stroke-dasharray="2 2"/><path d="M4 20L20 16" stroke-dasharray="2 2"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Gann Icons
// =============================================================================

pub const ICON_GANN_BOX: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="4" width="16" height="16"/><path d="M4 4L20 20"/><path d="M20 4L4 20"/><path d="M12 4v16"/><path d="M4 12h16"/>
</svg>"##;

pub const ICON_GANN_SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="4" width="16" height="16"/><circle cx="12" cy="12" r="6" stroke-dasharray="2 2"/>
  <path d="M4 4L20 20" stroke-dasharray="2 2"/><path d="M20 4L4 20" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_GANN_FAN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20L20 4"/><path d="M4 20L20 8"/><path d="M4 20L20 12"/><path d="M4 20L20 16"/><path d="M4 20h16"/>
  <circle cx="4" cy="20" r="2" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Pattern Icons
// =============================================================================

pub const ICON_XABCD_PATTERN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 18L6 6L12 14L16 8L22 16"/>
  <circle cx="2" cy="18" r="1.5" fill="currentColor"/><circle cx="6" cy="6" r="1.5" fill="currentColor"/>
  <circle cx="12" cy="14" r="1.5" fill="currentColor"/><circle cx="16" cy="8" r="1.5" fill="currentColor"/>
  <circle cx="22" cy="16" r="1.5" fill="currentColor"/>
</svg>"##;

pub const ICON_HEAD_SHOULDERS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 16L6 10L10 14L12 4L14 14L18 10L22 16"/>
  <path d="M6 14h12" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_ABCD_PATTERN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L8 6L16 14L20 6"/>
  <circle cx="4" cy="18" r="1.5" fill="currentColor"/><circle cx="8" cy="6" r="1.5" fill="currentColor"/>
  <circle cx="16" cy="14" r="1.5" fill="currentColor"/><circle cx="20" cy="6" r="1.5" fill="currentColor"/>
</svg>"##;

pub const ICON_TRIANGLE_PATTERN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 8L8 16L12 8L16 16L20 12"/>
  <path d="M4 6L20 10" stroke-dasharray="2 2"/><path d="M4 18L20 14" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_THREE_DRIVES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 20L6 8L10 14L14 6L18 12L22 4"/>
  <circle cx="6" cy="8" r="1.5" fill="currentColor"/><circle cx="14" cy="6" r="1.5" fill="currentColor"/><circle cx="22" cy="4" r="1.5" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Elliott Wave Icons
// =============================================================================

pub const ICON_ELLIOTT_WAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 18L5 14L8 16L12 4L15 12L18 10L22 14"/>
  <circle cx="5" cy="14" r="1.5" fill="currentColor"/>
  <circle cx="12" cy="4" r="1.5" fill="currentColor"/>
  <circle cx="22" cy="14" r="1.5" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Cycle Icons
// =============================================================================

pub const ICON_CYCLE_LINES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 4v16"/><path d="M12 4v16"/><path d="M20 4v16"/>
  <path d="M4 20h16"/>
</svg>"##;

pub const ICON_TIME_CYCLES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="8" cy="12" r="5"/><circle cx="16" cy="12" r="5"/>
</svg>"##;

pub const ICON_SINE_WAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12c2-4 4-4 6 0s4 4 6 0 4-4 6 0 4 4 6 0"/>
</svg>"##;

// =============================================================================
// Shape Icons
// =============================================================================

pub const ICON_RECTANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="6" width="16" height="12" rx="1"/>
</svg>"##;

pub const ICON_ROTATED_RECTANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M6 4L20 8L18 20L4 16L6 4z"/>
</svg>"##;

pub const ICON_CIRCLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="9"/>
</svg>"##;

pub const ICON_ELLIPSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M5 12a7 4 0 1 0 14 0a7 4 0 1 0 -14 0"/>
</svg>"##;

pub const ICON_TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 4L21 20H3L12 4z"/>
</svg>"##;

pub const ICON_ARC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18Q12 2 20 18"/>
  <circle cx="4" cy="18" r="2" fill="currentColor"/><circle cx="20" cy="18" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_POLYLINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18L8 8L14 14L20 6"/>
  <circle cx="4" cy="18" r="1.5" fill="currentColor"/><circle cx="8" cy="8" r="1.5" fill="currentColor"/>
  <circle cx="14" cy="14" r="1.5" fill="currentColor"/><circle cx="20" cy="6" r="1.5" fill="currentColor"/>
</svg>"##;

pub const ICON_PATH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18Q8 4 12 12T20 6"/>
</svg>"##;

pub const ICON_CURVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18Q12 2 20 18"/>
  <path d="M4 18L12 2" stroke-dasharray="2 2"/><path d="M12 2L20 18" stroke-dasharray="2 2"/>
  <circle cx="4" cy="18" r="1.5" fill="currentColor"/><circle cx="12" cy="2" r="1.5" fill="currentColor"/><circle cx="20" cy="18" r="1.5" fill="currentColor"/>
</svg>"##;

pub const ICON_DOUBLE_CURVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 12Q8 4 12 12T20 12"/>
  <path d="M4 16Q8 8 12 16T20 16" stroke-dasharray="2 2"/>
</svg>"##;

// =============================================================================
// Arrow Icons
// =============================================================================

pub const ICON_ARROW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M5 19L19 5"/>
  <path d="M19 5h-6"/>
  <path d="M19 5v6"/>
</svg>"##;

pub const ICON_ARROW_UP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 4l8 14H4z"/>
</svg>"##;

pub const ICON_ARROW_DOWN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 20l8-14H4z"/>
</svg>"##;

// =============================================================================
// Brush Icons
// =============================================================================

pub const ICON_BRUSH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 19l7-7 3 3-7 7-3-3z" fill="currentColor"/>
  <path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z"/>
  <path d="M2 2l7.586 7.586"/>
</svg>"##;

pub const ICON_HIGHLIGHTER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M14 3l7 7-10 10H4v-7L14 3z" fill="currentColor" fill-opacity="0.3"/>
  <path d="M14 3l7 7-10 10H4v-7L14 3z"/>
</svg>"##;

// =============================================================================
// Annotation Icons
// =============================================================================

pub const ICON_TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 6h16"/>
  <path d="M12 6v14"/>
  <path d="M8 20h8"/>
</svg>"##;

pub const ICON_ANCHORED_TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 6h16"/><path d="M12 6v14"/><path d="M8 20h8"/>
  <circle cx="12" cy="6" r="2" fill="currentColor"/>
</svg>"##;

pub const ICON_NOTE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
  <path d="M14 3v6h6"/>
</svg>"##;

pub const ICON_PRICE_NOTE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
  <path d="M14 3v6h6"/><path d="M8 13h8"/><path d="M8 17h4"/>
</svg>"##;

pub const ICON_SIGNPOST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 3v18"/><path d="M6 7h10l2 2-2 2H6V7z"/>
</svg>"##;

pub const ICON_TABLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18"/><path d="M3 16h18"/><path d="M10 4v16"/>
</svg>"##;

pub const ICON_CALLOUT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"/>
</svg>"##;

pub const ICON_COMMENT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
</svg>"##;

pub const ICON_PRICE_LABEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 9h12l4 3-4 3H4z"/>
  <path d="M4 9v6"/>
</svg>"##;

pub const ICON_SIGN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="6" width="18" height="12" rx="2"/><path d="M12 6v-3"/><path d="M12 21v-3"/>
</svg>"##;

pub const ICON_FLAG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M8 21V4"/><path d="M8 4l12 4-12 4"/>
</svg>"##;

pub const ICON_DIAMOND: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="none">
  <path d="M12 2L22 12L12 22L2 12Z"/>
</svg>"##;

pub const ICON_EMOJI: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/>
</svg>"##;

pub const ICON_IMAGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="M21 15l-5-5L5 21"/>
</svg>"##;

// =============================================================================
// Measurement Icons
// =============================================================================

pub const ICON_PRICE_RANGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 4v16"/>
  <path d="M8 8l4-4 4 4"/>
  <path d="M8 16l4 4 4-4"/>
  <path d="M4 12h4"/>
  <path d="M16 12h4"/>
</svg>"##;

pub const ICON_DATE_RANGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 12h16"/>
  <path d="M8 8l-4 4 4 4"/>
  <path d="M16 8l4 4-4 4"/>
  <path d="M12 4v4"/>
  <path d="M12 16v4"/>
</svg>"##;

pub const ICON_PRICE_DATE_RANGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="4 2"/>
  <path d="M4 4l16 16"/>
  <circle cx="4" cy="4" r="2" fill="currentColor"/>
  <circle cx="20" cy="20" r="2" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Volume Icons
// =============================================================================

pub const ICON_VOLUME_PROFILE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="4" width="8" height="3" fill="currentColor" fill-opacity="0.5"/>
  <rect x="4" y="8" width="14" height="3" fill="currentColor"/>
  <rect x="4" y="12" width="10" height="3" fill="currentColor" fill-opacity="0.5"/>
  <rect x="4" y="16" width="6" height="3" fill="currentColor" fill-opacity="0.3"/>
  <path d="M20 4v16"/>
</svg>"##;

// =============================================================================
// Projection Icons
// =============================================================================

pub const ICON_BARS_PATTERN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="8" width="3" height="8"/>
  <rect x="9" y="6" width="3" height="10"/>
  <rect x="14" y="10" width="3" height="8" stroke-dasharray="2 2"/>
  <rect x="19" y="7" width="3" height="9" stroke-dasharray="2 2"/>
</svg>"##;

pub const ICON_PRICE_PROJECTION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 18l6-8 4 4 6-10"/>
  <path d="M4 6h16" stroke-dasharray="4 2"/>
  <path d="M20 4v4h-4"/>
</svg>"##;

pub const ICON_PROJECTION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 20l7-7"/>
  <path d="M10 13l4 4" stroke-dasharray="2 2"/>
  <path d="M14 17l4-8" stroke-dasharray="2 2"/>
  <circle cx="3" cy="20" r="2" fill="currentColor"/>
  <circle cx="10" cy="13" r="1.5" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Tool Icons
// =============================================================================

pub const ICON_CROSSHAIR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 3v18"/>
  <path d="M3 12h18"/>
</svg>"##;

pub const ICON_MAGNET: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 3h4v4H4z" fill="currentColor"/>
  <path d="M16 3h4v4h-4z" fill="currentColor"/>
  <path d="M4 7v5a8 8 0 0 0 16 0V7"/>
  <path d="M8 7v5a4 4 0 0 0 8 0V7"/>
</svg>"##;

/// Strong magnet icon — same U-shape as ICON_MAGNET but with an electric discharge
/// (zigzag lightning bolt) between the two pole tips, indicating strong body-snap mode.
pub const ICON_MAGNET_STRONG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 3h4v4H4z" fill="currentColor"/>
  <path d="M16 3h4v4h-4z" fill="currentColor"/>
  <path d="M4 7v5a8 8 0 0 0 16 0V7"/>
  <path d="M8 7v5a4 4 0 0 0 8 0V7"/>
  <polyline points="8,7 10,4 12,7 14,4 16,7" stroke-width="1.5"/>
</svg>"##;

pub const ICON_CURSOR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 4l7 17 2-7 7-2L4 4z" fill="currentColor"/>
</svg>"##;

pub const ICON_HAND: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M18 11V6a2 2 0 0 0-4 0"/>
  <path d="M14 10V4a2 2 0 0 0-4 0v6"/>
  <path d="M10 10.5V6a2 2 0 0 0-4 0v8"/>
  <path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-5.9-2.4L3.3 16"/>
</svg>"##;

pub const ICON_ZOOM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="10" cy="10" r="7"/>
  <path d="M21 21l-4.35-4.35"/>
  <path d="M10 7v6"/>
  <path d="M7 10h6"/>
</svg>"##;

// =============================================================================
// Action Icons
// =============================================================================

pub const ICON_UNDO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 7h10a5 5 0 0 1 5 5v0a5 5 0 0 1-5 5H9"/>
  <path d="M7 4l-4 3 4 3"/>
</svg>"##;

pub const ICON_REDO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M20 7H10a5 5 0 0 0-5 5v0a5 5 0 0 0 5 5h5"/>
  <path d="M17 4l4 3-4 3"/>
</svg>"##;

pub const ICON_DELETE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 6h18"/>
  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/>
  <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
</svg>"##;

pub const ICON_LOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="5" y="11" width="14" height="10" rx="2"/>
  <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
</svg>"##;

pub const ICON_UNLOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="5" y="11" width="14" height="10" rx="2"/>
  <path d="M7 11V7a5 5 0 0 1 9.9-1"/>
</svg>"##;

pub const ICON_EYE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z"/>
  <circle cx="12" cy="12" r="3"/>
  <path d="M7 7L5.5 4.5"/>
  <path d="M12 5V2"/>
  <path d="M17 7l1.5-2.5"/>
</svg>"##;

pub const ICON_EYE_OFF: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z"/>
  <path d="M7 17l-1.5 2.5"/>
  <path d="M12 19v3"/>
  <path d="M17 17l1.5 2.5"/>
</svg>"##;

pub const ICON_COPY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="9" y="9" width="13" height="13" rx="2"/>
  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
</svg>"##;

pub const ICON_SETTINGS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="3"/>
  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
</svg>"##;

pub const ICON_BOOKMARK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
</svg>"##;

pub const ICON_EXPAND: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M0 0l8 8"/>
  <path d="M0 0h8"/>
  <path d="M0 0v8"/>
  <path d="M24 0l-8 8"/>
  <path d="M24 0h-8"/>
  <path d="M24 0v8"/>
  <path d="M0 24l8-8"/>
  <path d="M0 24h8"/>
  <path d="M0 24v-8"/>
  <path d="M24 24l-8-8"/>
  <path d="M24 24h-8"/>
  <path d="M24 24v-8"/>
</svg>"##;

pub const ICON_MOVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M5 9l-3 3 3 3"/>
  <path d="M9 5l3-3 3 3"/>
  <path d="M15 19l-3 3-3-3"/>
  <path d="M19 9l3 3-3 3"/>
  <line x1="2" y1="12" x2="22" y2="12"/>
  <line x1="12" y1="2" x2="12" y2="22"/>
</svg>"##;

pub const ICON_CLOSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <line x1="18" y1="6" x2="6" y2="18"/>
  <line x1="6" y1="6" x2="18" y2="18"/>
</svg>"##;

// =============================================================================
// Position Icons
// =============================================================================

pub const ICON_LONG_POSITION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 20V4"/>
  <path d="M5 11l7-7 7 7"/>
  <path d="M5 20h14"/>
</svg>"##;

pub const ICON_SHORT_POSITION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 4v16"/>
  <path d="M5 13l7 7 7-7"/>
  <path d="M5 4h14"/>
</svg>"##;

// =============================================================================
// Navigation Icons
// =============================================================================

pub const ICON_PLUS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 5v14"/>
  <path d="M5 12h14"/>
</svg>"##;

pub const ICON_MINUS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M5 12h14"/>
</svg>"##;

pub const ICON_INDICATORS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 8l4-3 5 4 5-5 4 3"/>
  <rect x="4" y="14" width="3" height="7" fill="currentColor" stroke="none"/>
  <rect x="10" y="16" width="3" height="5" fill="currentColor" stroke="none"/>
  <rect x="16" y="12" width="3" height="9" fill="currentColor" stroke="none"/>
</svg>"##;

pub const ICON_CHEVRON_UP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M18 15l-6-6-6 6"/>
</svg>"##;

pub const ICON_CHEVRON_DOWN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M6 9l6 6 6-6"/>
</svg>"##;

pub const ICON_CHEVRON_RIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M9 6l6 6-6 6"/>
</svg>"##;

pub const ICON_CHEVRON_LEFT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 18l-6-6 6-6"/></svg>"##;

pub const ICON_GRID: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="7" height="7"/>
  <rect x="14" y="3" width="7" height="7"/>
  <rect x="14" y="14" width="7" height="7"/>
  <rect x="3" y="14" width="7" height="7"/>
</svg>"##;

pub const ICON_LAYERS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <polygon points="12 2 2 7 12 12 22 7 12 2"/>
  <polyline points="2 17 12 22 22 17"/>
  <polyline points="2 12 12 17 22 12"/>
</svg>"##;

pub const ICON_LAYOUT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
</svg>"##;

// =============================================================================
// UI Element Icons
// =============================================================================

pub const ICON_SEARCH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="11" cy="11" r="8"/>
  <path d="M21 21l-4.35-4.35"/>
</svg>"##;

pub const ICON_CLOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="10"/>
  <path d="M12 6v6l4 2"/>
</svg>"##;

pub const ICON_WATERMARK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M7 12h10" stroke-opacity="0.5"/>
  <path d="M12 7v10" stroke-opacity="0.5"/>
</svg>"##;

pub const ICON_LEGEND: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M7 8h2"/>
  <path d="M11 8h6"/>
  <path d="M7 12h2"/>
  <path d="M11 12h6"/>
  <path d="M7 16h2"/>
  <path d="M11 16h6"/>
</svg>"##;

pub const ICON_TOOLTIP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="10"/>
  <path d="M12 16v-4"/>
  <circle cx="12" cy="8" r="0.5" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Panel Icons
// =============================================================================

pub const ICON_WATCHLIST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M7 7h6"/>
  <path d="M7 11h10"/>
  <path d="M7 15h8"/>
</svg>"##;

pub const ICON_ALERT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/>
  <path d="M13.73 21a2 2 0 0 1-3.46 0"/>
</svg>"##;

pub const ICON_TRADING: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 2v20"/>
  <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/>
</svg>"##;

pub const ICON_POSITIONS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="2" y="7" width="20" height="14" rx="2"/>
  <path d="M16 7V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v2"/>
  <path d="M12 12v4"/>
  <path d="M2 12h20"/>
</svg>"##;

pub const ICON_PANEL_RIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M15 3v18"/>
</svg>"##;

pub const ICON_PANEL_BOTTOM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M3 15h18"/>
</svg>"##;

// =============================================================================
// Theme Icons
// =============================================================================

pub const ICON_PALETTE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="13.5" cy="6.5" r="2"/>
  <circle cx="17.5" cy="10.5" r="2"/>
  <circle cx="8.5" cy="7.5" r="2"/>
  <circle cx="6.5" cy="12.5" r="2"/>
  <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.555C21.965 6.012 17.461 2 12 2z"/>
</svg>"##;

// =============================================================================
// Info / Signal / Menu Icons
// =============================================================================

pub const ICON_INFO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="10"/>
  <path d="M12 16v-4"/>
  <path d="M12 8h.01"/>
</svg>"##;

pub const ICON_SIGNAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 16l4-6 5 8 5-12 4 6"/>
  <polygon points="3,19 1,23 5,23" fill="currentColor" stroke="none"/>
  <polygon points="17,4 15,0 19,0" fill="currentColor" stroke="none"/>
</svg>"##;

pub const ICON_MENU: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 6h16"/>
  <path d="M4 12h16"/>
  <path d="M4 18h16"/>
</svg>"##;

// =============================================================================
// Line Style Icons
// =============================================================================

pub const ICON_LINE_SOLID: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 12h18"/>
</svg>"##;

pub const ICON_LINE_DASHED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 12h4"/>
  <path d="M10 12h4"/>
  <path d="M17 12h4"/>
</svg>"##;

pub const ICON_LINE_DOTTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="4" cy="12" r="1" fill="currentColor"/>
  <circle cx="8" cy="12" r="1" fill="currentColor"/>
  <circle cx="12" cy="12" r="1" fill="currentColor"/>
  <circle cx="16" cy="12" r="1" fill="currentColor"/>
  <circle cx="20" cy="12" r="1" fill="currentColor"/>
</svg>"##;

// =============================================================================
// Primitive Toolbar Icons
// =============================================================================

pub const ICON_PENCIL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>
  <path d="m15 5 4 4"/>
</svg>"##;

pub const ICON_COLOR_FILL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="m19 11-8-8-8.6 8.6a2 2 0 0 0 0 2.8l5.2 5.2c.8.8 2 .8 2.8 0L19 11Z"/>
  <path d="m5 2 5 5"/>
  <path d="M2 13h15"/>
  <path d="M22 20a2 2 0 1 1-4 0c0-1.6 1.7-2.4 2-4 .3 1.6 2 2.4 2 4Z"/>
</svg>"##;

pub const ICON_TEXT_COLOR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M4 20h16"/>
  <path d="m6 16 6-12 6 12"/>
  <path d="M8 12h8"/>
</svg>"##;

pub const ICON_LINE_WIDTH_1: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round">
  <path d="M4 12h16" stroke-width="1"/>
</svg>"##;

pub const ICON_LINE_WIDTH_2: &str = r##"<svg xmlns="http://www.w3.org/2020/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round">
  <path d="M4 12h16" stroke-width="2"/>
</svg>"##;

pub const ICON_LINE_WIDTH_3: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round">
  <path d="M4 12h16" stroke-width="3"/>
</svg>"##;

pub const ICON_LINE_WIDTH_4: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round">
  <path d="M4 12h16" stroke-width="4"/>
</svg>"##;

pub const ICON_MORE_HORIZONTAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
  <circle cx="5" cy="12" r="2"/>
  <circle cx="12" cy="12" r="2"/>
  <circle cx="19" cy="12" r="2"/>
</svg>"##;

// =============================================================================
// Window Layout Icons
// =============================================================================

pub const ICON_LAYOUT_SINGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="18" height="18" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_SPLIT_H: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="8" height="18" rx="1"/>
  <rect x="13" y="3" width="8" height="18" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_SPLIT_V: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="18" height="8" rx="1"/>
  <rect x="3" y="13" width="18" height="8" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_GRID_2X2: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="8" height="8" rx="1"/>
  <rect x="13" y="3" width="8" height="8" rx="1"/>
  <rect x="3" y="13" width="8" height="8" rx="1"/>
  <rect x="13" y="13" width="8" height="8" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_2LEFT_1RIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="8" height="8" rx="1"/>
  <rect x="3" y="13" width="8" height="8" rx="1"/>
  <rect x="13" y="3" width="8" height="18" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_1LEFT_2RIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="8" height="18" rx="1"/>
  <rect x="13" y="3" width="8" height="8" rx="1"/>
  <rect x="13" y="13" width="8" height="8" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_2TOP_1BOTTOM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="8" height="8" rx="1"/>
  <rect x="13" y="3" width="8" height="8" rx="1"/>
  <rect x="3" y="13" width="18" height="8" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_1TOP_2BOTTOM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="18" height="8" rx="1"/>
  <rect x="3" y="13" width="8" height="8" rx="1"/>
  <rect x="13" y="13" width="8" height="8" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_3COLUMNS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="5" height="18" rx="1"/>
  <rect x="9.5" y="3" width="5" height="18" rx="1"/>
  <rect x="16" y="3" width="5" height="18" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_3ROWS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="18" height="5" rx="1"/>
  <rect x="3" y="9.5" width="18" height="5" rx="1"/>
  <rect x="3" y="16" width="18" height="5" rx="1"/>
</svg>"##;

pub const ICON_LAYOUT_1BIG_3SMALL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <rect x="3" y="3" width="12" height="18" rx="1"/>
  <rect x="17" y="3" width="4" height="5" rx="1"/>
  <rect x="17" y="9.5" width="4" height="5" rx="1"/>
  <rect x="17" y="16" width="4" height="5" rx="1"/>
</svg>"##;

// =============================================================================
// Expand/Collapse Icons
// =============================================================================

pub const ICON_COLLAPSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M8 8L0 0"/>
  <path d="M8 8H0"/>
  <path d="M8 8V0"/>
  <path d="M16 8l8-8"/>
  <path d="M16 8h8"/>
  <path d="M16 8V0"/>
  <path d="M8 16L0 24"/>
  <path d="M8 16H0"/>
  <path d="M8 16v8"/>
  <path d="M16 16l8 8"/>
  <path d="M16 16h8"/>
  <path d="M16 16v8"/>
</svg>"##;

// =============================================================================
// Object Tree / Sidebar Icons
// =============================================================================

pub const ICON_OBJECT_TREE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <polygon points="12 2 2 7 12 12 22 7 12 2"/>
  <polyline points="2 17 12 22 22 17"/>
  <polyline points="2 12 12 17 22 12"/>
</svg>"##;

// =============================================================================
// Zoom Control Icons
// =============================================================================

pub const ICON_ZOOM_IN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<circle cx="11" cy="11" r="8"/>
<line x1="21" y1="21" x2="16.65" y2="16.65"/>
<line x1="11" y1="8" x2="11" y2="14"/>
<line x1="8" y1="11" x2="14" y2="11"/>
</svg>"##;

pub const ICON_ZOOM_OUT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<circle cx="11" cy="11" r="8"/>
<line x1="21" y1="21" x2="16.65" y2="16.65"/>
<line x1="8" y1="11" x2="14" y2="11"/>
</svg>"##;

pub const ICON_ZOOM_RESET: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<polyline points="15 3 21 3 21 9"/>
<polyline points="9 21 3 21 3 15"/>
<line x1="21" y1="3" x2="14" y2="10"/>
<line x1="3" y1="21" x2="10" y2="14"/>
</svg>"##;

// =============================================================================
// Screenshot Icon
// =============================================================================

pub const ICON_SCREENSHOT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/>
<circle cx="12" cy="13" r="4"/>
</svg>"##;

// =============================================================================
// Connector Icons
// =============================================================================

pub const ICON_CIRCUIT_BOARD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M11 9h4a2 2 0 0 0 2-2V3"/>
  <circle cx="9" cy="9" r="2"/>
  <path d="M7 21v-4a2 2 0 0 1 2-2h4"/>
  <circle cx="15" cy="15" r="2"/>
</svg>"##;

/// CPU chip with pins — processor icon for Performance panel
pub const ICON_CPU: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="4" y="4" width="16" height="16" rx="2"/>
  <rect x="9" y="9" width="6" height="6" rx="1"/>
  <path d="M9 1v3"/><path d="M15 1v3"/>
  <path d="M9 20v3"/><path d="M15 20v3"/>
  <path d="M20 9h3"/><path d="M20 14h3"/>
  <path d="M1 9h3"/><path d="M1 14h3"/>
</svg>"##;

// =============================================================================
// Window Management Icons
// =============================================================================

pub const ICON_NEW_WINDOW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M15 3L21 3L21 9"/>
  <path d="M21 3L10 14"/>
  <path d="M18 13L18 19C18 20.1046 17.1046 21 16 21L5 21C3.89543 21 3 20.1046 3 19L3 8C3 6.89543 3.89543 6 5 6L11 6"/>
</svg>"##;

// =============================================================================
// User / Auth / Cloud Icons
// =============================================================================

pub const ICON_CLOUD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/></svg>"##;

pub const ICON_CLOUD_DOWNLOAD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="8 17 12 21 16 17"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.88 18.09A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.29"/></svg>"##;

pub const ICON_USER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>"##;

pub const ICON_LOG_IN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><polyline points="10 17 15 12 10 7"/><line x1="15" y1="12" x2="3" y2="12"/></svg>"##;

pub const ICON_LOG_OUT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>"##;

pub const ICON_REFRESH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>"##;

pub const ICON_SHIELD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>"##;

pub const ICON_SHIELD_CHECK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/></svg>"##;

pub const ICON_GLOBE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>"##;

pub const ICON_KEY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>"##;

// =============================================================================
// Star Icons (Watchlist)
// =============================================================================

pub const ICON_STAR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>"##;

pub const ICON_STAR_FILLED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>"##;

// =============================================================================
// Agent Panel Icons
// =============================================================================

/// Terminal / PTY prompt icon (>_)
pub const ICON_TERMINAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>"##;

/// Chat bubble icon (speech balloon)
pub const ICON_CHAT_BUBBLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>"##;

/// Replace layout icon — bold geometric letter "R"
pub const ICON_LAYOUT_REPLACE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M6 4h7.5a5.5 5.5 0 0 1 1.5 10.78L19.5 20h-3.7l-4-5H9.5v5H6V4zm3.5 3v5H13a2.5 2.5 0 0 0 0-5H9.5z"/></svg>"##;
