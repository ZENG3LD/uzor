//! Default parameters for button rendering
//!
//! ⚠️  **ВАЖНО / IMPORTANT** ⚠️
//!
//! ЭТО ЭКСПОРТНЫЕ ВАРИАНТЫ ДЛЯ БЫСТРОГО ПРОТОТИПИРОВАНИЯ НА ОСНОВЕ ЕНУМА!
//! THIS IS FOR QUICK PROTOTYPING BASED ON THE ENUM CATALOG!
//!
//! Для конечной production реализации используйте КАСТОМНЫЕ ПАРАМЕТРЫ.
//! For final production implementation, use CUSTOM PARAMETERS.
//!
//! Терминал (layout/render_ui.rs) НЕ использует эти дефолты - там кастомные значения.
//! Terminal (layout/render_ui.rs) does NOT use these defaults - it has custom values.
//!
//! # Назначение / Purpose
//!
//! This module provides default sizes and prototype colors for each button variant.
//! Prototype colors are fallback values used when theme is unavailable.
//!
//! # Архитектура / Architecture
//!
//! ```
//! Enum (types.rs) → Defaults (THIS FILE) → Render (render.rs) → Production (custom)
//!     ↑ WHAT            ↑ Fallback params    ↑ Quick proto      ↑ Full control
//! ```

// =============================================================================
// Action::IconOnly Defaults
// =============================================================================

/// Default sizes for IconOnly action buttons
pub struct IconOnlyDefaults {
    pub icon_size: f64,           // 16.0px (universal standard)
    pub padding: f64,             // 0.0px (no padding, icon only)
    pub border_radius: f64,       // 0.0px (transparent background)
    pub hover_bg_radius: f64,     // 4.0px (if hover background shown)
    pub spacing_from_edge: f64,   // 12.0px (distance from container edge)
    pub icon_gap: f64,            // 4.0-8.0px (gap between sequential icons)
}

impl Default for IconOnlyDefaults {
    fn default() -> Self {
        Self {
            icon_size: 16.0,
            padding: 0.0,
            border_radius: 0.0,
            hover_bg_radius: 4.0,
            spacing_from_edge: 12.0,
            icon_gap: 6.0,
        }
    }
}

/// Prototype colors for IconOnly action buttons (fallback when theme unavailable)
pub struct IconOnlyPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "transparent"
    pub icon_normal: &'static str,        // "#787b86" (muted gray)

    // Hover state
    pub background_hover: &'static str,   // "transparent" or "#2a2a2a"
    pub icon_hover: &'static str,         // "#e5e7eb" (bright gray)

    // Active state (for toggle-like behavior)
    pub background_active: &'static str,  // "#1e3a5f" (blue-tinted)
    pub icon_active: &'static str,        // "#ffffff" (bright white)

    // Disabled state
    pub icon_disabled: &'static str,      // "#4a4a4a" (very dim)
}

impl Default for IconOnlyPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "transparent",
            icon_normal: "#787b86",
            background_hover: "transparent",
            icon_hover: "#e5e7eb",
            background_active: "#1e3a5f",
            icon_active: "#ffffff",
            icon_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Action::Text Defaults
// =============================================================================

/// Default sizes for Text action buttons
pub struct TextDefaults {
    pub height: f64,          // 28.0px (standard button height)
    pub min_width: f64,       // 70.0px (minimum button width)
    pub padding_x: f64,       // 12.0px (horizontal text padding)
    pub padding_y: f64,       // 4.0px (vertical text padding)
    pub font_size: f64,       // 13.0px (text size)
    pub border_radius: f64,   // 4.0px (rounded corners)
    pub border_width: f64,    // 1.0px (border stroke)
    pub button_gap: f64,      // 8.0px (gap between buttons)
}

impl Default for TextDefaults {
    fn default() -> Self {
        Self {
            height: 28.0,
            min_width: 70.0,
            padding_x: 12.0,
            padding_y: 4.0,
            font_size: 13.0,
            border_radius: 4.0,
            border_width: 1.0,
            button_gap: 8.0,
        }
    }
}

