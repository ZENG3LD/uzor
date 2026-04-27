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
// DropdownMenuRow geometry (section 38) — kept here pending composite Dropdown widget
// =============================================================================

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
    fn radius(&self)            -> f64 { 3.0 }
    fn highlight_inset_x(&self) -> f64 { 1.0 }
    fn text_padding_x(&self)    -> f64 { 8.0 }
    fn font_size(&self)         -> f64 { 13.0 }
    fn separator_height(&self)  -> f64 { 1.0 }
}

/// Flat `DropdownMenuRow` style — alert_settings.rs condition list (radius 0.0).
pub struct FlatDropdownMenuRowStyle;

impl DropdownMenuRowStyle for FlatDropdownMenuRowStyle {
    fn radius(&self)            -> f64 { 0.0 }
    fn highlight_inset_x(&self) -> f64 { 1.0 }
    fn text_padding_x(&self)    -> f64 { 8.0 }
    fn font_size(&self)         -> f64 { 13.0 }
    fn separator_height(&self)  -> f64 { 1.0 }
}

// CloseButtonStyle and ScrollChevronStyle have been extracted to:
//   atomic::close_button::style
//   atomic::scroll_chevron::style
// Backward-compat re-exports live in button/mod.rs.
