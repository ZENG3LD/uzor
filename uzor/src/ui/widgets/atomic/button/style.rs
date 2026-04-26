//! Geometry parameters for button rendering.
//!
//! Numbers ported from mlc `chart/src/ui/widgets/button.rs` `ButtonConfig`
//! defaults plus the recurring values from cluster-A research:
//! `radius=4.0` dominates, `radius=3.0` for compact, `radius=0.0` for flat
//! footer buttons (Primary/Cancel).

/// Geometry trait — overridable by callers via custom `impl`.
pub trait ButtonStyle {
    /// Corner radius (mlc default 4.0).
    fn radius(&self)         -> f64;
    /// Horizontal padding (mlc 8.0).
    fn padding_x(&self)      -> f64;
    /// Vertical padding (mlc 4.0).
    fn padding_y(&self)      -> f64;
    /// Icon side length (mlc 16.0 — universal standard).
    fn icon_size(&self)      -> f64;
    /// Font size (mlc 13.0).
    fn font_size(&self)      -> f64;
    /// Gap between icon and text (mlc 6.0).
    fn gap(&self)            -> f64;
    /// Border thickness when active (mlc 1.0).
    fn border_width(&self)   -> f64;
    /// Whether the active state additionally strokes a border (mlc false).
    fn show_active_border(&self) -> bool;
}

/// Default style — values copied from mlc `ButtonConfig::default`.
pub struct DefaultButtonStyle;

impl Default for DefaultButtonStyle {
    fn default() -> Self {
        Self
    }
}