/// Prototype colors for Text action buttons
pub struct TextPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "transparent"
    pub text_normal: &'static str,        // "#d1d5db" (standard text)
    pub border_normal: &'static str,      // "#3a3a3a" (separator)

    // Hover state
    pub background_hover: &'static str,   // "#2a2a2a" (subtle hover)
    pub text_hover: &'static str,         // "#d1d5db" (same as normal)
    pub border_hover: &'static str,       // "#e5e7eb" (brighter border)

    // Pressed state
    pub background_pressed: &'static str, // "#1e3a5f" (blue-tinted)
    pub text_pressed: &'static str,       // "#ffffff" (bright white)

    // Disabled state
    pub background_disabled: &'static str, // "#2a2a2a" (dim)
    pub text_disabled: &'static str,      // "#4a4a4a" (very dim)
    pub border_disabled: &'static str,    // "#3a3a3a" (separator)
}

impl Default for TextPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "transparent",
            text_normal: "#d1d5db",
            border_normal: "#3a3a3a",
            background_hover: "#2a2a2a",
            text_hover: "#d1d5db",
            border_hover: "#e5e7eb",
            background_pressed: "#1e3a5f",
            text_pressed: "#ffffff",
            background_disabled: "#2a2a2a",
            text_disabled: "#4a4a4a",
            border_disabled: "#3a3a3a",
        }
    }
}

// =============================================================================
// Action::IconText Defaults
// =============================================================================

/// Default sizes for IconText action buttons
pub struct IconTextDefaults {
    pub height: f64,          // 28.0px (standard button height)
    pub min_width: f64,       // 70.0px (minimum button width)
    pub padding_x: f64,       // 8.0px (horizontal padding)
    pub padding_y: f64,       // 4.0px (vertical padding)
    pub icon_size: f64,       // 16.0px (standard icon)
    pub icon_text_gap: f64,   // 6.0px (gap between icon and text)
    pub font_size: f64,       // 13.0px (text size)
    pub border_radius: f64,   // 4.0px (rounded corners)
}

impl Default for IconTextDefaults {
    fn default() -> Self {
        Self {
            height: 28.0,
            min_width: 70.0,
            padding_x: 8.0,
            padding_y: 4.0,
            icon_size: 16.0,
            icon_text_gap: 6.0,
            font_size: 13.0,
            border_radius: 4.0,
        }
    }
}

/// Prototype colors for IconText action buttons
pub struct IconTextPrototypeColors {
    // Primary style (default for IconText)
    pub background_normal: &'static str,  // "#2962ff" (primary blue)
    pub background_hover: &'static str,   // "#4080ff" (lighter blue)
    pub text_normal: &'static str,        // "#ffffff" (white)
    pub icon_normal: &'static str,        // "#ffffff" (white)

    // Disabled state
    pub background_disabled: &'static str, // "#2a2a2a" (dim)
    pub text_disabled: &'static str,      // "#4a4a4a" (very dim)
    pub icon_disabled: &'static str,      // "#4a4a4a" (very dim)
}

impl Default for IconTextPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "#2962ff",
            background_hover: "#4080ff",
            text_normal: "#ffffff",
            icon_normal: "#ffffff",
            background_disabled: "#2a2a2a",
            text_disabled: "#4a4a4a",
            icon_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Action::LineText Defaults
// =============================================================================

/// Default sizes for LineText action buttons
pub struct LineTextDefaults {
    pub width: f64,               // 36.0px (wider than standard to fit line)
    pub height: f64,              // 28.0px (standard toolbar button)
    pub line_length_ratio: f64,   // 0.6 (line length = width * 0.6)
    pub line_offset_y: f64,       // -8.0px (line above center)
    pub number_offset_y: f64,     // 6.0px (number below line)
    pub number_font_size: f64,    // 11.0px (small number text)
    pub border_radius: f64,       // 4.0px (rounded corners)
}

