//! Button theme trait - Contract/Connector for button colors
//!
//! # Architecture Role
//!
//! **ButtonTheme is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - System theme managers (e.g., `ToolbarTheme`, `AppTheme`, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ 1. System Theme Manager (e.g., ToolbarTheme)           │
//! │    - Stores actual color values                         │
//! │    - Implements ButtonTheme trait (mapping)             │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 2. ButtonTheme trait (THIS MODULE)                      │
//! │    - Defines contract (which methods/colors needed)     │
//! │    - Acts as connector interface                        │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/render.rs)        │
//! │    - Accept &dyn ButtonTheme                            │
//! │    - Call trait methods to get colors                   │
//! └─────────────────────────────────────────────────────────┘
//! ```

/// Theme trait for button colors - Contract between system themes and factory rendering
pub trait ButtonTheme {
    // =========================================================================
    // Background colors
    // =========================================================================

    /// Normal state background color
    /// Typical: "transparent" or "#1e222d"
    fn button_bg_normal(&self) -> &str;

    /// Hover state background color
    /// Typical: "#2a2a2a"
    fn button_bg_hover(&self) -> &str;

    /// Pressed state background color
    /// Typical: "#1e3a5f"
    fn button_bg_pressed(&self) -> &str;

    /// Active/toggled state background color
    /// Typical: "#1e3a5f"
    fn button_bg_active(&self) -> &str;

    /// Disabled state background color
    /// Typical: "#3a3a3a"
    fn button_bg_disabled(&self) -> &str;

    // =========================================================================
    // Text colors
    // =========================================================================

    /// Normal state text color
    /// Typical: "#787b86"
    fn button_text_normal(&self) -> &str;

    /// Hover state text color
    /// Typical: "#ffffff"
    fn button_text_hover(&self) -> &str;

    /// Active state text color
    /// Typical: "#ffffff"
    fn button_text_active(&self) -> &str;

    /// Disabled state text color
    /// Typical: "#4a4a4a"
    fn button_text_disabled(&self) -> &str;

    // =========================================================================
    // Icon colors
    // =========================================================================

    /// Normal state icon color
    /// Typical: "#787b86"
    fn button_icon_normal(&self) -> &str;

    /// Hover state icon color
    /// Typical: "#ffffff"
    fn button_icon_hover(&self) -> &str;

    /// Active state icon color
    /// Typical: "#ffffff"
    fn button_icon_active(&self) -> &str;

    /// Disabled state icon color
    /// Typical: "#4a4a4a"
    fn button_icon_disabled(&self) -> &str;

    // =========================================================================
    // Border colors
    // =========================================================================

    /// Normal state border color
    /// Typical: "#3a3a3a"
    fn button_border_normal(&self) -> &str;

    /// Hover state border color
    /// Typical: "#2a2a2a"
    fn button_border_hover(&self) -> &str;

    /// Focused state border color
    /// Typical: "#1e3a5f"
    fn button_border_focused(&self) -> &str;

    // =========================================================================
    // Semantic colors
    // =========================================================================

    /// Primary/accent color for primary actions and active state
    /// Typical: "#2962ff"
    fn button_accent(&self) -> &str;

    /// Danger color for delete/remove actions
    /// Typical: "#ef4444"
    fn button_danger(&self) -> &str;

    /// Success color for confirm/success actions
    /// Typical: "#10b981"
    fn button_success(&self) -> &str;

    /// Warning color for warning actions
    /// Typical: "#f59e0b"
    fn button_warning(&self) -> &str;

    // =========================================================================
    // Toolbar-specific slots
    // =========================================================================

    /// Toolbar item background on hover.
    /// Typical: "#2a2e39"
    fn toolbar_item_bg_hover(&self) -> &str;

    /// Toolbar item background when active/selected.
    /// Typical: "#2196F3"
    fn toolbar_item_bg_active(&self) -> &str;

    /// Default toolbar item icon/text colour.
    /// Typical: "#d1d4dc"
    fn toolbar_item_text(&self) -> &str;

    /// Toolbar item icon/text colour on hover.
    /// Typical: "#ffffff"
    fn toolbar_item_text_hover(&self) -> &str;

    /// Toolbar item icon/text colour when active.
    /// Typical: "#ffffff"
    fn toolbar_item_text_active(&self) -> &str;

