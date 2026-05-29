//! Generic i18n mechanism. No concrete languages or strings live here —
//! the consumer crate defines its Language enum, keys and translation tables
//! and implements `Translate` for its key types.

use std::sync::atomic::{AtomicU8, Ordering};

/// Generic translation contract. `lang_index` is the consumer's language
/// position (0 = primary/fallback). uzor does not know how many languages
/// exist or what they are.
pub trait Translate: Copy {
    /// Return the translated string for `lang_index`.
    /// Implementors MUST fall back to index 0 when a cell is empty.
    fn translate(self, lang_index: usize) -> &'static str;
}

static LANG_INDEX: AtomicU8 = AtomicU8::new(0);

/// Current language as an opaque index (consumer maps it to its enum).
#[inline]
pub fn current_lang_index() -> usize {
    LANG_INDEX.load(Ordering::Relaxed) as usize
}

/// Set current language by opaque index.
#[inline]
pub fn set_lang_index(idx: u8) {
    LANG_INDEX.store(idx, Ordering::Relaxed);
}

/// Translate a key using the current global language index.
#[inline]
pub fn t<K: Translate>(key: K) -> &'static str {
    key.translate(current_lang_index())
}

/// Inline helper for static table row lookup with En-fallback.
///
/// Usage: `uzor::table_lookup!(&TABLE[row_index], lang_index)`
///
/// Returns `row[lang_index]` if non-empty, otherwise `row[0]` (En fallback).
/// Clamps `lang_index` to the row length so callers never panic on out-of-range.
#[macro_export]
macro_rules! table_lookup {
    ($row:expr, $idx:expr) => {{
        let row: &[&str] = $row;
        let idx = $idx;
        let i = if idx < row.len() { idx } else { 0 };
        let s = row[i];
        if s.is_empty() { row[0] } else { s }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum TestKey {
        Hello,
        Bye,
    }

    // Simulated 2-language table: [En, Ru]
    static HELLO_ROW: [&str; 2] = ["Hello", "Привет"];
    static BYE_ROW: [&str; 2] = ["Bye", ""];  // empty cell — must fall back to [0]

    impl Translate for TestKey {
        fn translate(self, lang_index: usize) -> &'static str {
            let row: &[&str] = match self {
                TestKey::Hello => &HELLO_ROW,
                TestKey::Bye => &BYE_ROW,
            };
            let s = row[lang_index.min(row.len() - 1)];
            if s.is_empty() { row[0] } else { s }
        }
    }

    #[test]
    fn test_translate_index_0() {
        set_lang_index(0);
        assert_eq!(t(TestKey::Hello), "Hello");
        assert_eq!(t(TestKey::Bye), "Bye");
    }

    #[test]
    fn test_translate_index_1() {
        set_lang_index(1);
        assert_eq!(t(TestKey::Hello), "Привет");
    }

    #[test]
    fn test_fallback_empty_cell() {
        // BYE_ROW[1] is empty — must fall back to BYE_ROW[0]
        set_lang_index(1);
        assert_eq!(t(TestKey::Bye), "Bye");
        // Reset
        set_lang_index(0);
    }

    #[test]
    fn test_set_and_get_index() {
        set_lang_index(3);
        assert_eq!(current_lang_index(), 3);
        set_lang_index(0);
    }
}
