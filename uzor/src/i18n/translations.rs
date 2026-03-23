//! Translation utilities
//!
//! This module provides additional translation utilities and documentation
//! for extending the i18n system.

use super::{Language, TextKey, MonthKey, TooltipKey};

/// Trait for types that can be translated
pub trait Translatable {
    /// Get the translated display name for this value
    fn display_name(&self, lang: Language) -> &'static str;

    /// Get display name using current global language
    fn display_name_current(&self) -> &'static str {
        self.display_name(super::current_language())
    }
}

impl Translatable for TextKey {
    fn display_name(&self, lang: Language) -> &'static str {
        self.get(lang)
    }
}

impl Translatable for MonthKey {
    /// Returns the short month name for the given language
    fn display_name(&self, lang: Language) -> &'static str {
        self.short(lang)
    }
}

impl Translatable for TooltipKey {
    fn display_name(&self, lang: Language) -> &'static str {
        self.get(lang)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translatable_text_key() {
        let key = TextKey::Delete;
        assert_eq!(key.display_name(Language::En), "Delete");
        assert_eq!(key.display_name(Language::Ru), "Удалить");
    }

    #[test]
    fn test_translatable_month_key() {
        let key = MonthKey::January;
        assert_eq!(key.display_name(Language::En), "Jan");
        assert_eq!(key.display_name(Language::Ru), "Янв");
    }

    #[test]
    fn test_translatable_tooltip_key() {
        let key = TooltipKey::Undo;
        assert_eq!(key.display_name(Language::En), "Undo");
        assert_eq!(key.display_name(Language::Ru), "Отменить");
    }
}
