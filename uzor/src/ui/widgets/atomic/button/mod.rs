//! Button widget — Action / Tab / ColorSwatch / Dropdown,
//! plus 10 toolbar-specific variants, 7 modal action button variants,
//! 2 inline tab-as-button variants (SidebarTab, HorizontalTab).
//!
//! Toggle, Checkbox, and Radio have been extracted to their own atomic modules:
//! `atomic::toggle`, `atomic::checkbox`, `atomic::radio`.
//!
//! ColorSwatch + FillToggle extracted to `atomic::color_swatch`.
//! DropdownTrigger (Split + Field) extracted to `atomic::dropdown_trigger`.
//! ShapeSelector (Shape, ThemePreset, UIStyle) extracted to `atomic::shape_selector`.
//! CloseButton extracted to `atomic::close_button`.
//! ScrollChevron extracted to `atomic::scroll_chevron`.
//! Clock (draw_toolbar_clock) extracted to `atomic::clock`.
//! Item (draw_toolbar_label) extracted to `atomic::item`.
//! DropdownMenuRow remains here pending composite Dropdown widget.
//!
//! Self-contained:
//! - `types`     — variant catalog (`ButtonType`, ActionVariant, …) + capabilities.
//! - `defaults`  — per-variant prototype defaults.
//! - `state`     — `ButtonState` placeholder + `SplitButtonHoverZone`.
//! - `theme`     — `ButtonTheme` colour trait + `DefaultButtonTheme`
//!                 (toolbar + modal action + color swatch + dropdown + selector colour slots).
//! - `style`     — `ButtonStyle` geometry trait + Default/Compact/Flat/Toolbar/Label
//!                 presets + Primary/GhostOutline/Danger/Utility modal presets
//!                 + `SidebarTabStyle` / `HorizontalTabStyle`
//!                 + `DropdownMenuRowStyle` presets (section 38).
//! - `settings`  — `ButtonSettings` bundle (theme + style).
//! - `render`    — `draw_button` + toolbar fns + modal action fns + tab fns
//!                 + `draw_dropdown_menu_row`
//!                 + view structs.
//! - `input`     — `register` helper for `InputCoordinator`.

pub mod types;
pub mod defaults;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{
    ButtonType, ActionVariant, ToggleVariant, CheckboxVariant, TabVariant,
    ColorSwatchVariant, DropdownVariant, ButtonStyle as ButtonStyleEnum,
    ButtonContent, ChevronDirection,
};
pub use state::{ButtonState, SplitButtonHoverZone};
pub use theme::{ButtonTheme, DefaultButtonTheme};
pub use style::{
    ButtonStyle, DefaultButtonStyle, CompactButtonStyle, FlatButtonStyle,
    ToolbarButtonStyle, ToolbarLabelStyle,
    // modal action button styles
    PrimaryButtonStyle, PrimaryRoundedButtonStyle,
    GhostOutlineButtonStyle, GhostOutlineRoundedButtonStyle,
    DangerButtonStyle, UtilityButtonStyle,
    // tab-as-button styles
    SidebarTabStyle, HorizontalTabStyle,
    // dropdown menu row geometry (section 38) — stays in button until composite Dropdown
    DropdownMenuRowStyle, RoundedDropdownMenuRowStyle, FlatDropdownMenuRowStyle,
};
pub use settings::ButtonSettings;
pub use render::{
    // base
    draw_button, ButtonResult, ButtonView,
    // toolbar view structs
    ColorButtonView, LineWidthButtonView, DropdownTriggerView,
    SplitIconButtonView, SplitLineWidthButtonView,
    ToolbarHitRect, PanelToolbarResult, PanelToolbarItem, PanelToolbarOrientation,
    // toolbar render fns
    draw_toolbar_icon_button,
    draw_toolbar_button,
    draw_toolbar_dropdown_trigger,
    draw_toolbar_color_button,
    draw_toolbar_line_width_button,
    draw_toolbar_split_icon_button,
    draw_toolbar_split_line_width_button,
    draw_panel_toolbar,
    // modal action button view structs
    PrimaryButtonView,
    GhostOutlineButtonView,
    DangerVariant, DangerButtonView,
    SecondaryNeutralButtonView,
    SignInButtonView,
    UtilityButtonView,
    // modal action button render fns
    draw_primary_button,
    draw_ghost_outline_button,
    draw_danger_button,
    draw_secondary_neutral_button,
    draw_signin_button,
    draw_utility_button,
    // tab-as-button view structs + render fns
    SidebarTabView,
    HorizontalTabView, HorizontalTabActiveStyle,
    draw_sidebar_tab_button,
    draw_horizontal_tab_button,
    // dropdown menu row (section 38) — stays in button until composite Dropdown
    DropdownMenuRowView,
    draw_dropdown_menu_row,
};
pub use input::register;

// Re-exports from extracted atomic modules (backward-compat surface).
// New code should import directly from atomic::color_swatch, etc.
pub use super::color_swatch::{
    ColorSwatchStyle, ColorSwatchTheme, ColorSwatchView, FillToggleView,
    SimpleSwatchStyle, IndicatorSwatchStyle, AppearanceSwatchStyle, PrimitiveSwatchStyle,
    FillToggleStyle, PrimitiveFillToggleStyle,
    draw_color_swatch, draw_fill_toggle,
};
pub use super::dropdown_trigger::{
    SplitDropdownView, DropdownFieldView,
    SplitDropdownStyle, DefaultSplitDropdownStyle,
    DropdownFieldStyle, DefaultDropdownFieldStyle,
    draw_split_dropdown, draw_dropdown_field,
};
pub use super::shape_selector::{
    ShapeSelectorView, ThemePresetView, UIStyleView,
    SelectorButtonStyle, ShapeSelectorStyle, ThemePresetButtonStyle, UIStyleSelectorStyle,
    draw_shape_selector_button, draw_theme_preset_button, draw_ui_style_button,
};

// Re-exports from extracted atomic modules (backward-compat surface).
// New code should import directly from atomic::close_button, etc.
pub use super::close_button::{
    CloseButtonStyle, DefaultCloseButtonStyle, LargeCloseButtonStyle,
    CloseButtonTheme, DefaultCloseButtonTheme,
    CloseButtonView, CloseButtonResult,
    draw_close_button,
};
pub use super::scroll_chevron::{
    ScrollChevronStyle, DefaultScrollChevronStyle,
    ScrollChevronTheme, DefaultScrollChevronTheme,
    ScrollChevronView, ScrollChevronResult, ChevronDirection as ScrollChevronDirection,
    draw_scroll_chevron,
};
pub use super::clock::{
    ClockView, ClockRenderKind,
    ClockTheme, DefaultClockTheme,
    draw_clock,
};
pub use super::item::{
    ItemView, ItemRenderKind,
    ItemTheme, DefaultItemTheme,
    draw_item,
};