impl ButtonStyle for DefaultButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 8.0 }
    fn padding_y(&self)          -> f64  { 4.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Compact style for small inline buttons (mini delete, stepper).
/// Cluster-A research: `radius=3.0`, smaller icon/font.
pub struct CompactButtonStyle;

impl ButtonStyle for CompactButtonStyle {
    fn radius(&self)             -> f64  { 3.0 }
    fn padding_x(&self)          -> f64  { 4.0 }
    fn padding_y(&self)          -> f64  { 2.0 }
    fn icon_size(&self)          -> f64  { 12.0 }
    fn font_size(&self)          -> f64  { 11.0 }
    fn gap(&self)                -> f64  { 4.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Flat (no rounding) style for modal footer Primary / Cancel buttons.
/// Cluster-A research: `radius=0.0`.
pub struct FlatButtonStyle;

impl ButtonStyle for FlatButtonStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 6.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Toolbar button style — used for `draw_toolbar_button` and
/// `draw_toolbar_icon_button`.  Matches mlc toolbar defaults:
/// `item_size=28`, `icon_size=16`, `font_size=13`, `radius=4`, no gap padding.
pub struct ToolbarButtonStyle;

impl ButtonStyle for ToolbarButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 0.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 4.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Toolbar label style — used for `draw_toolbar_label` and
/// `draw_panel_toolbar` `draw_label`.  Matches mlc `toolbar_render.rs`:
/// `font_size=11`, no active border.
pub struct ToolbarLabelStyle;

impl ButtonStyle for ToolbarLabelStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 4.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 11.0 }
    fn gap(&self)                -> f64  { 2.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

// =============================================================================
// Modal action button styles (sections 12-18)
// =============================================================================

/// Primary/Accent modal button style (OK, Save, Create).
/// Section 12: flat (radius=0) in chart_settings, radius=4 in preset_name.
/// Using 0 for flat footer variant; callers can override via custom impl.
/// mlc: height 32, white text bold 12-13px.
pub struct PrimaryButtonStyle;

impl ButtonStyle for PrimaryButtonStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 12.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Rounded primary button style (radius=4) — preset_name_input.rs variant.
pub struct PrimaryRoundedButtonStyle;

impl ButtonStyle for PrimaryRoundedButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 12.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Ghost-outline button style (Cancel, Template, Отмена).
/// Section 13: no fill idle, stroke border, fills on hover.
/// mlc chart_settings: flat (radius=0); preset_name: radius=4.
/// Using 0 to match the dominant chart_settings pattern.
pub struct GhostOutlineButtonStyle;

impl ButtonStyle for GhostOutlineButtonStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Rounded ghost-outline button style — alert_settings / preset_name variant.
pub struct GhostOutlineRoundedButtonStyle;

impl ButtonStyle for GhostOutlineRoundedButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Danger button style (LogOut, Delete).
/// Section 14/15: rounded, height 28-30, radius=4 (LogOut) / 3 (Delete).
/// Using 4 as default (LogOut variant); Delete uses CompactButtonStyle (r=3).
pub struct DangerButtonStyle;

impl ButtonStyle for DangerButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 10.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 14.0 }
    fn font_size(&self)          -> f64  { 11.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Sidebar tab button style (section 19).
/// mlc: `tab_button_height=44`, `sidebar_width=48`, `icon_size=20`.
/// No padding/radius — rect is already full-width, background handled by
/// `draw_sidebar_active_item`.
pub struct SidebarTabStyle;

impl ButtonStyle for SidebarTabStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 0.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 20.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 0.0 }
    fn border_width(&self)       -> f64  { 0.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Horizontal tab button style (section 20).
/// mlc: `tab_bar_h=32`, `font 12px`, no border.
pub struct HorizontalTabStyle;

impl ButtonStyle for HorizontalTabStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 12.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 0.0 }
    fn show_active_border(&self) -> bool { false }
}

// =============================================================================
// Checkbox geometry presets (sections 21-24)
// =============================================================================

/// Geometry parameters for checkbox rendering.
pub trait CheckboxStyle {
    /// Outer square side length (mlc: 16.0 for standard / notification).
    fn size(&self) -> f64;
    /// Outer corner radius (mlc: 3.0 standard, 2.0 level/notification).
    fn radius(&self) -> f64;
    /// Outer border stroke width (mlc: 1.0).
    fn border_width(&self) -> f64;
    /// Checkmark stroke width (mlc: 2.0).
    fn checkmark_width(&self) -> f64;
    /// Gap between checkbox box right edge and label text (mlc: 12.0 standard / 8.0 others).
    fn label_gap(&self) -> f64;
    /// Pixel inset for checkmark bottom/top anchor points.
    /// mlc draw_checkbox (section 21): 4.0
    /// mlc indicator/primitive (sections 22-23): 3.0
    fn checkmark_inset(&self) -> f64 { 3.0 }
}

/// Standard checkbox (section 21 — chart_settings.rs `draw_checkbox`).
/// Size 16×16, radius 3.0, label gap 12.
pub struct StandardCheckboxStyle;

impl CheckboxStyle for StandardCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 3.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 12.0 }
    fn checkmark_inset(&self) -> f64 { 4.0 }
}

/// Visibility checkbox (section 22 — indicator_settings.rs).
/// Size 16×16, radius 3.0, label gap 8. Checkmark points at -3 instead of -4.
pub struct VisibilityCheckboxStyle;

impl CheckboxStyle for VisibilityCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 3.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 8.0 }
}

/// Level-visibility checkbox (section 23 — primitive_settings.rs).
/// Size 16×16, radius 2.0 (sole distinction from Visibility).
pub struct LevelVisibilityCheckboxStyle;

impl CheckboxStyle for LevelVisibilityCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 2.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 8.0 }
}

/// Notification checkbox (section 24 — alert_settings.rs `draw_toggle`).
/// Size 16×16, radius 2.0, no checkmark path — uses filled inner rect.
pub struct NotificationCheckboxStyle;

impl CheckboxStyle for NotificationCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 2.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }  // unused by notification variant
    fn label_gap(&self)       -> f64 { 8.0 }
}

// =============================================================================
// Toggle switch geometry presets (sections 25-26)
// =============================================================================

/// Geometry parameters for toggle switch rendering.
///
/// The track is a pill (rounded rect with `border-radius = height / 2`).
/// The thumb is a filled circle positioned inside the track.
pub trait ToggleSwitchStyle {
    /// Track width in pixels.
    fn track_width(&self) -> f64;
    /// Track height in pixels. `border-radius = height / 2`.
    fn track_height(&self) -> f64;
    /// Thumb radius. The thumb diameter is `radius * 2`.
    fn thumb_radius(&self) -> f64;
    /// Gap between thumb edge and the nearest track edge (inner padding).
    fn thumb_padding(&self) -> f64;
    /// Gap between the toggle track right edge and an optional label.
    fn label_gap(&self) -> f64 { 8.0 }
}

