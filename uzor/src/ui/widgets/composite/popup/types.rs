//! Popup type definitions — semantic popup variants.
//!
//! Ported from the mlc deep audit in `popup-deep.md`.
//! Six structurally-distinct templates cover all mlc popup variants.

use super::settings::PopupSettings;
use super::state::PopupState;
use crate::input::InputCoordinator;
use crate::render::RenderContext;
use crate::types::Rect;

// ---------------------------------------------------------------------------
// ColorPickerLevel
// ---------------------------------------------------------------------------

/// State machine for the two-level color picker.
///
/// L1 and L2 are mutually exclusive render outputs of the same popup instance —
/// they share a single origin, a single z-layer registration, and a single
/// `PopupState`. Transition is driven by `PopupState.level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorPickerLevel {
    /// Color picker is closed (no render).
    #[default]
    Closed,
    /// L1 — swatch grid + opacity row + "+" button.
    L1,
    /// L2 — HSV square + hue bar + hex input + opacity + Back/Add buttons.
    L2,
}

// ---------------------------------------------------------------------------
// BackdropKind
// ---------------------------------------------------------------------------

/// Controls any fill drawn behind the popup frame.
///
/// Popups are non-modal by default (`None`). Color pickers (`is_modal = true`)
/// use `Dim` to signal to the coordinator that events to lower layers are blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackdropKind {
    /// No backdrop — popup floats freely (default for non-modal popups).
    #[default]
    None,
    /// Semi-transparent dim fill `rgba(0,0,0,0.45)`.
    /// Used by color pickers (`is_modal = true`).
    Dim,
}

// ---------------------------------------------------------------------------
// HsvColor
// ---------------------------------------------------------------------------

/// HSV color representation used by the color picker L2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HsvColor {
    /// Hue in degrees, 0–360.
    pub h: f64,
    /// Saturation, 0.0–1.0.
    pub s: f64,
    /// Value (brightness), 0.0–1.0.
    pub v: f64,
}

impl Default for HsvColor {
    fn default() -> Self {
        Self { h: 0.0, s: 1.0, v: 1.0 }
    }
}

// ---------------------------------------------------------------------------
// IndicatorRowInfo
// ---------------------------------------------------------------------------

/// Per-row data for the `IndicatorStrip` template.
pub struct IndicatorRowInfo<'a> {
    /// Stable numeric id for this indicator row.
    pub id: u64,
    /// Display name shown left-aligned.
    pub display_name: &'a str,
    /// Whether the indicator is currently visible.
    pub visible: bool,
}

// ---------------------------------------------------------------------------
// DropdownItem  (ItemList template)
// ---------------------------------------------------------------------------

/// Row type for the `ItemList` template.
pub enum DropdownItem<'a> {
    /// Section header — non-clickable bold title row.
    Header { label: &'a str },
    /// Regular selectable item.
    Item {
        /// Stable id (returned on click).
        id: &'a str,
        /// Display label.
        label: &'a str,
        /// Optional right-side text (shortcut / subtitle).
        right_label: Option<&'a str>,
        /// Whether the item is selectable.
        disabled: bool,
        /// Whether the item is danger-styled (red).
        danger: bool,
    },
    /// Horizontal divider.
    Separator,
    /// Item with a right-arrow indicating a submenu.
    Submenu { id: &'a str, label: &'a str },
}

// ---------------------------------------------------------------------------
// PopupView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_popup`.
///
/// The `kind` field inside `PopupRenderKind` carries all template-specific
/// data. Fields that are not relevant to the active kind are ignored.
pub struct PopupView<'a> {
    /// Top-left origin of the popup in screen coordinates.
    pub origin: (f64, f64),

    /// Anchor rect used for smart re-positioning on window resize.
    ///
    /// `None` — fixed origin, no re-anchor.
    pub anchor: Option<Rect>,

    /// Backdrop fill strategy (non-modal popups use `None`).
    pub backdrop: BackdropKind,

    /// Template-specific data and (for `Plain`) the body closure.
    pub kind: PopupViewKind<'a>,
}