impl Default for LineTextDefaults {
    fn default() -> Self {
        Self {
            width: 36.0,
            height: 28.0,
            line_length_ratio: 0.6,
            line_offset_y: -8.0,
            number_offset_y: 6.0,
            number_font_size: 11.0,
            border_radius: 4.0,
        }
    }
}

/// Prototype colors for LineText action buttons
pub struct LineTextPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "transparent"
    pub line_normal: &'static str,        // "#787b86" (muted gray)
    pub text_normal: &'static str,        // "#787b86" (muted gray)

    // Hover state
    pub background_hover: &'static str,   // "#2a2a2a" (subtle hover)
    pub line_hover: &'static str,         // "#ffffff" (bright white)
    pub text_hover: &'static str,         // "#ffffff" (bright white)

    // Active state (selected width)
    pub background_active: &'static str,  // "#2962ff" (accent blue)
    pub line_active: &'static str,        // "#ffffff" (bright white)
    pub text_active: &'static str,        // "#ffffff" (bright white)

    // Pressed state
    pub background_pressed: &'static str, // "#1e3a5f" (blue-tinted)
    pub line_pressed: &'static str,       // "#ffffff" (bright white)
    pub text_pressed: &'static str,       // "#ffffff" (bright white)

    // Disabled state
    pub background_disabled: &'static str, // "transparent"
    pub line_disabled: &'static str,      // "#4a4a4a" (very dim)
    pub text_disabled: &'static str,      // "#4a4a4a" (very dim)
}

impl Default for LineTextPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "transparent",
            line_normal: "#787b86",
            text_normal: "#787b86",
            background_hover: "#2a2a2a",
            line_hover: "#ffffff",
            text_hover: "#ffffff",
            background_active: "#2962ff",
            line_active: "#ffffff",
            text_active: "#ffffff",
            background_pressed: "#1e3a5f",
            line_pressed: "#ffffff",
            text_pressed: "#ffffff",
            background_disabled: "transparent",
            line_disabled: "#4a4a4a",
            text_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Action::CheckboxText Defaults
// =============================================================================

/// Default sizes for CheckboxText action buttons
pub struct CheckboxTextDefaults {
    pub height: f64,             // 28.0px (standard button height)
    pub padding_x: f64,          // 8.0px (horizontal padding)
    pub padding_y: f64,          // 4.0px (vertical padding)
    pub checkbox_size: f64,      // 16.0px (checkbox square)
    pub checkbox_text_gap: f64,  // 6.0px (gap between checkbox and text)
    pub font_size: f64,          // 12.0px (text size)
    pub border_radius: f64,      // 4.0px (rounded corners)
    pub button_margin: f64,      // 4.0px (margin between buttons)
}

impl Default for CheckboxTextDefaults {
    fn default() -> Self {
        Self {
            height: 28.0,
            padding_x: 8.0,
            padding_y: 4.0,
            checkbox_size: 16.0,
            checkbox_text_gap: 6.0,
            font_size: 12.0,
            border_radius: 4.0,
            button_margin: 4.0,
        }
    }
}

/// Prototype colors for CheckboxText action buttons
pub struct CheckboxTextPrototypeColors {
    // Normal state (unchecked)
    pub background_normal: &'static str,     // "#1e222d" (toolbar bg)
    pub text_normal: &'static str,           // "#787b86" (muted text)
    pub checkbox_bg: &'static str,           // "transparent"
    pub checkbox_border: &'static str,       // "#3a3a3a" (separator)

    // Hover state
    pub background_hover: &'static str,      // "#2a2a2a" (subtle hover)
    pub text_hover: &'static str,            // "#e5e7eb" (bright text)

    // Active state (checked/selected)
    pub background_active: &'static str,     // "#1e3a5f" (blue-tinted)
    pub text_active: &'static str,           // "#ffffff" (bright white)
    pub checkbox_bg_active: &'static str,    // "#2962ff" (accent blue)
    pub checkbox_check_active: &'static str, // "#ffffff" (white checkmark)
    pub border_active: &'static str,         // "#2962ff" (focused border)

