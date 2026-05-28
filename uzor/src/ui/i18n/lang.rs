//! Language enum and metadata table.
//!
//! Single source of truth for supported languages.
//! Adding a new language requires editing only this file + tables.rs.

/// Number of supported languages.
/// Increment this constant when adding a new language variant.
pub const N_LANG: usize = 15;

/// Supported UI languages.
///
/// Discriminant = column index in all translation tables.
/// Order is **frozen** — do not reorder existing variants.
/// New languages are appended at the end only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Language {
    /// English (default fallback)
    #[default]
    En = 0,
    /// Russian
    Ru = 1,
    /// Spanish
    Es = 2,
    /// German
    De = 3,
    /// French
    Fr = 4,
    /// Portuguese
    Pt = 5,
    /// Chinese (Simplified)
    Zh = 6,
    /// Japanese
    Ja = 7,
    /// Korean
    Ko = 8,
    /// Arabic
    Ar = 9,
    /// Italian
    It = 10,
    /// Turkish
    Tr = 11,
    /// Polish
    Pl = 12,
    /// Ukrainian
    Uk = 13,
    /// Hindi
    Hi = 14,
}

struct LangMeta {
    code: &'static str,
    name_en: &'static str,
    native: &'static str,
}

const LANG_META: [LangMeta; N_LANG] = [
    LangMeta { code: "en", name_en: "English",    native: "English"     }, // 0
    LangMeta { code: "ru", name_en: "Russian",     native: "Русский"     }, // 1
    LangMeta { code: "es", name_en: "Spanish",     native: "Español"     }, // 2
    LangMeta { code: "de", name_en: "German",      native: "Deutsch"     }, // 3
    LangMeta { code: "fr", name_en: "French",      native: "Français"    }, // 4
    LangMeta { code: "pt", name_en: "Portuguese",  native: "Português"   }, // 5
    LangMeta { code: "zh", name_en: "Chinese",     native: "中文"         }, // 6
    LangMeta { code: "ja", name_en: "Japanese",    native: "日本語"        }, // 7
    LangMeta { code: "ko", name_en: "Korean",      native: "한국어"         }, // 8
    LangMeta { code: "ar", name_en: "Arabic",      native: "العربية"      }, // 9
    LangMeta { code: "it", name_en: "Italian",     native: "Italiano"    }, // 10
    LangMeta { code: "tr", name_en: "Turkish",     native: "Türkçe"      }, // 11
    LangMeta { code: "pl", name_en: "Polish",      native: "Polski"      }, // 12
    LangMeta { code: "uk", name_en: "Ukrainian",   native: "Українська"  }, // 13
    LangMeta { code: "hi", name_en: "Hindi",       native: "हिन्दी"         }, // 14
];

impl Language {
    /// ISO 639-1 language code.
    #[inline]
    pub fn code(self) -> &'static str {
        LANG_META[self as usize].code
    }

    /// Language name in English.
    #[inline]
    pub fn name(self) -> &'static str {
        LANG_META[self as usize].name_en
    }

    /// Language name in native script.
    #[inline]
    pub fn native_name(self) -> &'static str {
        LANG_META[self as usize].native
    }

    /// Convert from raw `u8`. Returns `En` for out-of-range values.
    ///
    /// # Safety
    ///
    /// Safe because `repr(u8)` guarantees layout and we check `v < N_LANG`
    /// so every transmuted value corresponds to a valid discriminant.
    pub fn from_u8(v: u8) -> Self {
        if (v as usize) < N_LANG {
            // SAFETY: repr(u8), v < N_LANG, all discriminants 0..14 are valid
            unsafe { std::mem::transmute(v) }
        } else {
            Language::En
        }
    }

    /// Find language by ISO 639-1 code. Returns `None` if not found.
    pub fn from_code(code: &str) -> Option<Self> {
        LANG_META
            .iter()
            .position(|m| m.code == code)
            .map(|i| Language::from_u8(i as u8))
    }

    /// All supported languages in discriminant order.
    pub fn all() -> &'static [Language] {
        use Language::*;
        &[En, Ru, Es, De, Fr, Pt, Zh, Ja, Ko, Ar, It, Tr, Pl, Uk, Hi]
    }
}
