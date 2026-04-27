//! Tab geometry — generic trait + per-variant preset structs.

// ---------------------------------------------------------------------------
// Generic trait
// ---------------------------------------------------------------------------

pub trait TabStyle {
    fn radius(&self)            -> f64;
    fn padding_x(&self)         -> f64;
    fn padding_y(&self)         -> f64;
    fn font_size(&self)         -> f64;
    fn icon_size(&self)         -> f64;
    fn gap(&self)               -> f64;
    fn close_btn_size(&self)    -> f64;
    /// Active accent bar thickness (left edge of vertical tab, or bottom of chrome tab).
    fn accent_bar(&self)        -> f64;
}

// ---------------------------------------------------------------------------
// Default (generic / fallback)
// ---------------------------------------------------------------------------

pub struct DefaultTabStyle;

impl Default for DefaultTabStyle {
    fn default() -> Self {
        Self
    }
}

impl TabStyle for DefaultTabStyle {
    fn radius(&self)         -> f64 { 4.0 }
    fn padding_x(&self)      -> f64 { 12.0 }
    fn padding_y(&self)      -> f64 { 6.0 }
    fn font_size(&self)      -> f64 { 13.0 }
    fn icon_size(&self)      -> f64 { 14.0 }
    fn gap(&self)            -> f64 { 6.0 }
    fn close_btn_size(&self) -> f64 { 14.0 }
    fn accent_bar(&self)     -> f64 { 3.0 }
}

// ---------------------------------------------------------------------------
// Chrome variant
// Matches mlc: CHROME_HEIGHT=32, TAB_PADDING_H=12, TAB_CLOSE_SIZE=16,
// accent_bar=2 (bottom line rendered at CHROME_HEIGHT-3 .. CHROME_HEIGHT-1).
// ---------------------------------------------------------------------------

/// Style preset for the Chrome (browser-style) tab strip.
///
/// Geometry from mlc:
/// - `height`     = 32 px (CHROME_HEIGHT)
/// - `padding_h`  = 12 px per side (TAB_PADDING_H)
/// - `close_size` = 16 px hit zone, 14 px icon (TAB_CLOSE_SIZE)
/// - `accent_bar` = 2 px (bottom line thickness)
/// - `gap`        = 1 px between tabs (TAB_GAP)
/// - `font_size`  = 12 px
///
/// Tab intrinsic width = `padding_h + text_w + close_size + padding_h`
///                     = `12 + text_w + 16 + 12`
pub struct ChromeTabStyle {
    /// Total strip height (default 32).
    pub height: f64,
    /// Left/right padding inside each tab cell (default 12).
    pub padding_h: f64,
    /// Close-button hit-zone width (default 16); icon drawn at 14×14 inside.
    pub close_size: f64,
    /// Bottom accent line thickness for the active tab (default 2).
    pub accent_bar_thickness: f64,
    /// Gap between adjacent tabs (default 1).
    pub tab_gap: f64,
    /// Font size for tab labels (default 12).
    pub font_size: f64,
    /// Left margin before the first tab (default 4).
    pub left_margin: f64,
}

impl Default for ChromeTabStyle {
    fn default() -> Self {
        Self {
            height: 32.0,
            padding_h: 12.0,
            close_size: 16.0,
            accent_bar_thickness: 2.0,
            tab_gap: 1.0,
            font_size: 12.0,
            left_margin: 4.0,
        }
    }
}

impl TabStyle for ChromeTabStyle {
    fn radius(&self)         -> f64 { 0.0 }
    fn padding_x(&self)      -> f64 { self.padding_h }
    fn padding_y(&self)      -> f64 { 0.0 }
    fn font_size(&self)      -> f64 { self.font_size }
    fn icon_size(&self)      -> f64 { 14.0 }
    fn gap(&self)            -> f64 { self.tab_gap }
    fn close_btn_size(&self) -> f64 { self.close_size }
    /// Bottom accent bar thickness.
    fn accent_bar(&self)     -> f64 { self.accent_bar_thickness }
}

// ---------------------------------------------------------------------------
// ModalSidebar variant
// Matches mlc: sidebar_width=48, button_height=44 (or 40), accent_bar=3,
// icon_size=20, icon centered in cell.
// ---------------------------------------------------------------------------

/// Style preset for the icon-only vertical modal sidebar tab strip.
///
/// Geometry from mlc:
/// - `width`         = 48 px (fixed sidebar column width)
/// - `button_height` = 44 px (chart/indicator/user settings) or 40 px (search overlay)
/// - `icon_size`     = 20 px (centered inside button cell)
/// - `accent_bar`    = 3 px (left edge solid bar)
pub struct ModalSidebarTabStyle {
    /// Fixed sidebar column width (default 48).
    pub width: f64,
    /// Height of each tab button cell (default 44; use 40 for search overlay).
    pub button_height: f64,
    /// Icon size drawn centered in the cell (default 20).
    pub icon_size: f64,
    /// Left accent bar width for the active tab (default 3).
    pub accent_bar_width: f64,
    /// Font size if a label is shown (rare; default 11).
    pub font_size: f64,
}