    // Disabled state
    pub background_disabled: &'static str,   // "#2a2a2a" (dim)
    pub text_disabled: &'static str,         // "#4a4a4a" (very dim)
}

impl Default for CheckboxTextPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "#1e222d",
            text_normal: "#787b86",
            checkbox_bg: "transparent",
            checkbox_border: "#3a3a3a",
            background_hover: "#2a2a2a",
            text_hover: "#e5e7eb",
            background_active: "#1e3a5f",
            text_active: "#ffffff",
            checkbox_bg_active: "#2962ff",
            checkbox_check_active: "#ffffff",
            border_active: "#2962ff",
            background_disabled: "#2a2a2a",
            text_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Toggle::IconSwap Defaults
// =============================================================================

/// Default sizes for IconSwap toggle buttons
pub struct IconSwapDefaults {
    pub icon_size: f64,      // 16.0px (standard icon)
    pub padding: f64,        // 0.0px (no padding)
    pub button_area: f64,    // 16.0px (icon size only)
    pub icon_gap: f64,       // 4.0px (gap between sequential icons)
}

impl Default for IconSwapDefaults {
    fn default() -> Self {
        Self {
            icon_size: 16.0,
            padding: 0.0,
            button_area: 16.0,
            icon_gap: 4.0,
        }
    }
}

/// Prototype colors for IconSwap toggle buttons
pub struct IconSwapPrototypeColors {
    // Normal state (both ON and OFF)
    pub background: &'static str,  // "transparent" (no background change)
    pub icon_off: &'static str,    // "#787b86" (muted gray)
    pub icon_on: &'static str,     // "#787b86" (same - only icon changes)

    // Hover state
    pub background_hover: &'static str,  // "#2a2a2a" (optional, context-dependent)
    pub icon_hover: &'static str,        // "#e5e7eb" (brighter)

    // Disabled state
    pub icon_disabled: &'static str,     // "#4a4a4a" (very dim)
}

