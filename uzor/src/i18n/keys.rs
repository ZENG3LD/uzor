//! Translation keys
//!
//! All translatable text has a typed key for compile-time safety.

use super::Language;

// =============================================================================
// General Text Keys
// =============================================================================

/// General text keys used across the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextKey {
    // Common actions
    Delete,
    Clone,
    Copy,
    Cancel,
    Apply,
    Save,
    Reset,
    Close,
    Ok,
    Yes,
    No,

    // Visibility/state
    Show,
    Hide,
    Lock,
    Unlock,
    Enable,
    Disable,

    // Common labels
    Settings,
    Properties,
    Color,
    Style,
    Width,
    Opacity,
    Background,
    Foreground,
    Border,
    Text,
    Font,
    Size,

    // Position
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl TextKey {
    /// Get translation for this key
    pub fn get(self, lang: Language) -> &'static str {
        match lang {
            Language::En => self.en(),
            Language::Ru => self.ru(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::Delete => "Delete",
            Self::Clone => "Clone",
            Self::Copy => "Copy",
            Self::Cancel => "Cancel",
            Self::Apply => "Apply",
            Self::Save => "Save",
            Self::Reset => "Reset",
            Self::Close => "Close",
            Self::Ok => "OK",
            Self::Yes => "Yes",
            Self::No => "No",
            Self::Show => "Show",
            Self::Hide => "Hide",
            Self::Lock => "Lock",
            Self::Unlock => "Unlock",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
            Self::Settings => "Settings",
            Self::Properties => "Properties",
            Self::Color => "Color",
            Self::Style => "Style",
            Self::Width => "Width",
            Self::Opacity => "Opacity",
            Self::Background => "Background",
            Self::Foreground => "Foreground",
            Self::Border => "Border",
            Self::Text => "Text",
            Self::Font => "Font",
            Self::Size => "Size",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Center => "Center",
        }
    }

    fn ru(self) -> &'static str {
        match self {
            Self::Delete => "Удалить",
            Self::Clone => "Клонировать",
            Self::Copy => "Копировать",
            Self::Cancel => "Отмена",
            Self::Apply => "Применить",
            Self::Save => "Сохранить",
            Self::Reset => "Сбросить",
            Self::Close => "Закрыть",
            Self::Ok => "ОК",
            Self::Yes => "Да",
            Self::No => "Нет",
            Self::Show => "Показать",
            Self::Hide => "Скрыть",
            Self::Lock => "Заблокировать",
            Self::Unlock => "Разблокировать",
            Self::Enable => "Включить",
            Self::Disable => "Отключить",
            Self::Settings => "Настройки",
            Self::Properties => "Свойства",
            Self::Color => "Цвет",
            Self::Style => "Стиль",
            Self::Width => "Ширина",
            Self::Opacity => "Прозрачность",
            Self::Background => "Фон",
            Self::Foreground => "Передний план",
            Self::Border => "Граница",
            Self::Text => "Текст",
            Self::Font => "Шрифт",
            Self::Size => "Размер",
            Self::Left => "Слева",
            Self::Right => "Справа",
            Self::Top => "Сверху",
            Self::Bottom => "Снизу",
            Self::Center => "По центру",
        }
    }
}

// =============================================================================
// Month Names (for TimeScale)
// =============================================================================