/// Indicator-param toggle (section 25 — `indicator_settings.rs` Bool param).
///
/// mlc geometry: track 44×22, thumb radius 8.0, padding 4.0.
pub struct IndicatorToggleStyle;

impl ToggleSwitchStyle for IndicatorToggleStyle {
    fn track_width(&self)  -> f64 { 44.0 }
    fn track_height(&self) -> f64 { 22.0 }
    fn thumb_radius(&self) -> f64 { 8.0 }
    fn thumb_padding(&self)-> f64 { 4.0 }
}

/// Signals-enable toggle (section 26 — signals tab in `indicator_settings.rs`).
///
/// mlc geometry: track 44×22, thumb radius 9.0 (`height/2 - 2`), padding 2.0.
pub struct SignalsToggleStyle;

impl ToggleSwitchStyle for SignalsToggleStyle {
    fn track_width(&self)  -> f64 { 44.0 }
    fn track_height(&self) -> f64 { 22.0 }
    fn thumb_radius(&self) -> f64 { 9.0 }
    fn thumb_padding(&self)-> f64 { 2.0 }
}

// =============================================================================
// Color swatch geometry presets (sections 27-30)
// =============================================================================

/// Geometry parameters for color swatch rendering.
///
/// Controls swatch square size, border/outline widths, corner radius,
/// hover-expand amount, and checkerboard tile size (for transparency variant).
pub trait ColorSwatchStyle {
    /// Side length of the swatch square in pixels.
    fn swatch_size(&self) -> f64;
    /// Corner radius. `0.0` = sharp corners, `3.0-4.0` = rounded.
    fn radius(&self) -> f64;
    /// Border stroke width drawn around the idle swatch.
    fn border_width(&self) -> f64;
    /// Extra expand (pixels on each side) for the hover-highlight rect.
    /// `0.0` means no expand. mlc indicator_settings uses `2.0` (+4 total).
    fn hover_expand(&self) -> f64;
    /// Stroke width for the selected/picker-open border.
    /// mlc indicator_settings: `2.0` when picker open, `1.0` otherwise.
    fn selected_border_width(&self) -> f64;
    /// Checkerboard tile side length (relevant only for `WithTransparency` variant).
    /// mlc appearance tab: `swatch_size / 2.0` (9.0 for 18×18 swatch).
    fn checker_tile_size(&self) -> f64 {
        self.swatch_size() / 2.0
    }
}

/// Simple color swatch style — chart_settings instrument tab (section 27).
/// 24×24, radius 4.0, 1 px border, no hover expand.
pub struct SimpleSwatchStyle;

impl ColorSwatchStyle for SimpleSwatchStyle {
    fn swatch_size(&self)          -> f64 { 24.0 }
    fn radius(&self)               -> f64 { 4.0 }
    fn border_width(&self)         -> f64 { 1.0 }
    fn hover_expand(&self)         -> f64 { 0.0 }
    fn selected_border_width(&self)-> f64 { 1.0 }
}

/// Indicator swatch style — indicator_settings Style tab (section 28).
/// 20×20, radius 0.0 (sharp), 1 px border, 2 px hover expand, 2 px selected border.
pub struct IndicatorSwatchStyle;

impl ColorSwatchStyle for IndicatorSwatchStyle {
    fn swatch_size(&self)          -> f64 { 20.0 }
    fn radius(&self)               -> f64 { 0.0 }
    fn border_width(&self)         -> f64 { 1.0 }
    fn hover_expand(&self)         -> f64 { 2.0 }
    fn selected_border_width(&self)-> f64 { 2.0 }
}

/// Appearance swatch style — appearance tab with transparency checkerboard (section 29).
/// 18×18, radius 0.0, 1 px border, no hover expand.
/// Checker tile = 9×9 (swatch_size / 2).
pub struct AppearanceSwatchStyle;