impl Default for IconSwapPrototypeColors {
    fn default() -> Self {
        Self {
            background: "transparent",
            icon_off: "#787b86",
            icon_on: "#787b86",
            background_hover: "#2a2a2a",
            icon_hover: "#e5e7eb",
            icon_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Toggle::ButtonToggle Defaults
// =============================================================================

/// Default sizes for ButtonToggle toggle buttons
pub struct ButtonToggleDefaults {
    pub button_size: f64,          // 28.0px (square or height)
    pub icon_size: f64,            // 16.0px (standard icon)
    pub padding: f64,              // 4.0-6.0px (padding around icon)
    pub border_radius: f64,        // 3.0-4.0px (rounded corners)
    pub active_border_width: f64,  // 3.0px (left accent bar)
}

impl Default for ButtonToggleDefaults {
    fn default() -> Self {
        Self {
            button_size: 28.0,
            icon_size: 16.0,
            padding: 6.0,
            border_radius: 4.0,
            active_border_width: 3.0,
        }
    }
}

/// Prototype colors for ButtonToggle toggle buttons
pub struct ButtonTogglePrototypeColors {
    // Normal state (OFF)
    pub background_off: &'static str,     // "transparent" or toolbar_bg
    pub icon_off: &'static str,           // "#787b86" (muted gray)
    pub border_off: &'static str,         // "transparent" or separator

    // Normal state (ON)
    pub background_on: &'static str,      // "#1e3a5f" (blue-tinted active)
    pub icon_on: &'static str,            // "#ffffff" (bright white)
    pub border_on: &'static str,          // "#2962ff" (accent, 3px left bar)

    // Hover state (OFF)
    pub background_hover_off: &'static str, // "#2a2a2a" (subtle hover)
    pub icon_hover_off: &'static str,       // "#e5e7eb" (brighter)

    // Hover state (ON)
    pub background_hover_on: &'static str,  // "#2655cc" (brighter active)
    pub icon_hover_on: &'static str,        // "#ffffff" (bright white)

    // Disabled state
    pub background_disabled: &'static str,  // "transparent"
    pub icon_disabled: &'static str,        // "#4a4a4a" (very dim)
}

impl Default for ButtonTogglePrototypeColors {
    fn default() -> Self {
        Self {
            background_off: "transparent",
            icon_off: "#787b86",
            border_off: "transparent",
            background_on: "#1e3a5f",
            icon_on: "#ffffff",
            border_on: "#2962ff",
            background_hover_off: "#2a2a2a",
            icon_hover_off: "#e5e7eb",
            background_hover_on: "#2655cc",
            icon_hover_on: "#ffffff",
            background_disabled: "transparent",
            icon_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Checkbox::Standard Defaults
// =============================================================================

/// Default sizes for standard checkboxes
pub struct CheckboxDefaults {
    pub checkbox_size: f64,     // 16.0px × 16.0px (square)
    pub border_radius: f64,     // 3.0px (rounded corners)
    pub border_width: f64,      // 1.0px (border stroke)
    pub checkmark_size: f64,    // 10.0px (checkmark icon)
    pub row_height: f64,        // 32.0px (when in settings row)
    pub label_offset_x: f64,    // 12.0px (gap between checkbox and label)
}

impl Default for CheckboxDefaults {
    fn default() -> Self {
        Self {
            checkbox_size: 16.0,
            border_radius: 3.0,
            border_width: 1.0,
            checkmark_size: 10.0,
            row_height: 32.0,
            label_offset_x: 12.0,
        }
    }
}

/// Prototype colors for standard checkboxes
pub struct CheckboxPrototypeColors {
    // Normal state (unchecked)
    pub background_unchecked: &'static str,  // "transparent" or toolbar_bg
    pub border_unchecked: &'static str,      // "#3a3a3a" (separator)

    // Normal state (checked)
    pub background_checked: &'static str,    // "#2962ff" (accent blue)
    pub border_checked: &'static str,        // "#2962ff" (accent blue)
    pub checkmark: &'static str,             // "#ffffff" (white ✓)

    // Hover state (unchecked)
    pub background_hover_unchecked: &'static str,  // "#2a2a2a" (subtle hover)
    pub border_hover_unchecked: &'static str,      // "#e5e7eb" (brighter border)

    // Hover state (checked)
    pub background_hover_checked: &'static str,    // "#4080ff" (lighter blue)
    pub border_hover_checked: &'static str,        // "#4080ff" (lighter blue)

    // Disabled state
    pub background_disabled: &'static str,   // "#2a2a2a" (dim)
    pub border_disabled: &'static str,       // "#3a3a3a" (separator)
    pub checkmark_disabled: &'static str,    // "#4a4a4a" (dim checkmark if checked)
}

impl Default for CheckboxPrototypeColors {
    fn default() -> Self {
        Self {
            background_unchecked: "transparent",
            border_unchecked: "#3a3a3a",
            background_checked: "#2962ff",
            border_checked: "#2962ff",
            checkmark: "#ffffff",
            background_hover_unchecked: "#2a2a2a",
            border_hover_unchecked: "#e5e7eb",
            background_hover_checked: "#4080ff",
            border_hover_checked: "#4080ff",
            background_disabled: "#2a2a2a",
            border_disabled: "#3a3a3a",
            checkmark_disabled: "#4a4a4a",
        }
    }
}

// =============================================================================
// Tab::Vertical Defaults
// =============================================================================

/// Default sizes for vertical tabs
pub struct VerticalTabDefaults {
    pub tab_width: f64,            // 60.0-80.0px (sidebar width)
    pub tab_height: f64,           // 40.0-44.0px (tab height)
    pub icon_size: f64,            // 20.0px (larger than standard)
    pub active_bar_width: f64,     // 3.0px (left accent bar)
    pub icon_centering_offset: f64, // (sidebar_width - icon_size) / 2.0
}

impl Default for VerticalTabDefaults {
    fn default() -> Self {
        Self {
            tab_width: 70.0,
            tab_height: 44.0,
            icon_size: 20.0,
            active_bar_width: 3.0,
            icon_centering_offset: 25.0,  // (70 - 20) / 2
        }
    }
}

/// Prototype colors for vertical tabs
pub struct VerticalTabPrototypeColors {
    // Normal state (inactive)
    pub background_inactive: &'static str,  // "transparent" or toolbar_bg
    pub icon_inactive: &'static str,        // "#787b86" (muted gray)

    // Active state
    pub background_active: &'static str,    // "#1e3a5f" (blue-tinted)
    pub icon_active: &'static str,          // "#ffffff" (bright white)
    pub bar_active: &'static str,           // "#2962ff" (accent, 3px left bar)

    // Hover state (inactive)
    pub background_hover: &'static str,     // "#2a2a2a" (subtle hover)
    pub icon_hover: &'static str,           // "#e5e7eb" (brighter)
}

impl Default for VerticalTabPrototypeColors {
    fn default() -> Self {
        Self {
            background_inactive: "transparent",
            icon_inactive: "#787b86",
            background_active: "#1e3a5f",
            icon_active: "#ffffff",
            bar_active: "#2962ff",
            background_hover: "#2a2a2a",
            icon_hover: "#e5e7eb",
        }
    }
}

// =============================================================================
// Tab::Horizontal Defaults
// =============================================================================

/// Default sizes for horizontal tabs
pub struct HorizontalTabDefaults {
    pub tab_height: f64,         // 32.0px (tab height)
    pub padding_x: f64,          // 12.0px (horizontal text padding)
    pub tab_gap: f64,            // 2.0px (gap between tabs)
    pub font_size: f64,          // 13.0px (text size)
    pub border_radius: f64,      // 0.0px (rectangular tabs)
    pub underline_height: f64,   // 2.0px (bottom underline)
}

impl Default for HorizontalTabDefaults {
    fn default() -> Self {
        Self {
            tab_height: 32.0,
            padding_x: 12.0,
            tab_gap: 2.0,
            font_size: 13.0,
            border_radius: 0.0,
            underline_height: 2.0,
        }
    }
}

/// Prototype colors for horizontal tabs
pub struct HorizontalTabPrototypeColors {
    // Normal state (inactive)
    pub background_inactive: &'static str,  // "transparent"
    pub text_inactive: &'static str,        // "#787b86" (muted text)
    pub border_bottom: &'static str,        // "#3a3a3a" (shared separator line)

    // Active state
    pub background_active: &'static str,    // "#1e3a5f" (full background fill)
    pub text_active: &'static str,          // "#ffffff" (bright white)

    // Hover state (inactive)
    pub background_hover: &'static str,     // "#2a2a2a" (subtle hover)
    pub text_hover: &'static str,           // "#787b86" (same as inactive)
}

impl Default for HorizontalTabPrototypeColors {
    fn default() -> Self {
        Self {
            background_inactive: "transparent",
            text_inactive: "#787b86",
            border_bottom: "#3a3a3a",
            background_active: "#1e3a5f",
            text_active: "#ffffff",
            background_hover: "#2a2a2a",
            text_hover: "#787b86",
        }
    }
}

// =============================================================================
// ColorSwatch::Square Defaults
// =============================================================================

/// Default sizes for square color swatches
pub struct ColorSwatchSquareDefaults {
    pub swatch_size: f64,     // 24.0px × 24.0px (square)
    pub border_radius: f64,   // 4.0px (rounded corners)
    pub border_width: f64,    // 1.0px (border stroke)
    pub swatch_gap: f64,      // 8.0px (gap between swatches)
    pub row_height: f64,      // 32.0px (when in settings row)
}

impl Default for ColorSwatchSquareDefaults {
    fn default() -> Self {
        Self {
            swatch_size: 24.0,
            border_radius: 4.0,
            border_width: 1.0,
            swatch_gap: 8.0,
            row_height: 32.0,
        }
    }
}

/// Prototype colors for square color swatches
pub struct ColorSwatchSquarePrototypeColors {
    // Border colors (background is the color value itself)
    pub border_normal: &'static str,    // "#3a3a3a" (separator)
    pub border_hover: &'static str,     // "#e5e7eb" (brighter, 2px)
    pub border_active: &'static str,    // "#2962ff" (accent, 2px)
    pub border_disabled: &'static str,  // "#3a3a3a" (separator)
}

impl Default for ColorSwatchSquarePrototypeColors {
    fn default() -> Self {
        Self {
            border_normal: "#3a3a3a",
            border_hover: "#e5e7eb",
            border_active: "#2962ff",
            border_disabled: "#3a3a3a",
        }
    }
}

// =============================================================================
// ColorSwatch::IconWithBar Defaults
// =============================================================================

/// Default sizes for icon with color bar swatches
pub struct ColorSwatchIconBarDefaults {
    pub button_size: f64,       // 24.0px × 24.0px (button area)
    pub icon_size: f64,         // 16.0px (standard icon)
    pub icon_padding: f64,      // 4.0px (padding from edges)
    pub bar_width: f64,         // 16.0px (button_size - 8px)
    pub bar_height: f64,        // 3.0px (color bar height)
    pub bar_position_y: f64,    // 18.0px (button_size - 6px)
    pub bar_padding_x: f64,     // 4.0px (from left/right edges)
}

impl Default for ColorSwatchIconBarDefaults {
    fn default() -> Self {
        Self {
            button_size: 24.0,
            icon_size: 16.0,
            icon_padding: 4.0,
            bar_width: 16.0,
            bar_height: 3.0,
            bar_position_y: 18.0,
            bar_padding_x: 4.0,
        }
    }
}

/// Prototype colors for icon with color bar swatches
pub struct ColorSwatchIconBarPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "transparent"
    pub icon_normal: &'static str,        // "#787b86" (muted gray)

    // Hover state
    pub background_hover: &'static str,   // "#2a2a2a" (subtle hover)
    pub icon_hover: &'static str,         // "#e5e7eb" (brighter)

    // Active state
    pub background_active: &'static str,  // "#1e3a5f" (blue-tinted)
    pub icon_active: &'static str,        // "#ffffff" (bright white)
}

impl Default for ColorSwatchIconBarPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "transparent",
            icon_normal: "#787b86",
            background_hover: "#2a2a2a",
            icon_hover: "#e5e7eb",
            background_active: "#1e3a5f",
            icon_active: "#ffffff",
        }
    }
}

// =============================================================================
// Dropdown::TextChevron Defaults
// =============================================================================

/// Default sizes for text+chevron dropdowns
pub struct DropdownTextChevronDefaults {
    pub width: f64,              // 140.0px (total dropdown width)
    pub height: f64,             // 28.0px (standard height)
    pub chevron_area_width: f64, // 20.0px (right chevron area)
    pub text_area_width: f64,    // 120.0px (left text area, width - chevron_area)
    pub text_padding_x: f64,     // 8.0px (text padding from left)
    pub chevron_size: f64,       // 6.0px × 6.0px (chevron triangle)
    pub separator_width: f64,    // 1.0px (vertical separator)
    pub border_radius: f64,      // 4.0px (rounded corners)
    pub border_width: f64,       // 1.0px (border stroke)
}

impl Default for DropdownTextChevronDefaults {
    fn default() -> Self {
        Self {
            width: 140.0,
            height: 28.0,
            chevron_area_width: 20.0,
            text_area_width: 120.0,
            text_padding_x: 8.0,
            chevron_size: 6.0,
            separator_width: 1.0,
            border_radius: 4.0,
            border_width: 1.0,
        }
    }
}

/// Prototype colors for text+chevron dropdowns
pub struct DropdownTextChevronPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "#1e222d" (modal/content bg)
    pub text_normal: &'static str,        // "#d1d5db" (standard text)
    pub chevron_normal: &'static str,     // "#d1d5db" (standard text)
    pub border_normal: &'static str,      // "#3a3a3a" (separator)
    pub separator: &'static str,          // "#3a3a3a" (vertical separator)

    // Hover state
    pub background_hover: &'static str,   // "#2a2a2a" (subtle hover)
    pub text_hover: &'static str,         // "#d1d5db" (same)
    pub chevron_hover: &'static str,      // "#d1d5db" (same)
    pub border_hover: &'static str,       // "#e5e7eb" (brighter)

    // Active/Open state
    pub background_active: &'static str,  // "#1e3a5f" (blue-tinted)
    pub text_active: &'static str,        // "#ffffff" (bright white)
    pub chevron_active: &'static str,     // "#ffffff" (bright white)
    pub border_active: &'static str,      // "#2962ff" (accent)

    // Disabled state
    pub background_disabled: &'static str, // "#2a2a2a" (dim)
    pub text_disabled: &'static str,      // "#4a4a4a" (very dim)
    pub chevron_disabled: &'static str,   // "#4a4a4a" (very dim)
    pub border_disabled: &'static str,    // "#3a3a3a" (separator)
}

impl Default for DropdownTextChevronPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "#1e222d",
            text_normal: "#d1d5db",
            chevron_normal: "#d1d5db",
            border_normal: "#3a3a3a",
            separator: "#3a3a3a",
            background_hover: "#2a2a2a",
            text_hover: "#d1d5db",
            chevron_hover: "#d1d5db",
            border_hover: "#e5e7eb",
            background_active: "#1e3a5f",
            text_active: "#ffffff",
            chevron_active: "#ffffff",
            border_active: "#2962ff",
            background_disabled: "#2a2a2a",
            text_disabled: "#4a4a4a",
            chevron_disabled: "#4a4a4a",
            border_disabled: "#3a3a3a",
        }
    }
}

