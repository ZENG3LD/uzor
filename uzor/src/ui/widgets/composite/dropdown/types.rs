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

    /// Item that opens a sibling submenu panel.
    Submenu {
        /// Stable string id for the submenu trigger row.
        id: &'a str,
        /// Display label.
        label: &'a str,
        /// Optional icon identifier.
        icon: Option<&'a str>,
        /// How the submenu opens. `Hover` = opens whenever the row is
        /// hovered; `ChevronClick` = opens only when the user clicks the
        /// chevron at the trailing edge of the row.
        trigger: SubmenuTrigger,
        /// Whether the chevron paints a hover state when the cursor is
        /// over its rect. Default `false` — the chevron is decorative
        /// and only follows the row's hover. Set `true` for ChevronClick
        /// rows that want the chevron to light up independently.
        chevron_hover: bool,
    },
}

/// How a submenu opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmenuTrigger {
    /// Hovering the trigger row opens the submenu (classic menu UX).
    Hover,
    /// Clicking the chevron at the trailing edge of the row opens the
    /// submenu. Hovering the row alone does nothing.
    ChevronClick,
}

/// How the submenu panel width is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubmenuWidth {
    /// Auto-fit: width = `measure_flat(sub_items).w`. Each panel hugs
    /// its own labels (default, the natural choice).
    #[default]
    Auto,
    /// Inherit the parent panel's width — the two columns line up.
    InheritParent,
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
// DropdownViewKind
// ---------------------------------------------------------------------------

/// Template-specific per-frame data.
///
/// Two variants only:
/// - `Flat`   — vertical list of [`DropdownItem`] rows, with optional sibling
///   submenu panel; covers single-level menus *and* one-level-deep menus.
/// - `Custom` — escape hatch.
///
/// More elaborate pickers (icon grids, grouped rows, inline current-value
/// triggers) are domain-specific compositions the application assembles
/// on top of `Flat` or in a `Custom` closure.
pub enum DropdownViewKind<'a> {
    /// Single-level item list anchored below the trigger.
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

    /// Escape hatch — caller supplies draw closure; composite provides frame.
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

    /// How the dropdown picks its outer rect. Default `AutoFit` measures
    /// content; `Fixed(w, h)` pins the rect.
    pub size_mode: crate::types::SizeMode,

    /// What to do when content exceeds the panel rect. `Clip` (default)
    /// just hides; `Scrollbar` / `Chevrons` activate paging affordances.
    /// Dropdowns are never resizable / draggable — they're transient
    /// surfaces.
    pub overflow: crate::types::OverflowMode,

    /// How a submenu panel sizes its width relative to the parent.
    /// `Auto` (default) — each panel measures its own labels.
    /// `InheritParent` — sub panel matches parent panel width.
    pub submenu_width: SubmenuWidth,
}

// ---------------------------------------------------------------------------
// DropdownRenderKind  (discriminant-only for dispatch)
// ---------------------------------------------------------------------------

/// Layout / input registration strategy selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownRenderKind {
    /// Single-level item list with optional sibling submenu panel.
    Flat,
    /// Escape hatch — caller-driven draw.
    Custom,
}
