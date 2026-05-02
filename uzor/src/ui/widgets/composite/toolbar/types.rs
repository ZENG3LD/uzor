//! Toolbar type definitions — per-frame view data and render kind enum.
//!
//! Ported from the mlc deep audit in `toolbar-deep.md`.
//! Five structurally-distinct templates cover all mlc Toolbar variants.

use crate::core::types::icon::IconId;
use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::ToolbarSettings;
use super::state::ToolbarState;

// ---------------------------------------------------------------------------
// SplitButtonHoverZone
// ---------------------------------------------------------------------------

/// Which sub-zone of a split button the pointer is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitButtonHoverZone {
    /// Main icon / preview area.
    Main,
    /// Chevron / expand area.
    Chevron,
}

// ---------------------------------------------------------------------------
// ToolbarItem
// ---------------------------------------------------------------------------

/// A single item in a toolbar section.
pub enum ToolbarItem<'a> {
    /// Icon-only clickable button.
    IconButton {
        /// Stable string id returned on click.
        id: &'a str,
        /// Icon to display.
        icon: &'a IconId,
        /// Whether the button is in an active/toggled state.
        active: bool,
        /// Tooltip text shown on hover.
        tooltip: Option<&'a str>,
    },

    /// Text-only clickable button.
    TextButton {
        /// Stable string id.
        id: &'a str,
        /// Display text.
        text: &'a str,
        /// Whether the button is in an active/toggled state.
        active: bool,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Button with both icon and text label.
    IconTextButton {
        /// Stable string id.
        id: &'a str,
        /// Icon to display.
        icon: &'a IconId,
        /// Display text.
        text: &'a str,
        /// Whether the button is active.
        active: bool,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Dropdown trigger — shows a label + optional icon + chevron.
    /// Opens a separate dropdown composite when clicked.
    Dropdown {
        /// Stable string id.
        id: &'a str,
        /// Display label.
        label: &'a str,
        /// Optional icon.
        icon: Option<&'a IconId>,
        /// Currently selected value displayed in the trigger.
        current: Option<&'a str>,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Color swatch button — shows a filled square of the given RGBA color.
    ColorButton {
        /// Stable string id.
        id: &'a str,
        /// RGBA color `[r, g, b, a]` (0–255 each).
        color: [u8; 4],
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Line width preview button — renders a horizontal line of the given width.
    LineWidthButton {
        /// Stable string id.
        id: &'a str,
        /// Line width in pixels (1–8 typical).
        width: u32,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Split icon button: main press area + chevron expand area.
    SplitIconButton {
        /// Stable string id (chevron zone appends `:chevron`).
        id: &'a str,
        /// Icon shown in the main area.
        icon: &'a IconId,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Split line-width button: main press area + chevron expand area.
    SplitLineWidthButton {
        /// Stable string id (chevron zone appends `:chevron`).
        id: &'a str,
        /// Line width in pixels.
        width: u32,
        /// Tooltip text.
        tooltip: Option<&'a str>,
    },

    /// Real-time clock display (read-only, hover reveals detail).
    Clock {
        /// Stable string id.
        id: &'a str,
        /// Pre-formatted time string (e.g. `"14:32:09"`).
        time_text: &'a str,
    },

    /// Non-interactive text label.
    Label {
        /// Stable string id.
        id: &'a str,
        /// Display text.
        text: &'a str,
    },

    /// Horizontal (Horizontal toolbar) or vertical (Vertical toolbar) divider line.
    Separator,

    /// Fixed-width (horizontal) or fixed-height (vertical) empty gap.
    Spacer {
        /// Gap size in pixels along the layout axis.
        width: f64,
    },

    /// Escape hatch — caller provides the draw closure.
    Custom {
        /// Stable string id.
        id: &'a str,
        /// Draw closure called with `(ctx, item_rect)`.
        draw: Box<dyn Fn(&mut dyn RenderContext, Rect) + 'a>,
    },
}

// ---------------------------------------------------------------------------
// ToolbarSection
// ---------------------------------------------------------------------------

/// An ordered slice of toolbar items.
pub struct ToolbarSection<'a> {
    /// Items in layout order.
    pub items: &'a [ToolbarItem<'a>],
}

impl<'a> ToolbarSection<'a> {
    /// Empty section — useful for optional start/center/end slots.
    pub const fn empty() -> Self {
        Self { items: &[] }
    }

    /// Returns `true` when the section has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ToolbarView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_input_coordinator_toolbar` /
/// `register_context_manager_toolbar`.
///
/// # Layout
///
/// - `Horizontal` / `Inline`: `start` left-aligned, `center` centred,
///   `end` right-aligned within the bar.
/// - `Vertical`: `start` top-aligned, `center` centred, `end` bottom-aligned.
/// - `ChromeStrip`: ignores `start`/`center`/`end`; reads `chrome` instead.
pub struct ToolbarView<'a> {
    /// Left / top section.
    pub start: ToolbarSection<'a>,
    /// Center section (optional — empty by default).
    pub center: ToolbarSection<'a>,
    /// Right / bottom section.
    pub end: ToolbarSection<'a>,
    /// ChromeStrip-specific data.  Ignored by non-Chrome kinds.
    pub chrome: Option<ChromeStripView<'a>>,
    /// What to do when items overflow the bar's main axis.
    /// Default `Clip` matches legacy behaviour.
    pub overflow: crate::types::OverflowMode,
    /// Allow user to drag the inner edge to resize toolbar thickness.
    /// `false` (default) keeps the toolbar at its measured thickness.
    /// The host is expected to clamp the resulting size — typical cap is
    /// 10% of the viewport on the toolbar's main axis.
    pub resizable: bool,
}

impl<'a> ToolbarView<'a> {
    /// Convenience constructor for Horizontal / Vertical / Inline toolbars
    /// (no `center`, no `chrome`).
    pub fn simple(start: ToolbarSection<'a>, end: ToolbarSection<'a>) -> Self {
        Self {
            start,
            center: ToolbarSection::empty(),
            end,
            chrome: None,
            overflow: crate::types::OverflowMode::Clip,
            resizable: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ChromeStripView
// ---------------------------------------------------------------------------

/// Descriptor for one tab in a ChromeStrip.
pub struct TabConfig<'a> {
    /// Stable string id.
    pub id: &'a str,
    /// Display name shown in the tab.
    pub label: &'a str,
    /// Whether this tab is currently selected.
    pub active: bool,
    /// Whether a close-X button is shown on this tab.
    pub closable: bool,
}

/// Per-frame data for the `ChromeStrip` kind.
///
/// ChromeStrip has a distinct layout from section-based toolbars:
/// - Left: tabs + optional `+` new-tab button.
/// - Middle: caption drag zone.
/// - Right: window-control buttons (minimize / maximize / close).
pub struct ChromeStripView<'a> {
    /// Tab descriptors in display order.
    pub tabs: &'a [TabConfig<'a>],
    /// Whether the `+` new-tab button is shown after the last tab.
    pub show_new_tab_button: bool,
    /// Whether the center gap registers as a window-drag zone.
    pub drag_zone: bool,
    /// Whether minimize / maximize / close buttons are shown on the right.
    pub window_controls: bool,
    /// Index of the currently hovered tab (`None` = none).
    pub hovered_tab: Option<usize>,
}

// ---------------------------------------------------------------------------
// ToolbarRenderKind
// ---------------------------------------------------------------------------

/// Selects which layout pipeline the toolbar composite runs.
pub enum ToolbarRenderKind {
    /// Full-width horizontal strip (Top / Bottom).
    ///
    /// Items flow left→right within each section.  Height is fixed by
    /// `style.height()`.
    Horizontal,

    /// Vertical column (Left / Right sidebar).
    ///
    /// Items stack top→bottom.  Width is fixed by `style.width()`.
    Vertical,

    /// Window titlebar variant with tabs + drag zone + window controls.
    ///
    /// Uses `view.chrome` instead of `view.start/center/end`.
    ChromeStrip,

    /// Small embedded toolbar inside a panel or modal.
    ///
    /// Identical to `Horizontal` but uses smaller default sizes from
    /// `InlineToolbarStyle`.
    Inline,

    /// Escape hatch — caller provides the full renderer.
    ///
    /// The composite skips all layout; the closure receives `(ctx, rect, state, settings)`.
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &ToolbarState, &ToolbarSettings)>),
}