// =============================================================================
// Dropdown::ChevronOnly Defaults
// =============================================================================

/// Default sizes for chevron-only dropdowns
pub struct DropdownChevronOnlyDefaults {
    pub button_width: f64,       // 24.0-28.0px (padding + chevron)
    pub button_height: f64,      // 24.0-28.0px (standard height)
    pub chevron_size: f64,       // 12.0-16.0px (chevron icon)
    pub padding_x: f64,          // 4.0-6.0px (horizontal padding)
    pub border_radius: f64,      // 3.0-4.0px (rounded corners)
    pub border_width: f64,       // 0.5px (subtle border)
}

impl Default for DropdownChevronOnlyDefaults {
    fn default() -> Self {
        Self {
            button_width: 24.0,
            button_height: 24.0,
            chevron_size: 12.0,
            padding_x: 6.0,
            border_radius: 3.0,
            border_width: 0.5,
        }
    }
}

/// Prototype colors for chevron-only dropdowns
pub struct DropdownChevronOnlyPrototypeColors {
    // Normal state
    pub background_normal: &'static str,  // "transparent"
    pub chevron_normal: &'static str,     // "#787b86" with 0.7 opacity
    pub border_normal: &'static str,      // "#3a3a3a" with 0.8 opacity

    // Hover state
    pub background_hover: &'static str,   // "#2a2a2a" (subtle hover)
    pub chevron_hover: &'static str,      // "#e5e7eb" (brighter)
    pub border_hover: &'static str,       // "#3a3a3a" with 0.8 opacity (same)

    // Active/Pressed state
    pub background_active: &'static str,  // "#1e3a5f" (blue-tinted)
    pub chevron_active: &'static str,     // "#ffffff" (bright white)

    // Disabled state
    pub background_disabled: &'static str, // "transparent"
    pub chevron_disabled: &'static str,   // "#4a4a4a" with opacity
}

impl Default for DropdownChevronOnlyPrototypeColors {
    fn default() -> Self {
        Self {
            background_normal: "transparent",
            chevron_normal: "#787b86",  // with 0.7 opacity applied at render
            border_normal: "#3a3a3a",   // with 0.8 opacity applied at render
            background_hover: "#2a2a2a",
            chevron_hover: "#e5e7eb",
            border_hover: "#3a3a3a",
            background_active: "#1e3a5f",
            chevron_active: "#ffffff",
            background_disabled: "transparent",
            chevron_disabled: "#4a4a4a",
        }
    }
}