    /// Toolbar separator line colour.
    /// Typical: "#2a2e39"
    fn toolbar_separator(&self) -> &str;

    /// Toolbar background fill colour.
    /// Typical: "#1e222d"
    fn toolbar_background(&self) -> &str;

    /// Toolbar accent colour — used for 3px sidebar active bar.
    /// Typical: "#2962ff"
    fn toolbar_accent(&self) -> &str;

    /// Non-interactive label text colour in the toolbar.
    /// Falls back to `toolbar_item_text` if not overridden.
    fn toolbar_label_text(&self) -> &str {
        self.toolbar_item_text()
    }

    /// Clock text colour.
    /// Falls back to `toolbar_item_text` if not overridden.
    fn clock_text(&self) -> &str {
        self.toolbar_item_text()
    }

    // =========================================================================
    // Modal action button slots (sections 12-18)
    // =========================================================================

    /// Primary action button idle background (#2962ff in mlc).
    fn button_primary_bg(&self) -> &str;

    /// Primary action button hover background (#4080ff in mlc).
    fn button_primary_bg_hover(&self) -> &str;

    /// Danger button idle fill — semi-transparent red.
    /// mlc profile_manager: "rgba(255,80,80,0.15)"
    fn button_danger_bg(&self) -> &str;

    /// Danger button hover fill — semi-transparent red, more opaque.
    /// mlc profile_manager: "rgba(255,80,80,0.35)"
    fn button_danger_bg_hover(&self) -> &str;

    /// Danger button border idle.
    /// mlc user_settings: "rgba(239,83,80,0.5)"
    fn button_danger_border(&self) -> &str;

    /// Danger button border hover.
    /// mlc user_settings: "rgba(239,83,80,0.75)"
    fn button_danger_border_hover(&self) -> &str;

    /// Danger button text/icon colour.
    /// mlc: "#ef5350" (toolbar_theme.danger)
    fn button_danger_text(&self) -> &str;

    /// Secondary-neutral button hover fill.
    /// mlc user_settings: "rgba(255,255,255,0.12)"
    fn button_secondary_hover_bg(&self) -> &str;

    /// Muted text used in secondary/neutral buttons idle state.
    /// Falls back to toolbar_item_text_muted if not overridden.
    /// mlc: "rgba(254,255,238,0.7)"
    fn button_secondary_text_muted(&self) -> &str;

    /// Normal text used in secondary/neutral buttons hover state.
    /// mlc: "rgba(254,255,238,0.95)"
    fn button_secondary_text(&self) -> &str;

    /// Dropdown/modal background for ghost-outline Cancel idle fill.
    /// mlc: toolbar_theme.dropdown_bg
    fn button_ghost_idle_bg(&self) -> &str;

    /// Utility / SignIn button idle background.
    /// mlc: toolbar_theme.button_bg
    fn button_utility_bg(&self) -> &str;

    /// Utility / SignIn button hover background.
    /// mlc: toolbar_theme.button_bg_hover
    fn button_utility_bg_hover(&self) -> &str;

    // Color swatch slots (sections 27-30)
    // =========================================================================

    /// Idle border drawn around a color swatch square.
    /// mlc: toolbar_theme.separator
    fn color_swatch_border(&self) -> &str {
        self.toolbar_separator()
    }

    /// Outline drawn around the swatch when hovered or picker is open.
    /// mlc indicator_settings: toolbar_theme.item_bg_hover (expand rect)
    /// Used as the hover-expand fill color.
    fn color_swatch_hover_outline(&self) -> &str {
        self.toolbar_item_bg_hover()
    }

    /// Accent border color when the color picker is open (selected).
    /// mlc indicator_settings: toolbar_theme.accent
    fn color_swatch_selected_border(&self) -> &str {
        self.toolbar_accent()
    }

    /// Light tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#ffffff"
    fn transparency_checker_a(&self) -> &str {
        "#ffffff"
    }

    /// Dark tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#cccccc"
    fn transparency_checker_b(&self) -> &str {
        "#cccccc"
    }

    // =========================================================================
    // Fill toggle slots (section 31)
    // =========================================================================

    /// Border color for the fill-toggle when fill is enabled (active state).
    /// mlc primitive_settings: toolbar_theme.item_bg_active
    fn fill_toggle_active_border(&self) -> &str {
        self.toolbar_item_bg_active()
    }