impl Default for ModalSidebarTabStyle {
    fn default() -> Self {
        Self {
            width: 48.0,
            button_height: 44.0,
            icon_size: 20.0,
            accent_bar_width: 3.0,
            font_size: 11.0,
        }
    }
}

impl TabStyle for ModalSidebarTabStyle {
    fn radius(&self)         -> f64 { 0.0 }
    fn padding_x(&self)      -> f64 { 0.0 }
    fn padding_y(&self)      -> f64 { 0.0 }
    fn font_size(&self)      -> f64 { self.font_size }
    fn icon_size(&self)      -> f64 { self.icon_size }
    fn gap(&self)            -> f64 { 0.0 }
    fn close_btn_size(&self) -> f64 { 0.0 }
    fn accent_bar(&self)     -> f64 { self.accent_bar_width }
}

// ---------------------------------------------------------------------------
// ModalHorizontal variant
// Matches mlc primitive_settings: tab_height=32, tab_padding_h=12, gap=2,
// intrinsic width = text_w + padding_h * 2.
// ---------------------------------------------------------------------------

/// Style preset for the text-label horizontal tab row (e.g. primitive settings).
///
/// Geometry from mlc:
/// - `height`     = 32 px
/// - `padding_h`  = 12 px per side (tab width = `text_w + padding_h * 2`)
/// - `gap`        = 2 px between tabs
/// - `font_size`  = 13 px
/// - Active tab: filled background via `draw_active_rect`, white text.
/// - No accent bar — full rect background highlight.
pub struct ModalHorizontalTabStyle {
    /// Tab row height (default 32).
    pub height: f64,
    /// Horizontal padding per side inside each tab (default 12).
    pub padding_h: f64,
    /// Gap between adjacent tabs (default 2).
    pub tab_gap: f64,
    /// Font size for labels (default 13).
    pub font_size: f64,
}

impl Default for ModalHorizontalTabStyle {
    fn default() -> Self {
        Self {
            height: 32.0,
            padding_h: 12.0,
            tab_gap: 2.0,
            font_size: 13.0,
        }
    }
}

impl TabStyle for ModalHorizontalTabStyle {
    fn radius(&self)         -> f64 { 0.0 }
    fn padding_x(&self)      -> f64 { self.padding_h }
    fn padding_y(&self)      -> f64 { 0.0 }
    fn font_size(&self)      -> f64 { self.font_size }
    fn icon_size(&self)      -> f64 { 0.0 }
    fn gap(&self)            -> f64 { self.tab_gap }
    fn close_btn_size(&self) -> f64 { 0.0 }
    fn accent_bar(&self)     -> f64 { 0.0 }
}

// ---------------------------------------------------------------------------
// TagsTabsSidebar variant
// Matches mlc tags_tabs_modal: SIDEBAR_WIDTH=80, item_height=40,
// rounded_rect inset x+4/y+2 → 72×36, radius=4.
// ---------------------------------------------------------------------------

/// Style preset for the text-only pill sidebar (TagsTabsSidebar: Tabs / Tags / Map).
///
/// Geometry from mlc:
/// - `width`         = 80 px (fixed sidebar column)
/// - `item_height`   = 40 px
/// - `pill_inset_x`  = 4 px (pill x = sidebar_x + inset_x)
/// - `pill_inset_y`  = 2 px (pill y = item_y + inset_y)
/// - `pill_radius`   = 4 px
/// - `font_size`     = 11 px bold
/// - Active: `fill_rounded_rect` with `accent` at 0.20 alpha; text = accent.
/// - Hover: same rect with `item_text` at 0.08 alpha; text = item_text.
pub struct TagsTabsSidebarTabStyle {
    /// Fixed sidebar column width (default 80).
    pub width: f64,
    /// Height of each item cell (default 40).
    pub item_height: f64,
    /// Horizontal inset of the pill from the cell left edge (default 4).
    pub pill_inset_x: f64,
    /// Vertical inset of the pill from the cell top edge (default 2).
    pub pill_inset_y: f64,
    /// Corner radius of the pill (default 4).
    pub pill_radius: f64,
    /// Font size (default 11).
    pub font_size: f64,
    /// Alpha for the active pill background (default 0.20).
    pub active_pill_alpha: f64,
    /// Alpha for the hover pill background (default 0.08).
    pub hover_pill_alpha: f64,
}

impl Default for TagsTabsSidebarTabStyle {
    fn default() -> Self {
        Self {
            width: 80.0,
            item_height: 40.0,
            pill_inset_x: 4.0,
            pill_inset_y: 2.0,
            pill_radius: 4.0,
            font_size: 11.0,
            active_pill_alpha: 0.20,
            hover_pill_alpha: 0.08,
        }
    }
}

impl TabStyle for TagsTabsSidebarTabStyle {
    fn radius(&self)         -> f64 { self.pill_radius }
    fn padding_x(&self)      -> f64 { self.pill_inset_x }
    fn padding_y(&self)      -> f64 { self.pill_inset_y }
    fn font_size(&self)      -> f64 { self.font_size }
    fn icon_size(&self)      -> f64 { 0.0 }
    fn gap(&self)            -> f64 { 0.0 }
    fn close_btn_size(&self) -> f64 { 0.0 }
    fn accent_bar(&self)     -> f64 { 0.0 }
}