impl ColorSwatchStyle for AppearanceSwatchStyle {
    fn swatch_size(&self)          -> f64 { 18.0 }
    fn radius(&self)               -> f64 { 0.0 }
    fn border_width(&self)         -> f64 { 1.0 }
    fn hover_expand(&self)         -> f64 { 0.0 }
    fn selected_border_width(&self)-> f64 { 1.0 }
}

/// Primitive-level swatch style — primitive_settings level color (section 30).
/// Width = 16, height = variable (caller passes rect). Radius 3.0, 1 px border.
/// `swatch_size` returns the fixed width; height comes from the caller rect.
pub struct PrimitiveSwatchStyle;

impl ColorSwatchStyle for PrimitiveSwatchStyle {
    fn swatch_size(&self)          -> f64 { 16.0 }
    fn radius(&self)               -> f64 { 3.0 }
    fn border_width(&self)         -> f64 { 1.0 }
    fn hover_expand(&self)         -> f64 { 0.0 }
    fn selected_border_width(&self)-> f64 { 1.0 }
}

// =============================================================================
// Fill toggle geometry preset (section 31)
// =============================================================================

/// Geometry parameters for fill-toggle rendering.
///
/// The fill toggle is a small rounded square that shows the fill color when
/// enabled and a diagonal strikethrough when disabled.
pub trait FillToggleStyle {
    /// Square side length. mlc: 16 × `row_height - 8` (variable height).
    fn size(&self) -> f64;
    /// Corner radius. mlc: 3.0.
    fn radius(&self) -> f64;
    /// Border stroke width. mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default fill-toggle style — primitive_settings level fill (section 31).
/// Width 16, radius 3.0. Height is variable — pass it in the rect.
pub struct PrimitiveFillToggleStyle;

impl FillToggleStyle for PrimitiveFillToggleStyle {
    fn size(&self)         -> f64 { 16.0 }
    fn radius(&self)       -> f64 { 3.0 }
    fn border_width(&self) -> f64 { 1.0 }
}

/// Utility / SignIn button style (Run Setup Wizard, Create New Profile, Sign In).
/// Sections 17/18: radius=4, height 28-36, font 11-13px.
pub struct UtilityButtonStyle;

impl ButtonStyle for UtilityButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 10.0 }
    fn padding_y(&self)          -> f64  { 0.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 11.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

// =============================================================================
// Dropdown geometry presets (sections 32-33, 38)
// =============================================================================

/// Geometry for `draw_split_dropdown` (section 32).
///
/// Controls overall corner radius, text padding, the chevron zone width,
/// and font size.
pub trait SplitDropdownStyle {
    /// Corner radius.  mlc: 4.0.
    fn radius(&self) -> f64;
    /// Left text padding from field left edge.  mlc: 8.0.
    fn text_padding_x(&self) -> f64;
    /// Width of the right chevron zone (vertical separator + chevron icon).
    /// mlc: 20.0.
    fn chevron_width(&self) -> f64;
    /// Font size for the current-value label.  mlc: 13.0.
    fn font_size(&self) -> f64;
    /// Border stroke width.  mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default `SplitDropdown` style.  Matches chart_settings.rs prototype values.
pub struct DefaultSplitDropdownStyle;

impl SplitDropdownStyle for DefaultSplitDropdownStyle {
    fn radius(&self)        -> f64 { 4.0 }
    fn text_padding_x(&self)-> f64 { 8.0 }
    fn chevron_width(&self) -> f64 { 20.0 }
    fn font_size(&self)     -> f64 { 13.0 }
    fn border_width(&self)  -> f64 { 1.0 }
}

/// Geometry for `draw_dropdown_field` (section 33).
///
/// Single-zone trigger styled as a form input with an inline chevron icon.
pub trait DropdownFieldStyle {
    /// Corner radius.  mlc: 4.0.
    fn radius(&self) -> f64;
    /// Left text padding.  mlc: 8.0.
    fn text_padding_x(&self) -> f64;
    /// Chevron icon side length.  mlc: 12.0.
    fn chevron_size(&self) -> f64;
    /// Gap between chevron right edge and field right edge.  mlc: 6.0.
    fn chevron_margin_right(&self) -> f64;
    /// Font size.  mlc: 13.0.
    fn font_size(&self) -> f64;
    /// Border stroke width.  mlc: 1.0.
    fn border_width(&self) -> f64;
}

/// Default `DropdownField` style.  Matches alert_settings.rs prototype values.
pub struct DefaultDropdownFieldStyle;

impl DropdownFieldStyle for DefaultDropdownFieldStyle {
    fn radius(&self)              -> f64 { 4.0 }
    fn text_padding_x(&self)      -> f64 { 8.0 }
    fn chevron_size(&self)        -> f64 { 12.0 }
    fn chevron_margin_right(&self)-> f64 { 6.0 }
    fn font_size(&self)           -> f64 { 13.0 }
    fn border_width(&self)        -> f64 { 1.0 }
}

/// Geometry for `draw_dropdown_menu_row` (section 38).
///
/// One item inside an open dropdown menu.
pub trait DropdownMenuRowStyle {
    /// Corner radius for the hover/selected highlight rect.
    /// chart_settings variant: 3.0.  alert_settings variant: 0.0 (sharp).
    fn radius(&self) -> f64;
    /// Horizontal inset of the highlight rect from field x/width (mlc: 1.0).
    fn highlight_inset_x(&self) -> f64;
    /// Left text padding inside row.  mlc: 8.0.
    fn text_padding_x(&self) -> f64;
    /// Font size.  mlc: 13.0.
    fn font_size(&self) -> f64;
    /// Separator line height (width of the line drawn below the row).
    /// mlc: 1.0.
    fn separator_height(&self) -> f64;
}

/// Rounded `DropdownMenuRow` style — chart_settings.rs template list (radius 3.0).
pub struct RoundedDropdownMenuRowStyle;

impl DropdownMenuRowStyle for RoundedDropdownMenuRowStyle {
    fn radius(&self)           -> f64 { 3.0 }
    fn highlight_inset_x(&self)-> f64 { 1.0 }
    fn text_padding_x(&self)   -> f64 { 8.0 }
    fn font_size(&self)        -> f64 { 13.0 }
    fn separator_height(&self) -> f64 { 1.0 }
}

/// Flat `DropdownMenuRow` style — alert_settings.rs condition list (radius 0.0).
pub struct FlatDropdownMenuRowStyle;

impl DropdownMenuRowStyle for FlatDropdownMenuRowStyle {
    fn radius(&self)           -> f64 { 0.0 }
    fn highlight_inset_x(&self)-> f64 { 1.0 }
    fn text_padding_x(&self)   -> f64 { 8.0 }
    fn font_size(&self)        -> f64 { 13.0 }
    fn separator_height(&self) -> f64 { 1.0 }
}

// =============================================================================
// Selector button geometry presets (sections 34, 39, 40)
// =============================================================================

/// Geometry parameters for selector-style toggle buttons (shape, theme preset,
/// UI style).  Each button is a square/rectangle visual preview with a selected
/// outline and an optional label.
pub trait SelectorButtonStyle {
    /// Button width.  For square buttons set equal to `height()`.
    fn width(&self) -> f64;
    /// Button height.
    fn height(&self) -> f64;
    /// Corner radius.  mlc shape selector: 4.0; appearance buttons: 3.0.
    fn radius(&self) -> f64;
    /// Border stroke width for idle and hover outlines.
    fn border_width(&self) -> f64;
    /// Border stroke width for the selected outline (may be thicker).
    fn selected_border_width(&self) -> f64;
    /// Font size for the optional label drawn outside the button rect.
    fn label_font_size(&self) -> f64;
    /// Gap between button bottom edge and the label baseline.
    fn label_gap(&self) -> f64 { 4.0 }
}

/// Shape selector button style — 24×24 square (section 34).
/// mlc indicator_settings signals tab: `shape_btn_size = 24.0`.
pub struct ShapeSelectorStyle;

impl SelectorButtonStyle for ShapeSelectorStyle {
    fn width(&self)                  -> f64 { 24.0 }
    fn height(&self)                 -> f64 { 24.0 }
    fn radius(&self)                 -> f64 { 4.0 }
    fn border_width(&self)           -> f64 { 1.0 }
    fn selected_border_width(&self)  -> f64 { 1.5 }
    fn label_font_size(&self)        -> f64 { 11.0 }
}

/// Theme preset button style — appearance tab preview button (section 39).
/// mlc chart_settings appearance tab: `button_height = 26.0`, full content width.
/// Width is variable (caller sets the rect width), height 26.
pub struct ThemePresetButtonStyle;

impl SelectorButtonStyle for ThemePresetButtonStyle {
    fn width(&self)                  -> f64 { 120.0 }  // default; caller passes actual rect
    fn height(&self)                 -> f64 { 26.0 }
    fn radius(&self)                 -> f64 { 3.0 }
    fn border_width(&self)           -> f64 { 1.0 }
    fn selected_border_width(&self)  -> f64 { 1.5 }
    fn label_font_size(&self)        -> f64 { 13.0 }
}

/// UI style button style — appearance tab style selector (section 40).
/// Text-only selector, same height as ThemePreset, radius 3.
pub struct UIStyleSelectorStyle;

impl SelectorButtonStyle for UIStyleSelectorStyle {
    fn width(&self)                  -> f64 { 80.0 }  // default; caller passes actual rect
    fn height(&self)                 -> f64 { 26.0 }
    fn radius(&self)                 -> f64 { 3.0 }
    fn border_width(&self)           -> f64 { 1.0 }
    fn selected_border_width(&self)  -> f64 { 1.5 }
    fn label_font_size(&self)        -> f64 { 13.0 }
}

// =============================================================================
// Radio button geometry (sections 35-37)
// =============================================================================

/// Geometry parameters for `draw_radio_group` (section 35 — canonical group).
///
/// Dimensions ported from `mlc/chart/src/ui/widgets/radio_group.rs`.
pub trait RadioStyle {
    /// Height of each radio row.  mlc: 52.0.
    fn row_height(&self) -> f64;
    /// Vertical gap between rows.  mlc: 8.0.
    fn gap(&self) -> f64;
    /// Outer ring radius.  mlc: 7.0.
    fn outer_radius(&self) -> f64;
    /// Inner dot radius (drawn when selected).  mlc: 4.0.
    fn inner_radius(&self) -> f64;
    /// Corner radius of the hover-highlight rect behind each row.  mlc: 6.0.
    fn row_corner_radius(&self) -> f64 { 6.0 }
    /// Stroke width for the outer ring.  mlc: 1.5.
    fn ring_stroke_width(&self) -> f64 { 1.5 }
    /// Horizontal offset from `x` to the circle center.
    /// mlc: `circle_radius + 4.0`.
    fn circle_offset_x(&self) -> f64 { self.outer_radius() + 4.0 }
    /// Vertical offset from row top to circle center.  mlc: 14.0.
    fn circle_offset_y(&self) -> f64 { 14.0 }
    /// Horizontal offset from `x` to the label start.
    /// mlc: `circle_radius * 2.0 + 16.0`.
    fn label_offset_x(&self) -> f64 { self.outer_radius() * 2.0 + 16.0 }
    /// Vertical offset from row top to label baseline.  mlc: 4.0.
    fn label_offset_y(&self) -> f64 { 4.0 }
    /// Vertical offset from row top to description baseline.  mlc: 24.0.
    fn desc_offset_y(&self) -> f64 { 24.0 }
    /// Label font size.  mlc: 13.0.
    fn label_font_size(&self) -> f64 { 13.0 }
    /// Description font size.  mlc: 11.0.
    fn desc_font_size(&self) -> f64 { 11.0 }
}

/// Default radio group style — values from `mlc/radio_group.rs`.
pub struct DefaultRadioStyle;

impl RadioStyle for DefaultRadioStyle {
    fn row_height(&self)   -> f64 { 52.0 }
    fn gap(&self)          -> f64 { 8.0 }
    fn outer_radius(&self) -> f64 { 7.0 }
    fn inner_radius(&self) -> f64 { 4.0 }
}

/// Geometry parameters for `draw_radio_pair` (sections 36-37).
///
/// Used for both the button-pair toggle (36) and the inline radio dot (37).
pub trait RadioPairStyle {
    /// Circle radius for inline radio dots.  mlc: 5.0.
    fn radio_radius(&self) -> f64;
    /// Row height for inline radio rows.  mlc: 22.0.
    fn row_height(&self) -> f64;
    /// Stroke width for the outer ring.  mlc: 1.5 (user_settings) / 1.0 (profile_manager).
    fn ring_stroke_width(&self) -> f64 { 1.5 }
    /// Inner dot radius when selected (outer_r − 2.5).  mlc: 2.5.
    fn inner_dot_radius(&self) -> f64 { self.radio_radius() - 2.5 }
    /// Horizontal gap between the radio circle and its inline label.  mlc: ~6.0.
    fn label_gap(&self) -> f64 { 6.0 }
    /// Font size for the inline label.  mlc: 13.0.
    fn label_font_size(&self) -> f64 { 13.0 }
}

/// Default radio pair style — values from `mlc/profile_manager.rs` and
/// `mlc/user_settings.rs`.
pub struct DefaultRadioPairStyle;

impl RadioPairStyle for DefaultRadioPairStyle {
    fn radio_radius(&self) -> f64 { 5.0 }
    fn row_height(&self)   -> f64 { 22.0 }
}

// =============================================================================
// Close button style (section 41)
// =============================================================================

/// Geometry parameters for `draw_close_button` (section 41 — modal close X).
///
/// Ported from mlc modal patterns:
/// - `chart_settings` / `indicator_settings`: 18 × 18 px
/// - `alert_settings`: 20 × 20 px
/// - `profile_manager`: 28 × 28 px with hover bg fill
pub trait CloseButtonStyle {
    /// Side length of the square hit target.  mlc default: 18.0.
    fn size(&self) -> f64;
    /// Stroke width of each X arm.  mlc: 1.5.
    fn x_stroke_width(&self) -> f64 { 1.5 }
    /// Corner radius of the hover-bg fill rect.  mlc profile_manager: 4.0.
    fn hover_bg_radius(&self) -> f64 { 4.0 }
    /// Inset from the button edge to the X arms endpoints.  mlc: 4.0.
    fn x_inset(&self) -> f64 { 4.0 }
}

/// Default close button — matches `chart_settings` / `indicator_settings` (18 px).
pub struct DefaultCloseButtonStyle;

impl CloseButtonStyle for DefaultCloseButtonStyle {
    fn size(&self) -> f64 { 18.0 }
}

/// Larger close button — matches `profile_manager` (28 px, with hover bg).
pub struct LargeCloseButtonStyle;

impl CloseButtonStyle for LargeCloseButtonStyle {
    fn size(&self) -> f64 { 28.0 }
}

// =============================================================================
// ScrollChevron button style (section 42)
// =============================================================================

/// Geometry parameters for `draw_scroll_chevron_button` (section 42 —
/// toolbar overflow navigation).
///
/// Ported from `mlc/toolbar_core.rs` `draw_toolbar_with_icons`:
/// `chevron_size = 16.0`.
pub trait ScrollChevronStyle {
    /// Side length of the square hit target.  mlc: 16.0.
    fn size(&self) -> f64;
    /// Stroke width of the chevron arms.  mlc: ~1.5.
    fn chevron_thickness(&self) -> f64 { 1.5 }
    /// Corner radius of the hover-bg fill rect.  mlc: 4.0.
    fn hover_bg_radius(&self) -> f64 { 4.0 }
    /// Inset from the button edge used to size the chevron triangle.  mlc: 4.0.
    fn chevron_inset(&self) -> f64 { 4.0 }
}

/// Default scroll-chevron style — matches `mlc/toolbar_core.rs` (16 px).
pub struct DefaultScrollChevronStyle;

impl ScrollChevronStyle for DefaultScrollChevronStyle {
    fn size(&self) -> f64 { 16.0 }
}
