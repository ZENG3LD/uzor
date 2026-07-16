//! [`FontSpec`] — typed font description.
//!
//! The shaper's CSS-shorthand string (`"bold 16px Roboto"`) is an internal
//! adapter detail of [`crate::shape::cosmic`], never leaked into this
//! crate's own public API (design law 2: typed contracts, no config bags).

use uzor::fonts::FontFamily;

/// Typed font description consumed by [`crate::shape::LineShaper`]
/// implementations.
///
/// `uzor::shaper` / `uzor::fonts::parse_css_font` only resolve family names
/// to [`FontFamily::Roboto`] / [`FontFamily::PtRootUi`] /
/// [`FontFamily::JetBrainsMono`] today — `FontSpec` is pinned to the same
/// three families for Arc 2 Phase 1 (see the design doc's open question on
/// widening the family surface).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSpec {
    pub family: FontFamily,
    pub size_px: f64,
    pub bold: bool,
    pub italic: bool,
}

impl FontSpec {
    /// A regular-weight, non-italic spec at `size_px`.
    pub fn new(family: FontFamily, size_px: f64) -> Self {
        Self { family, size_px, bold: false, italic: false }
    }

    /// Builder: mark this spec bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Builder: mark this spec italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// CSS-shorthand string the shaper eats, e.g. `"bold 16px Roboto"`.
    pub fn to_css_font(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(3);
        if self.bold {
            parts.push("bold".to_string());
        }
        if self.italic {
            parts.push("italic".to_string());
        }
        parts.push(format!("{}px", self.size_px));
        parts.push(self.family_name().to_string());
        parts.join(" ")
    }

    fn family_name(&self) -> &'static str {
        match self.family {
            FontFamily::Roboto => "Roboto",
            FontFamily::PtRootUi => "PT Root UI",
            FontFamily::JetBrainsMono => "JetBrains Mono",
        }
    }
}

impl Default for FontSpec {
    /// Roboto, 16px, regular — matches `uzor::fonts::FontInfo::default()`.
    fn default() -> Self {
        Self::new(FontFamily::Roboto, 16.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_css_font_orders_weight_style_size_family() {
        let spec = FontSpec::new(FontFamily::JetBrainsMono, 14.0).bold().italic();
        assert_eq!(spec.to_css_font(), "bold italic 14px JetBrains Mono");
    }

    #[test]
    fn to_css_font_regular_has_no_weight_style_tokens() {
        let spec = FontSpec::new(FontFamily::Roboto, 16.0);
        assert_eq!(spec.to_css_font(), "16px Roboto");
    }

    #[test]
    fn default_is_16px_regular_roboto() {
        let spec = FontSpec::default();
        assert_eq!(spec.family, FontFamily::Roboto);
        assert_eq!(spec.size_px, 16.0);
        assert!(!spec.bold);
        assert!(!spec.italic);
    }
}
