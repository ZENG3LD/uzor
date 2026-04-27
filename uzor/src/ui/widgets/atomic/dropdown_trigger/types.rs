//! DropdownTrigger widget types — view and render-kind enum.
//!
//! NOTE: `DropdownTrigger` is the **trigger button only** (the visible clickable
//! control that opens a menu or cycles values).  The composite `Dropdown` widget
//! (trigger + open menu + rows) is a separate composite widget to be added later.

use crate::ui::widgets::atomic::button::state::SplitButtonHoverZone;

/// Per-frame data for the `Split` render kind.
///
/// Represents a settings dropdown with two clickable zones:
/// - Left (text) area: cycles through values when clicked if `cycle_on_click`.
/// - Right (chevron) area: opens the dropdown menu.
pub struct SplitDropdownView<'a> {
    /// Current value label displayed in the text zone.
    pub current_label: &'a str,
    /// `true` if clicking the text zone cycles the value.
    pub cycle_on_click: bool,
    /// Which zone (if any) the pointer is currently over.
    pub hovered_zone: SplitButtonHoverZone,
    /// `true` when the dropdown menu is currently open.
    pub open: bool,
}

/// Per-frame data for the `Field` render kind.
///
/// Single-zone trigger styled as a form input (like a text input box)
/// with a chevron icon on the right.
pub struct DropdownFieldView<'a> {
    /// Current value label displayed inside the field.
    pub current_label: &'a str,
    /// `true` when the dropdown menu is currently open (open state styling).
    pub open: bool,
    /// `true` when the pointer is over this field.
    pub hovered: bool,
}

/// Selects the visual variant used by `draw_dropdown_trigger`.
///
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum DropdownTriggerRenderKind<'a> {
    /// Two-zone trigger: text (clickable body) + chevron (menu opener).
    /// Visual: `[Text.................|▼]`
    /// Ports section 32 from button-full.md (`chart_settings.rs draw_split_dropdown`).
    Split,
    /// Single-zone trigger styled like a text input with an inline chevron.
    /// Visual: `╭─────────────────────────╮` `│ current_label      ↓   │` `╰─────────────────────────╯`
    /// Ports section 33 from button-full.md (`alert_settings.rs draw_dropdown_field`).
    Field,
    /// Caller-supplied renderer. Bypasses all built-in draw logic.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &super::settings::DropdownTriggerSettings) + 'a>),
}
