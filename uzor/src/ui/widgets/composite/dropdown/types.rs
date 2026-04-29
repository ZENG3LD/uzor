//! Dropdown type definitions — per-frame view data and render kind enum.
//!
//! Ported from the mlc deep audit in `dropdown-deep.md`.
//! Five structurally-distinct templates cover all mlc Dropdown variants.

use super::settings::DropdownSettings;
use super::state::DropdownState;
use crate::render::RenderContext;
use crate::types::Rect;

// ---------------------------------------------------------------------------
// DropdownItem  (Flat / Inline templates)
// ---------------------------------------------------------------------------

/// Row type for Flat and Inline dropdown templates.
pub enum DropdownItem<'a> {
    /// Section header — non-clickable bold title row with bottom border.
    Header { label: &'a str },

    /// Regular selectable item row.
    Item {
        /// Stable string id (returned on click).
        id: &'a str,
        /// Display label.
        label: &'a str,
        /// Optional icon identifier.
        icon: Option<&'a str>,
        /// Right-side content (shortcut / subtitle / toggle — mutually exclusive).
        right: DropdownItemRight<'a>,
        /// Whether the item is selectable.
        disabled: bool,
        /// Whether the item is danger-styled (red text + red hover bg).
        danger: bool,
        /// Accent bar color override (2 px left-edge bar).  `None` = no bar.
        accent_color: Option<&'a str>,
    },

    /// Horizontal 1 px divider.
    Separator,

    /// Item that opens a sibling submenu panel on hover/click.
    Submenu {
        /// Stable string id for the submenu trigger row.
        id: &'a str,
        /// Display label.
        label: &'a str,
        /// Optional icon identifier.
        icon: Option<&'a str>,
    },
}

// ---------------------------------------------------------------------------
// DropdownItemRight
// ---------------------------------------------------------------------------

/// Right-side content of an item row (mutually exclusive per row).
pub enum DropdownItemRight<'a> {
    /// No right-side content.
    None,
    /// Shortcut text, right-aligned in `shortcut_text` color.
    Shortcut(&'a str),
    /// Subtitle text, right-aligned in `item_text_disabled` color, smaller font.
    Subtitle(&'a str),
    /// Toggle pill switch — `true` = on (blue), `false` = off (disabled color).
    Toggle(bool),
}

// ---------------------------------------------------------------------------
// GridDropdownItem
// ---------------------------------------------------------------------------

/// Single cell for the `Grid` template (icon-only, no label).
pub struct GridDropdownItem<'a> {
    /// Stable string id.
    pub id: &'a str,
    /// Icon identifier.
    pub icon: &'a str,
    /// Whether the cell is selectable.
    pub disabled: bool,
}

// ---------------------------------------------------------------------------
// DropdownGroup  (Grouped template)
// ---------------------------------------------------------------------------

/// One group row in the `Grouped` template (row label + cells).
pub struct DropdownGroup<'a> {
    /// Row label shown on the left (e.g. "1", "2", "3").
    pub label: &'a str,
    /// Icon cells in this row.
    pub items: &'a [GridDropdownItem<'a>],
}

// ---------------------------------------------------------------------------
// CheckboxItem  (Grouped template list section)
// ---------------------------------------------------------------------------

/// Stroke-only checkbox item below the grid section in `Grouped`.
pub struct CheckboxItem<'a> {
    /// Stable string id.
    pub id: &'a str,
    /// Display label.
    pub label: &'a str,
    /// Whether the checkbox is checked.
    pub checked: bool,
    /// Whether the item is selectable.
    pub disabled: bool,
}

// ---------------------------------------------------------------------------
// DropdownViewKind
// ---------------------------------------------------------------------------

/// Template-specific per-frame data.
pub enum DropdownViewKind<'a> {
    /// Single-level item list anchored below trigger (Template 1: Flat).
    Flat {
        /// Ordered item rows.
        items: &'a [DropdownItem<'a>],
        /// Id of the currently hovered item, if any.
        hovered_id: Option<&'a str>,
        /// Submenu data: `(trigger_item_id, submenu_items)`.
        /// `None` = no submenu open.
        submenu_items: Option<(&'a str, &'a [DropdownItem<'a>])>,
        /// Id of the currently hovered item inside the open submenu, if any.
        submenu_hovered_id: Option<&'a str>,
    },

    /// Split trigger + option list inside settings modals (Template 2: Inline).
    Inline {
        /// Currently selected / displayed value.
        current_value: &'a str,
        /// Options as `(id, label)` pairs.
        options: &'a [(&'a str, &'a str)],
        /// Id of the currently hovered option, if any.
        hovered_id: Option<&'a str>,
    },

    /// Icon-only N×M grid of cells, no labels (Template 3: Grid).
    Grid {
        /// All cells in row-major order.
        items: &'a [GridDropdownItem<'a>],
        /// Number of columns.
        columns: usize,
        /// Id of the currently hovered cell, if any.
        hovered_id: Option<&'a str>,
    },

    /// Grouped rows of square cells + checkbox list section (Template 4: Grouped).
    Grouped {
        /// Row groups (label + cells).
        groups: &'a [DropdownGroup<'a>],
        /// Checkbox items in the list section below the grid.
        list_items: &'a [CheckboxItem<'a>],
        /// Id of the currently hovered item (grid cell or checkbox row).
        hovered_id: Option<&'a str>,
    },

    /// Escape hatch — caller supplies draw closure; composite provides frame (Template 5).
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &DropdownState, &DropdownSettings) + 'a>),
}

// ---------------------------------------------------------------------------
// DropdownView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_dropdown`.
pub struct DropdownView<'a> {
    /// Anchor rect of the trigger button used to re-compute panel origin.
    ///
    /// `None` = caller provides `position_override` instead.
    pub anchor: Option<Rect>,

    /// Override: when `Some`, use this exact screen-space origin for the panel.
    /// Takes priority over anchor-derived position.
    pub position_override: Option<(f64, f64)>,

    /// Whether the dropdown panel is open (should be drawn and registered).
    pub open: bool,

    /// Template-specific data.
    pub kind: DropdownViewKind<'a>,
}

// ---------------------------------------------------------------------------
// DropdownRenderKind  (discriminant-only for dispatch)
// ---------------------------------------------------------------------------

/// Layout / input registration strategy selector.
///
/// Mirrors the active `DropdownViewKind` variant but without per-frame data,
/// making it cheap to pass around for registration-only paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownRenderKind {
    /// Single-level item list (Template 1: Flat).
    Flat,
    /// Split trigger + option list (Template 2: Inline).
    Inline,
    /// Icon-only grid (Template 3: Grid).
    Grid,
    /// Grouped rows + checkbox list (Template 4: Grouped).
    Grouped,
    /// Escape hatch (Template 5: Custom).
    Custom,
}
