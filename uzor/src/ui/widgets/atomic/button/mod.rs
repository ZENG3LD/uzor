//! Button widget — Action / Toggle / Checkbox / Tab / ColorSwatch / Dropdown,
//! plus 10 toolbar-specific variants, 7 modal action button variants,
//! 2 inline tab-as-button variants (SidebarTab, HorizontalTab),
//! 3 radio variants (RadioGroup, RadioPair, RadioDot), and
//! 2 misc variants (CloseButton, ScrollChevron) — all 42 variants complete.
//!
//! Self-contained:
//! - `types`     — variant catalog (`ButtonType`, ActionVariant, …) + capabilities.
//! - `defaults`  — per-variant prototype defaults (kept from earlier work).
//! - `state`     — `ButtonState` placeholder + `SplitButtonHoverZone`.
//! - `theme`     — `ButtonTheme` colour trait + `DefaultButtonTheme`
//!                 (now includes toolbar + modal action + color swatch + dropdown colour slots
//!                 + selector button colour slots + radio colour slots).
//! - `style`     — `ButtonStyle` geometry trait + Default/Compact/Flat/Toolbar/Label
//!                 presets + Primary/GhostOutline/Danger/Utility modal presets
//!                 + `SidebarTabStyle` / `HorizontalTabStyle`
//!                 + `ColorSwatchStyle` / `FillToggleStyle` presets
//!                 + `SplitDropdownStyle` / `DropdownFieldStyle` / `DropdownMenuRowStyle`
//!                   presets (sections 32-33, 38)
//!                 + `SelectorButtonStyle` / `ShapeSelectorStyle` / `ThemePresetButtonStyle`
//!                   / `UIStyleSelectorStyle` presets (sections 34, 39, 40)
//!                 + `RadioStyle` / `DefaultRadioStyle` / `RadioPairStyle`
//!                   / `DefaultRadioPairStyle` (sections 35-37)
//!                 + `CloseButtonStyle` / `DefaultCloseButtonStyle` / `LargeCloseButtonStyle`
//!                   (section 41)
//!                 + `ScrollChevronStyle` / `DefaultScrollChevronStyle` (section 42).
//! - `settings`  — `ButtonSettings` bundle (theme + style).
//! - `render`    — `draw_button` + 10 `draw_toolbar_*` fns + 7 modal action fns
//!                 + `draw_sidebar_tab_button` + `draw_horizontal_tab_button`
//!                 + `draw_toggle_switch` + `draw_color_swatch` + `draw_fill_toggle`
//!                 + `draw_split_dropdown` + `draw_dropdown_field`
//!                 + `draw_dropdown_menu_row`
//!                 + `draw_shape_selector_button` + `draw_theme_preset_button`
//!                 + `draw_ui_style_button`
//!                 + `draw_radio_group` + `draw_radio_pair` + `draw_radio_dot`
//!                 + `draw_close_button` + `draw_scroll_chevron_button`
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
    // checkbox geometry presets
    CheckboxStyle,
    StandardCheckboxStyle, VisibilityCheckboxStyle,
    LevelVisibilityCheckboxStyle, NotificationCheckboxStyle,
    // toggle switch geometry presets
    ToggleSwitchStyle,
    IndicatorToggleStyle, SignalsToggleStyle,
    // color swatch geometry presets
    ColorSwatchStyle,
    SimpleSwatchStyle, IndicatorSwatchStyle, AppearanceSwatchStyle, PrimitiveSwatchStyle,
    // fill toggle geometry preset
    FillToggleStyle, PrimitiveFillToggleStyle,
    // dropdown geometry presets (sections 32-33, 38)
    SplitDropdownStyle, DefaultSplitDropdownStyle,
    DropdownFieldStyle, DefaultDropdownFieldStyle,
    DropdownMenuRowStyle, RoundedDropdownMenuRowStyle, FlatDropdownMenuRowStyle,
    // selector button geometry presets (sections 34, 39, 40)
    SelectorButtonStyle,
    ShapeSelectorStyle, ThemePresetButtonStyle, UIStyleSelectorStyle,
    // radio geometry presets (sections 35-37)
    RadioStyle, DefaultRadioStyle,
    RadioPairStyle, DefaultRadioPairStyle,
    // close button geometry presets (section 41)
    CloseButtonStyle, DefaultCloseButtonStyle, LargeCloseButtonStyle,
    // scroll chevron geometry presets (section 42)
    ScrollChevronStyle, DefaultScrollChevronStyle,
};
pub use settings::ButtonSettings;
pub use render::{
    // base
    draw_button, ButtonResult, ButtonView,
    // toolbar view structs
    ColorButtonView, LineWidthButtonView, DropdownTriggerView,
    SplitIconButtonView, SplitLineWidthButtonView, ClockView, LabelView,
    ToolbarHitRect, PanelToolbarResult, PanelToolbarItem, PanelToolbarOrientation,
    // toolbar render fns
    draw_toolbar_icon_button,
    draw_toolbar_button,
    draw_toolbar_dropdown_trigger,
    draw_toolbar_color_button,
    draw_toolbar_line_width_button,
    draw_toolbar_split_icon_button,
    draw_toolbar_split_line_width_button,
    draw_toolbar_clock,
    draw_toolbar_label,
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
    // checkbox view structs + render fns
    CheckboxView,
    draw_checkbox_standard,
    draw_checkbox_notification,
    // toggle switch view structs + render fns
    ToggleSwitchView,
    draw_toggle_switch,
    // tab-as-button view structs + render fns
    SidebarTabView,
    HorizontalTabView, HorizontalTabActiveStyle,
    draw_sidebar_tab_button,
    draw_horizontal_tab_button,
    // color swatch + fill toggle view structs + render fns
    ColorSwatchView,
    FillToggleView,
    draw_color_swatch,
    draw_fill_toggle,
    // dropdown trigger + menu row view structs + render fns (sections 32-33, 38)
    SplitDropdownView,
    DropdownFieldView,
    DropdownMenuRowView,
    draw_split_dropdown,
    draw_dropdown_field,
    draw_dropdown_menu_row,
    // selector-style button view structs + render fns (sections 34, 39, 40)
    ShapeSelectorView,
    ThemePresetView,
    UIStyleView,
    draw_shape_selector_button,
    draw_theme_preset_button,
    draw_ui_style_button,
    // radio view structs + render fns (sections 35-37)
    RadioOption,
    RadioGroupView,
    RadioPairView,
    RadioDotView,
    draw_radio_group,
    draw_radio_pair,
    draw_radio_dot,
    // close button view struct + render fn (section 41)
    CloseButtonView,
    draw_close_button,
    // scroll chevron view struct + render fn (section 42)
    ScrollChevronView,
    draw_scroll_chevron_button,
};
pub use input::register;
