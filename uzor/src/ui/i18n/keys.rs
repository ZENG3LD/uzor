//! Translation keys
//!
//! All translatable text has a typed key for compile-time safety.
//! Translations live in `tables.rs` — table-driven, zero-alloc.

use super::lang::Language;
use super::tables::{MONTH_TABLE_FULL, MONTH_TABLE_SHORT, TEXT_KEY_TABLE, TOOLTIP_KEY_TABLE};

// =============================================================================
// General Text Keys
// =============================================================================

/// General text keys used across the application.
///
/// Variant order is **frozen** — discriminant == row index in `TEXT_KEY_TABLE`.
/// New variants must be appended at the end; `COUNT` must be updated accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum TextKey {
    // Common actions
    Delete     = 0,
    Clone      = 1,
    Copy       = 2,
    Cancel     = 3,
    Apply      = 4,
    Save       = 5,
    Reset      = 6,
    Close      = 7,
    Ok         = 8,
    Yes        = 9,
    No         = 10,

    // Visibility/state
    Show       = 11,
    Hide       = 12,
    Lock       = 13,
    Unlock     = 14,
    Enable     = 15,
    Disable    = 16,

    // Common labels
    Settings   = 17,
    Properties = 18,
    Color      = 19,
    Style      = 20,
    Width      = 21,
    Opacity    = 22,
    Background = 23,
    Foreground = 24,
    Border     = 25,
    Text       = 26,
    Font       = 27,
    Size       = 28,

    // Position
    Left       = 29,
    Right      = 30,
    Top        = 31,
    Bottom     = 32,
    Center     = 33,
}

impl TextKey {
    /// Number of variants. Must equal the number of rows in `TEXT_KEY_TABLE`.
    pub const COUNT: usize = 34;

    /// Get translation for this key, with En fallback for empty cells.
    #[inline]
    pub fn get(self, lang: Language) -> &'static str {
        let row = &TEXT_KEY_TABLE[self as usize];
        let s = row[lang as usize];
        if !s.is_empty() { s } else { row[Language::En as usize] }
    }
}

// =============================================================================
// Month Names (for TimeScale)
// =============================================================================

/// Month name keys for time axis.
///
/// Variant order is **frozen** (January=0 .. December=11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum MonthKey {
    January   = 0,
    February  = 1,
    March     = 2,
    April     = 3,
    May       = 4,
    June      = 5,
    July      = 6,
    August    = 7,
    September = 8,
    October   = 9,
    November  = 10,
    December  = 11,
}

impl MonthKey {
    /// Get short month name (3 letters), En fallback for empty cells.
    #[inline]
    pub fn short(self, lang: Language) -> &'static str {
        let row = &MONTH_TABLE_SHORT[self as usize];
        let s = row[lang as usize];
        if !s.is_empty() { s } else { row[Language::En as usize] }
    }

    /// Get full month name, En fallback for empty cells.
    #[inline]
    pub fn full(self, lang: Language) -> &'static str {
        let row = &MONTH_TABLE_FULL[self as usize];
        let s = row[lang as usize];
        if !s.is_empty() { s } else { row[Language::En as usize] }
    }

    /// Get MonthKey from month number (1-12). Returns `January` for out-of-range.
    pub fn from_month(month: u32) -> Self {
        match month {
            1  => Self::January,
            2  => Self::February,
            3  => Self::March,
            4  => Self::April,
            5  => Self::May,
            6  => Self::June,
            7  => Self::July,
            8  => Self::August,
            9  => Self::September,
            10 => Self::October,
            11 => Self::November,
            12 => Self::December,
            _  => Self::January,
        }
    }
}

/// Get localized short month names array.
pub fn month_names_short(lang: Language) -> [&'static str; 12] {
    std::array::from_fn(|i| {
        let row = &MONTH_TABLE_SHORT[i];
        let s = row[lang as usize];
        if !s.is_empty() { s } else { row[Language::En as usize] }
    })
}

// =============================================================================
// Tooltip Keys
// =============================================================================