// ---------------------------------------------------------------------------
// PopupViewKind
// ---------------------------------------------------------------------------

/// Template-specific per-frame data.
pub enum PopupViewKind<'a> {
    /// Frame only — caller fills all content inside the body closure.
    Plain {
        /// Body closure called with the content rect after the frame is drawn.
        body: Box<dyn FnMut(&mut dyn RenderContext, Rect, &mut InputCoordinator) + 'a>,
    },

    /// Swatch grid (10×10) + custom row + opacity row.
    /// Transitions to `ColorPickerHsv` via "+" button.
    ColorPickerGrid {
        /// Currently selected color (hex string, e.g. `"#2962ff"`).
        current_color: &'a str,
        /// Palette swatches to display.
        swatches: &'a [&'a str],
        /// Index of the hovered swatch, if any.
        hovered_swatch: Option<usize>,
        /// Current opacity value, 0.0–1.0.
        opacity: f64,
        /// Whether opacity is toggled off (eye closed).
        opacity_hidden: bool,
    },

    /// SV square + hue bar + hex input + opacity row + Back/Add buttons.
    ColorPickerHsv {
        /// Current HSV values.
        hsv: HsvColor,
        /// Hex input string (may differ from HSV during editing).
        hex_input: &'a str,
        /// Whether the hex input field is focused.
        hex_editing: bool,
        /// Current opacity value, 0.0–1.0.
        opacity: f64,
        /// Whether opacity is toggled off.
        opacity_hidden: bool,
    },

    /// Compact swatch grid (4×3 preset) + optional custom swatches + Remove row.
    SwatchGrid {
        /// Preset swatches as RGBA byte arrays.
        preset_swatches: &'a [[f32; 4]],
        /// User-added custom swatches.
        custom_swatches: &'a [[f32; 4]],
        /// Index of the hovered swatch, if any.
        hovered_index: Option<usize>,
        /// Whether the Remove row is hovered.
        hovered_remove: bool,
        /// Whether the "+" add-custom button is hovered.
        hovered_add: bool,
    },

    /// Vertical list of typed rows (Item, Header, Separator, Submenu).
    ItemList {
        /// Ordered list of rows.
        items: &'a [DropdownItem<'a>],
        /// Id of the currently hovered item, if any.
        hovered_id: Option<&'a str>,
    },

    /// Semi-transparent strip of per-indicator rows with quick-action buttons.
    /// No popup chrome (no border, no shadow).
    IndicatorStrip {
        /// Indicator rows to display.
        indicators: &'a [IndicatorRowInfo<'a>],
        /// Id of the hovered indicator, if any.
        hovered_indicator_id: Option<u64>,
        /// `(indicator_id, action_name)` of the hovered action button, if any.
        hovered_action: Option<(u64, &'a str)>,
    },

    /// Escape hatch — caller drives all draw calls.
    Custom {
        /// Caller-supplied draw closure.
        draw: Box<dyn Fn(&mut dyn RenderContext, Rect, &PopupState, &PopupSettings) + 'a>,
    },
}

// ---------------------------------------------------------------------------
// PopupRenderKind  (discriminant-only, for registration/layout dispatch)
// ---------------------------------------------------------------------------

/// Layout / input registration strategy selector.
///
/// Mirrors the active `PopupViewKind` variant but without per-frame data,
/// making it cheap to pass around for registration-only paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupRenderKind {
    /// Frame only — body closure in `PopupViewKind::Plain`.
    Plain,
    /// Swatch grid color picker (L1). Level driven by `PopupState.level`.
    ColorPickerGrid,
    /// HSV editor color picker (L2). Level driven by `PopupState.level`.
    ColorPickerHsv,
    /// Compact swatch grid (sync-group color tags).
    SwatchGrid,
    /// Vertical item list / dropdown.
    ItemList,
    /// Semi-transparent indicator action strip (no popup chrome).
    IndicatorStrip,
    /// Escape hatch — caller drives all draw calls.
    Custom,
}