/// Month name keys for time axis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonthKey {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl MonthKey {
    /// Get short month name (3 letters)
    pub fn short(self, lang: Language) -> &'static str {
        match lang {
            Language::En => self.short_en(),
            Language::Ru => self.short_ru(),
        }
    }

    /// Get full month name
    pub fn full(self, lang: Language) -> &'static str {
        match lang {
            Language::En => self.full_en(),
            Language::Ru => self.full_ru(),
        }
    }

    fn short_en(self) -> &'static str {
        match self {
            Self::January => "Jan",
            Self::February => "Feb",
            Self::March => "Mar",
            Self::April => "Apr",
            Self::May => "May",
            Self::June => "Jun",
            Self::July => "Jul",
            Self::August => "Aug",
            Self::September => "Sep",
            Self::October => "Oct",
            Self::November => "Nov",
            Self::December => "Dec",
        }
    }

    fn short_ru(self) -> &'static str {
        match self {
            Self::January => "Янв",
            Self::February => "Фев",
            Self::March => "Мар",
            Self::April => "Апр",
            Self::May => "Май",
            Self::June => "Июн",
            Self::July => "Июл",
            Self::August => "Авг",
            Self::September => "Сен",
            Self::October => "Окт",
            Self::November => "Ноя",
            Self::December => "Дек",
        }
    }

    fn full_en(self) -> &'static str {
        match self {
            Self::January => "January",
            Self::February => "February",
            Self::March => "March",
            Self::April => "April",
            Self::May => "May",
            Self::June => "June",
            Self::July => "July",
            Self::August => "August",
            Self::September => "September",
            Self::October => "October",
            Self::November => "November",
            Self::December => "December",
        }
    }

    fn full_ru(self) -> &'static str {
        match self {
            Self::January => "Январь",
            Self::February => "Февраль",
            Self::March => "Март",
            Self::April => "Апрель",
            Self::May => "Май",
            Self::June => "Июнь",
            Self::July => "Июль",
            Self::August => "Август",
            Self::September => "Сентябрь",
            Self::October => "Октябрь",
            Self::November => "Ноябрь",
            Self::December => "Декабрь",
        }
    }

    /// Get MonthKey from month number (1-12)
    pub fn from_month(month: u32) -> Self {
        match month {
            1 => Self::January,
            2 => Self::February,
            3 => Self::March,
            4 => Self::April,
            5 => Self::May,
            6 => Self::June,
            7 => Self::July,
            8 => Self::August,
            9 => Self::September,
            10 => Self::October,
            11 => Self::November,
            12 => Self::December,
            _ => Self::January, // fallback
        }
    }
}

/// Get localized short month names array
pub fn month_names_short(lang: Language) -> [&'static str; 12] {
    [
        MonthKey::January.short(lang),
        MonthKey::February.short(lang),
        MonthKey::March.short(lang),
        MonthKey::April.short(lang),
        MonthKey::May.short(lang),
        MonthKey::June.short(lang),
        MonthKey::July.short(lang),
        MonthKey::August.short(lang),
        MonthKey::September.short(lang),
        MonthKey::October.short(lang),
        MonthKey::November.short(lang),
        MonthKey::December.short(lang),
    ]
}

// =============================================================================
// Tooltip Keys
// =============================================================================

/// Window chrome tooltip keys — generic desktop UI controls.
///
/// App-specific tooltip keys (toolbar buttons, sidebar panels, etc.)
/// should be defined in the application's own i18n module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TooltipKey {
    /// "Close window" / "Закрыть окно"
    CloseWindow,
    /// "Quit application" / "Закрыть приложение"
    CloseApp,
    /// "Minimize" / "Свернуть"
    Minimize,
    /// "Maximize" / "Развернуть"
    Maximize,
    /// "Restore" / "Восстановить"
    Restore,
    /// "New window" / "Новое окно"
    NewWindow,
    /// "Menu" / "Меню"
    Menu,
}

impl TooltipKey {
    /// Get translation for this key
    pub fn get(self, lang: Language) -> &'static str {
        match lang {
            Language::En => self.en(),
            Language::Ru => self.ru(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::CloseWindow => "Close window",
            Self::CloseApp => "Quit application",
            Self::Minimize => "Minimize",
            Self::Maximize => "Maximize",
            Self::Restore => "Restore",
            Self::NewWindow => "New window",
            Self::Menu => "Menu",
        }
    }

    fn ru(self) -> &'static str {
        match self {
            Self::CloseWindow => "Закрыть окно",
            Self::CloseApp => "Закрыть приложение",
            Self::Minimize => "Свернуть",
            Self::Maximize => "Развернуть",
            Self::Restore => "Восстановить",
            Self::NewWindow => "Новое окно",
            Self::Menu => "Меню",
        }
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
}