/// Window chrome tooltip keys — generic desktop UI controls.
///
/// App-specific tooltip keys (toolbar buttons, sidebar panels, etc.)
/// should be defined in the application's own i18n module.
///
/// Variant order is **frozen** — discriminant == row index in `TOOLTIP_KEY_TABLE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum TooltipKey {
    /// "Close window" / "Закрыть окно"
    CloseWindow = 0,
    /// "Quit application" / "Закрыть приложение"
    CloseApp    = 1,
    /// "Minimize" / "Свернуть"
    Minimize    = 2,
    /// "Maximize" / "Развернуть"
    Maximize    = 3,
    /// "Restore" / "Восстановить"
    Restore     = 4,
    /// "New window" / "Новое окно"
    NewWindow   = 5,
    /// "Menu" / "Меню"
    Menu        = 6,
    /// "New tab" / "Новая вкладка"
    NewTab      = 7,
    /// "Close tab" / "Закрыть вкладку"
    CloseTab    = 8,
    /// "Undo" / "Отменить"
    Undo        = 9,
}

impl TooltipKey {
    /// Number of variants. Must equal the number of rows in `TOOLTIP_KEY_TABLE`.
    pub const COUNT: usize = 10;

    /// Get translation for this key, with En fallback for empty cells.
    #[inline]
    pub fn get(self, lang: Language) -> &'static str {
        let row = &TOOLTIP_KEY_TABLE[self as usize];
        let s = row[lang as usize];
        if !s.is_empty() { s } else { row[Language::En as usize] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_keys() {
        assert_eq!(TextKey::Delete.get(Language::En), "Delete");
        assert_eq!(TextKey::Delete.get(Language::Ru), "Удалить");
    }

    #[test]
    fn test_month_keys() {
        assert_eq!(MonthKey::January.short(Language::En), "Jan");
        assert_eq!(MonthKey::January.short(Language::Ru), "Янв");
        assert_eq!(MonthKey::December.full(Language::Ru), "Декабрь");
    }

    #[test]
    fn test_month_from_number() {
        assert!(matches!(MonthKey::from_month(1), MonthKey::January));
        assert!(matches!(MonthKey::from_month(12), MonthKey::December));
        assert!(matches!(MonthKey::from_month(99), MonthKey::January));
    }

    #[test]
    fn test_month_names_short() {
        let names = month_names_short(Language::En);
        assert_eq!(names[0], "Jan");
        assert_eq!(names[11], "Dec");

        let names_ru = month_names_short(Language::Ru);
        assert_eq!(names_ru[0], "Янв");
        assert_eq!(names_ru[11], "Дек");
    }

    #[test]
    fn test_tooltip_keys() {
        assert_eq!(TooltipKey::CloseWindow.get(Language::En), "Close window");
        assert_eq!(TooltipKey::CloseWindow.get(Language::Ru), "Закрыть окно");
        assert_eq!(TooltipKey::Menu.get(Language::En), "Menu");
        assert_eq!(TooltipKey::Menu.get(Language::Ru), "Меню");
    }

    #[test]
    fn test_fallback_to_en_for_new_languages() {
        // Es (index 2) is stub → must return En value
        assert_eq!(TextKey::Delete.get(Language::Es), "Delete");
        assert_eq!(MonthKey::January.short(Language::De), "Jan");
        assert_eq!(TooltipKey::Undo.get(Language::Fr), "Undo");
    }

    #[test]
    fn test_all_en_cells_non_empty() {
        for row in TEXT_KEY_TABLE.iter() {
            assert!(!row[0].is_empty(), "En cell must not be empty in TEXT_KEY_TABLE");
        }
        for row in TOOLTIP_KEY_TABLE.iter() {
            assert!(!row[0].is_empty(), "En cell must not be empty in TOOLTIP_KEY_TABLE");
        }
        for row in MONTH_TABLE_SHORT.iter() {
            assert!(!row[0].is_empty(), "En cell must not be empty in MONTH_TABLE_SHORT");
        }
        for row in MONTH_TABLE_FULL.iter() {
            assert!(!row[0].is_empty(), "En cell must not be empty in MONTH_TABLE_FULL");
        }
    }
}