    /// Diagonal strikethrough color for the fill-toggle when fill is disabled.
    /// mlc primitive_settings: toolbar_theme.separator
    fn fill_toggle_off_pattern_color(&self) -> &str {
        self.toolbar_separator()
    }

    // =========================================================================
    // Dropdown slots (sections 32-33, 38)
    // =========================================================================

    /// Background fill for a `DropdownField` trigger in idle state.
    /// mlc alert_settings: toolbar_theme.dropdown_bg
    fn dropdown_field_bg(&self) -> &str {
        self.toolbar_background()
    }

    /// Background fill for a `DropdownField` trigger on hover.
    /// mlc alert_settings: toolbar_theme.item_bg_hover
    fn dropdown_field_bg_hover(&self) -> &str {
        self.toolbar_item_bg_hover()
    }

    /// Border color for a `DropdownField` trigger.
    /// mlc: toolbar_theme.separator
    fn dropdown_field_border(&self) -> &str {
        self.toolbar_separator()
    }

    /// Text color inside a `DropdownField` trigger.
    /// Falls back to `toolbar_item_text`.
    fn dropdown_field_text(&self) -> &str {
        self.toolbar_item_text()
    }

    /// Normal (idle) background for a `DropdownMenuRow`.
    /// Transparent by default — menu panel provides its own bg.
    fn dropdown_menu_row_bg_normal(&self) -> &str {
        "transparent"
    }

    /// Hovered background for a `DropdownMenuRow`.
    /// mlc: toolbar_theme.item_bg_hover
    fn dropdown_menu_row_bg_hover(&self) -> &str {
        self.toolbar_item_bg_hover()
    }

    /// Selected background for a `DropdownMenuRow`.
    /// mlc: toolbar_theme.item_bg_active
    fn dropdown_menu_row_bg_selected(&self) -> &str {
        self.toolbar_item_bg_active()
    }

    /// Text color for a non-selected `DropdownMenuRow`.
    /// Falls back to `toolbar_item_text`.
    fn dropdown_menu_row_text(&self) -> &str {
        self.toolbar_item_text()
    }

    /// Text color for a selected `DropdownMenuRow`.
    /// mlc: toolbar_theme.item_text_active
    fn dropdown_menu_row_text_selected(&self) -> &str {
        self.toolbar_item_text_active()
    }

    /// Separator line color drawn below a `DropdownMenuRow` when
    /// `separator_after = true`.
    /// mlc: toolbar_theme.separator
    fn dropdown_menu_separator(&self) -> &str {
        self.toolbar_separator()
    }

    /// Chevron icon color used in `SplitDropdown` and `DropdownField`.
    /// Falls back to muted text.  mlc: toolbar_theme.item_text (or item_text_muted).
    fn dropdown_chevron_color(&self) -> &str {
        self.toolbar_item_text()
    }

    // =========================================================================
    // Selector button slots (sections 34, 39, 40)
    // =========================================================================

    /// Border for a selector button in idle (not selected, not hovered) state.
    /// mlc indicator_settings shape selector: toolbar_theme.separator
    fn selector_idle_border(&self) -> &str {
        self.toolbar_separator()
    }

    /// Border drawn around the selected selector button.
    /// mlc shape selector active: toolbar_theme.accent
    fn selector_selected_border(&self) -> &str {
        self.toolbar_accent()
    }

    /// Border drawn when the pointer is over an unselected selector button.
    /// mlc shape selector hover: toolbar_theme.item_bg_hover (used as bg, no extra outline)
    fn selector_hover_border(&self) -> &str {
        self.toolbar_item_bg_hover()
    }

    /// Label text color below / beside the selector button.
    /// Falls back to `toolbar_item_text`.
    fn selector_label_text(&self) -> &str {
        self.toolbar_item_text()
    }

    // =========================================================================
    // Close button slots (section 41)
    // =========================================================================

    /// Color of the X glyph drawn inside a close button.
    /// mlc watchlist hover: `toolbar_theme.item_text`; idle: `item_text_muted`.
    /// Default: normal text color.
    fn close_button_x_color(&self) -> &str {
        self.toolbar_item_text()
    }

    /// Color of the X when the close button is hovered.
    /// mlc watchlist: `toolbar_theme.item_text` (full brightness).
    fn close_button_x_color_hover(&self) -> &str {
        self.toolbar_item_text_hover()
    }

