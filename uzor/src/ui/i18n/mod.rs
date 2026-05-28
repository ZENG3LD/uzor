//! Internationalization (i18n) module
//!
//! Simple, zero-dependency localization system using compile-time checked keys.
//!
//! # Design
//!
//! - All text keys are typed enums (compile-time safety)
//! - Translations are static strings (no heap allocation)
//! - Language can be switched at runtime with zero cost
//! - English is the default/fallback language
//!
//! # Usage
//!
//! ```ignore
//! use uzor::i18n::{Language, t, TextKey};
//!
//! // Get translation for current global language
//! let text = t(TextKey::Delete);
//!
//! // Or get for specific language
//! let text_ru = TextKey::Delete.get(Language::Ru);
//! ```
//!
//! # Adding New Languages
//!
//! 1. Increment `N_LANG` in `lang.rs`
//! 2. Add variant to `Language` enum in `lang.rs`
//! 3. Add row to `LANG_META` in `lang.rs`
//! 4. Add element to `Language::all()` in `lang.rs`
//! 5. Add column to every row in `tables.rs`

mod lang;
mod keys;
mod tables;
mod translations;

pub use lang::{Language, N_LANG};
pub use keys::{TextKey, MonthKey, TooltipKey, month_names_short};
pub use translations::Translatable;

use std::sync::atomic::{AtomicU8, Ordering};

// Global language setting (atomic for thread safety)
static CURRENT_LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Get current global language
#[inline]
pub fn current_language() -> Language {
    Language::from_u8(CURRENT_LANGUAGE.load(Ordering::Relaxed))
}

/// Set global language
#[inline]
pub fn set_language(lang: Language) {
    CURRENT_LANGUAGE.store(lang as u8, Ordering::Relaxed);
}

/// Translate a text key using current global language
#[inline]
pub fn t(key: TextKey) -> &'static str {
    key.get(current_language())
}

/// Translate a tooltip key using current global language
#[inline]
pub fn t_tooltip(key: TooltipKey) -> &'static str {
    key.get(current_language())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_default() {
        assert_eq!(Language::default(), Language::En);
    }

    #[test]
    fn test_language_codes() {
        assert_eq!(Language::En.code(), "en");
        assert_eq!(Language::Ru.code(), "ru");
        assert_eq!(Language::Es.code(), "es");
        assert_eq!(Language::Hi.code(), "hi");
    }

    #[test]
    fn test_language_names() {
        assert_eq!(Language::En.name(), "English");
        assert_eq!(Language::Ru.name(), "Russian");
        assert_eq!(Language::Ru.native_name(), "Русский");
    }

    #[test]
    fn test_language_from_code() {
        assert_eq!(Language::from_code("en"), Some(Language::En));
        assert_eq!(Language::from_code("ru"), Some(Language::Ru));
        assert_eq!(Language::from_code("xx"), None);
    }

    #[test]
    fn test_language_all() {
        let all = Language::all();
        assert_eq!(all.len(), N_LANG);
        assert_eq!(all[0], Language::En);
        assert_eq!(all[1], Language::Ru);
        assert_eq!(all[14], Language::Hi);
    }

    #[test]
    fn test_global_language() {
        // Save current
        let prev = current_language();

        set_language(Language::Ru);
        assert_eq!(current_language(), Language::Ru);

        set_language(Language::En);
        assert_eq!(current_language(), Language::En);

        // Restore
        set_language(prev);
    }

    #[test]
    fn test_translation() {
        set_language(Language::En);
        assert_eq!(t(TextKey::Delete), "Delete");

        set_language(Language::Ru);
        assert_eq!(t(TextKey::Delete), "Удалить");

        // Reset to default
        set_language(Language::En);
    }

    #[test]
    fn test_tooltip_translation() {
        set_language(Language::En);
        assert_eq!(t_tooltip(TooltipKey::CloseWindow), "Close window");

        set_language(Language::Ru);
        assert_eq!(t_tooltip(TooltipKey::CloseWindow), "Закрыть окно");

        // Reset to default
        set_language(Language::En);
    }
}
