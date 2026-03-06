//! Button type definitions and enums
//!
//! This module defines the complete button taxonomy for the trading terminal UI.
//! Coverage: 141 buttons across 6 types and 19 variants.

use crate::types::IconId;

/// Main button enum - catalog of all button types in the application
#[derive(Debug, Clone)]
pub enum ButtonType {
    /// Simple action button (click → action)
    /// Coverage: 55 buttons
    Action {
        variant: ActionVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Toggle button (click → flip ON/OFF)
    /// Coverage: 8 buttons
    Toggle {
        variant: ToggleVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Checkbox (boolean toggle with visual indicator)
    /// Coverage: 25 buttons
    Checkbox {
        variant: CheckboxVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Tab button (switches active view)
    /// Coverage: 23 buttons
    Tab {
        variant: TabVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Color swatch button (opens color picker)
    /// Coverage: 17 buttons
    ColorSwatch {
        variant: ColorSwatchVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Dropdown button (shows current value + opens menu)
    /// Coverage: 13 buttons
    Dropdown {
        variant: DropdownVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl ButtonType {
    /// Get the position of the button
    pub fn position(&self) -> (f64, f64) {
        match self {
            ButtonType::Action { position, .. } => *position,
            ButtonType::Toggle { position, .. } => *position,
            ButtonType::Checkbox { position, .. } => *position,
            ButtonType::Tab { position, .. } => *position,
            ButtonType::ColorSwatch { position, .. } => *position,
            ButtonType::Dropdown { position, .. } => *position,
        }
    }

    /// Get the width of the button
    pub fn width(&self) -> f64 {
        match self {
            ButtonType::Action { width, .. } => *width,
            ButtonType::Toggle { width, .. } => *width,
            ButtonType::Checkbox { width, .. } => *width,
            ButtonType::Tab { width, .. } => *width,
            ButtonType::ColorSwatch { width, .. } => *width,
            ButtonType::Dropdown { width, .. } => *width,
        }
    }

    /// Get the height of the button
    pub fn height(&self) -> f64 {
        match self {
            ButtonType::Action { height, .. } => *height,
            ButtonType::Toggle { height, .. } => *height,
            ButtonType::Checkbox { height, .. } => *height,
            ButtonType::Tab { height, .. } => *height,
            ButtonType::ColorSwatch { height, .. } => *height,
            ButtonType::Dropdown { height, .. } => *height,
        }
    }
}

// =============================================================================
// Action Variants
// =============================================================================

/// Action button - simple action on click
///
/// Examples:
/// - IconOnly: delete (47 buttons) - Close, Delete, Settings, Alert, Lock, Add, More
/// - Text: "Шаблон", "Отмена", "Применить" (5 buttons)
/// - IconText: "OK" with icon (3 buttons)
/// - LineText: Line width selector (1 button) - ━━━ 2
/// - CheckboxText: Theme/UI selection (7 buttons) - "Dark" with checkbox
#[derive(Debug, Clone)]
pub enum ActionVariant {
    /// Icon only
    /// Coverage: 47 buttons (including Close: 5, Delete: 10+, Settings: 5+, Alert: 5+, Lock: 3+)
    IconOnly {
        icon: IconId,
        disabled: bool,
    },

    /// Text only
    /// Coverage: 5 buttons
    /// Examples: "Шаблон", "Отмена", "Применить ко всем", "По умолчанию"
    Text {
        text: String,
        style: ButtonStyle,
        disabled: bool,
    },

    /// Icon + text
    /// Coverage: 3 buttons
    /// Examples: "OK" with icon (Primary style)
    IconText {
        icon: IconId,
        text: String,
        style: ButtonStyle,
        disabled: bool,
    },

    /// Line + text (visual preview of line thickness)
    /// Coverage: 1 button
    /// Example: ━━━ 2 (in inline primitive toolbar)
    LineText {
        line_width: u32,  // 1-10
        text: String,
        style: ButtonStyle,
        disabled: bool,
    },

    /// Checkbox + text (for Radio/Theme selection)
    /// Coverage: 7 buttons
    /// Examples: Theme selection (Dark/Light/Custom), UI style selection
    CheckboxText {
        checkbox_checked: bool,
        text: String,
        style: ButtonStyle,
        disabled: bool,
    },
}

/// Button style for action buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Normal button (border + transparent bg)
    Default,

    /// Accent button (filled background)
    Primary,

    /// Dangerous action (red hover)
    Danger,

    /// Transparent button (no border, only hover)
    Ghost,
}

// =============================================================================
// Toggle Variants
// =============================================================================

/// Toggle button - switches ON/OFF
///
/// Examples:
/// - IconSwap: Eye/EyeOff (visibility), Lock/Unlock (6 buttons)
/// - Switch: iOS-style toggle (0 buttons - reserve)
/// - ButtonToggle: Lock toggle with blue background (2 buttons)
#[derive(Debug, Clone)]
pub enum ToggleVariant {
    /// Icon swap toggle - changes icon WITHOUT active background
    /// Coverage: 6 buttons
    /// Examples: Eye/EyeOff (visibility in Object Tree, Indicator Overlay)
    IconSwap {
        icon_off: IconId,
        icon_on: IconId,
        toggled: bool,
    },

    /// Switch toggle - iOS-style oval track + ball
    /// Coverage: 0 buttons (reserve for future)
    /// Expected: Settings toggles, feature flags
    Switch {
        toggled: bool,
        label: Option<String>,
    },

    /// Button toggle - full button with active background
    /// Coverage: 2 buttons
    /// Examples: Lock toggle with blue background, toolbar filter buttons
    ButtonToggle {
        content: ButtonContent,
        toggled: bool,
        show_active_border: bool,
    },
}

/// Content for button toggle (icon and/or text)
#[derive(Debug, Clone)]
pub struct ButtonContent {
    pub text: Option<String>,
    pub icon: Option<IconId>,
    pub style: ButtonStyle,
}

// =============================================================================
// Checkbox Variants
// =============================================================================

/// Checkbox - boolean toggle with visual checkmark
///
/// All 25 buttons in Settings modal (Instrument, Scales, Status Line tabs)
#[derive(Debug, Clone)]
pub enum CheckboxVariant {
    /// Standard checkbox with checkmark
    /// Coverage: 25 buttons
    /// Examples: use_prev_close, body_enabled, show_grid, legend_show_ohlc
    Standard {
        checked: bool,
    },

    /// Checkbox with cross mark (reserve)
    /// Coverage: 0 buttons
    Cross {
        checked: bool,
    },

    /// Checkbox with circle (reserve)
    /// Coverage: 0 buttons
    Circle {
        checked: bool,
    },
}

// =============================================================================
// Tab Variants
// =============================================================================

/// Tab button - switches active view
///
/// Examples:
/// - Vertical: Sidebar tabs in Settings, Search overlay, Indicator modal (20 buttons)
/// - Horizontal: Primitive Settings tabs (Стиль, Координаты, Видимость) (3 buttons)
#[derive(Debug, Clone)]
pub enum TabVariant {
    /// Vertical tab - left bar indicator + active background
    /// Coverage: 20 buttons
    /// Examples: Settings tabs (7), Search category filters (8), Indicator tabs (5)
    Vertical {
        label: Option<String>,
        icon: Option<IconId>,
        active: bool,
    },

    /// Horizontal tab - bottom underline (or active background)
    /// Coverage: 3 buttons
    /// Examples: Primitive Settings tabs (Стиль, Координаты, Видимость)
    Horizontal {
        label: Option<String>,
        icon: Option<IconId>,
        active: bool,
    },
}

// =============================================================================
// ColorSwatch Variants
// =============================================================================

/// Color swatch button - opens color picker
///
/// Examples:
/// - Square: body_up_color, body_down_color, border_up_color (15 buttons)
/// - IconWithBar: ColorFill icon with color bar below (2 buttons)
#[derive(Debug, Clone)]
pub enum ColorSwatchVariant {
    /// Color square (most common)
    /// Coverage: 15 buttons
    /// Examples: body_up_color, body_down_color, border_up_color, wick_up_color,
    ///          crosshair_line_color, watermark_color
    Square {
        color: String,  // CSS hex color
    },

    /// Icon + color bar below
    /// Coverage: 2 buttons
    /// Examples: Inline primitive toolbar (ColorFill icon, TextColor icon)
    IconWithBar {
        icon: IconId,
        color: String,
    },

    /// Color square + label (reserve)
    /// Coverage: 0 buttons
    SwatchWithLabel {
        color: String,
        label: String,
    },
}

// =============================================================================
// Dropdown Variants
// =============================================================================

/// Dropdown button - shows current value + opens menu
///
/// Examples:
/// - TextChevron: "Не задано" + chevron (10-12 buttons) - MAIN pattern
/// - ChevronOnly: Chevron up for indicator overlay (1 button)
#[derive(Debug, Clone)]
pub enum DropdownVariant {
    /// Text left (body cycles) + Chevron right (opens menu)
    /// Coverage: 10-12 buttons
    /// This is the MAIN dropdown pattern in the application
    /// Examples: Settings dropdowns (time_zone, session_type, bar_style, etc.)
    ///
    /// Visual: [Text.................|▼]
    /// - Click on body (left part) → cycles values if cycle_on_click=true
    /// - Click on chevron (right part) → opens menu
    TextChevron {
        current_label: String,
        cycle_on_click: bool,  // true = body cycles values
    },

    /// Text only, no chevron (rare)
    /// Coverage: 0 buttons (reserve)
    Text {
        current_label: String,
        cycle_on_click: bool,
    },

    /// Icon + Text + Chevron (rare)
    /// Coverage: 0 buttons (reserve)
    IconTextChevron {
        current_icon: IconId,
        current_label: String,
        cycle_on_click: bool,
    },

    /// Icon + Chevron only (rare)
    /// Coverage: 0 buttons (reserve)
    IconChevron {
        current_icon: IconId,
        cycle_on_click: bool,
    },

    /// Chevron only (indicator overlay close button)
    /// Coverage: 1 button
    /// Example: Chevron up to collapse indicator overlay
    ChevronOnly {
        direction: ChevronDirection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChevronDirection {
    Up,
    Down,
    Left,
    Right,
}