    // =========================================================================
    // ScrollChevron slots (section 42)
    // =========================================================================

    /// Color of the chevron glyph in a scroll-chevron button.
    /// mlc toolbar_core: uses same color pipeline as toolbar icons.
    /// Default: normal toolbar text.
    fn scroll_chevron_color(&self) -> &str {
        self.toolbar_item_text()
    }

    /// Color of the chevron when hovered.
    /// Default: hover toolbar text.
    fn scroll_chevron_color_hover(&self) -> &str {
        self.toolbar_item_text_hover()
    }

    /// Color of the chevron when disabled (no items to scroll to).
    /// Default: disabled text.
    fn scroll_chevron_color_disabled(&self) -> &str {
        self.button_text_disabled()
    }
}

// =============================================================================
// Default Theme Implementation
// =============================================================================

/// Default button theme using prototype colors
pub struct DefaultButtonTheme;

impl DefaultButtonTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultButtonTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonTheme for DefaultButtonTheme {
    // Background colors
    fn button_bg_normal(&self) -> &str { "transparent" }
    fn button_bg_hover(&self) -> &str { "#2a2a2a" }
    fn button_bg_pressed(&self) -> &str { "#1e3a5f" }
    fn button_bg_active(&self) -> &str { "#1e3a5f" }
    fn button_bg_disabled(&self) -> &str { "#2a2a2a" }

    // Text colors
    fn button_text_normal(&self) -> &str { "#d1d5db" }
    fn button_text_hover(&self) -> &str { "#ffffff" }
    fn button_text_active(&self) -> &str { "#ffffff" }
    fn button_text_disabled(&self) -> &str { "#4a4a4a" }

    // Icon colors
    fn button_icon_normal(&self) -> &str { "#787b86" }
    fn button_icon_hover(&self) -> &str { "#e5e7eb" }
    fn button_icon_active(&self) -> &str { "#ffffff" }
    fn button_icon_disabled(&self) -> &str { "#4a4a4a" }

    // Border colors
    fn button_border_normal(&self) -> &str { "#3a3a3a" }
    fn button_border_hover(&self) -> &str { "#e5e7eb" }
    fn button_border_focused(&self) -> &str { "#2962ff" }

    // Semantic colors
    fn button_accent(&self) -> &str { "#2962ff" }
    fn button_danger(&self) -> &str { "#ef5350" }
    fn button_success(&self) -> &str { "#10b981" }
    fn button_warning(&self) -> &str { "#f59e0b" }

    // Toolbar-specific slots
    fn toolbar_item_bg_hover(&self) -> &str   { "#2a2e39" }
    fn toolbar_item_bg_active(&self) -> &str  { "#2196F3" }
    fn toolbar_item_text(&self) -> &str       { "#d1d4dc" }
    fn toolbar_item_text_hover(&self) -> &str { "#ffffff" }
    fn toolbar_item_text_active(&self) -> &str{ "#ffffff" }
    fn toolbar_separator(&self) -> &str       { "#2a2e39" }
    fn toolbar_background(&self) -> &str      { "#1e222d" }
    fn toolbar_accent(&self) -> &str          { "#2962ff" }

    // Modal action button slots
    fn button_primary_bg(&self) -> &str              { "#2962ff" }
    fn button_primary_bg_hover(&self) -> &str        { "#4080ff" }
    fn button_danger_bg(&self) -> &str               { "rgba(255,80,80,0.15)" }
    fn button_danger_bg_hover(&self) -> &str         { "rgba(255,80,80,0.35)" }
    fn button_danger_border(&self) -> &str           { "rgba(239,83,80,0.5)" }
    fn button_danger_border_hover(&self) -> &str     { "rgba(239,83,80,0.75)" }
    fn button_danger_text(&self) -> &str             { "#ef5350" }
    fn button_secondary_hover_bg(&self) -> &str      { "rgba(255,255,255,0.12)" }
    fn button_secondary_text_muted(&self) -> &str    { "rgba(254,255,238,0.7)" }
    fn button_secondary_text(&self) -> &str          { "rgba(254,255,238,0.95)" }
    fn button_ghost_idle_bg(&self) -> &str           { "#1e222d" }
    fn button_utility_bg(&self) -> &str              { "#2a2e39" }
    fn button_utility_bg_hover(&self) -> &str        { "#363a45" }
}
