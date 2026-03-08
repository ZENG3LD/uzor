//! macOS typography scale (SF Pro font family)
//!
//! 12-level hierarchy from Large Title (34px) to Caption 2 (11px)

/// Font weight
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Medium,
    Semibold,
    Bold,
}

/// Typography level in the macOS hierarchy
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypographyLevel {
    LargeTitle,
    Title1,
    Title2,
    Title3,
    Headline,
    Body,
    Callout,
    Subheadline,
    Footnote,
    Caption1,
    Caption2,
    Monospaced,
}

/// Get font size in pixels for a typography level
pub fn font_size(level: TypographyLevel) -> f64 {
    match level {
        TypographyLevel::LargeTitle => 34.0,
        TypographyLevel::Title1 => 28.0,
        TypographyLevel::Title2 => 22.0,
        TypographyLevel::Title3 => 20.0,
        TypographyLevel::Headline => 17.0,
        TypographyLevel::Body => 13.0,
        TypographyLevel::Callout => 13.0,
        TypographyLevel::Subheadline => 12.0,
        TypographyLevel::Footnote => 11.0,
        TypographyLevel::Caption1 => 11.0,
        TypographyLevel::Caption2 => 11.0,
        TypographyLevel::Monospaced => 13.0,
    }
}

/// Get font weight for a typography level
pub fn font_weight(level: TypographyLevel) -> FontWeight {
    match level {
        TypographyLevel::LargeTitle => FontWeight::Regular,
        TypographyLevel::Title1 => FontWeight::Regular,
        TypographyLevel::Title2 => FontWeight::Regular,
        TypographyLevel::Title3 => FontWeight::Regular,
        TypographyLevel::Headline => FontWeight::Bold,
        TypographyLevel::Body => FontWeight::Regular,
        TypographyLevel::Callout => FontWeight::Regular,
        TypographyLevel::Subheadline => FontWeight::Regular,
        TypographyLevel::Footnote => FontWeight::Regular,
        TypographyLevel::Caption1 => FontWeight::Medium,
        TypographyLevel::Caption2 => FontWeight::Regular,
        TypographyLevel::Monospaced => FontWeight::Regular,
    }
}

/// Generate a CSS-style font string for use with RenderContext::set_font
pub fn font_string(level: TypographyLevel) -> String {
    let size = font_size(level);
    let weight = font_weight(level);
    let weight_str = match weight {
        FontWeight::Regular => "",
        FontWeight::Medium => "500 ",
        FontWeight::Semibold => "600 ",
        FontWeight::Bold => "bold ",
    };
    let family = if matches!(level, TypographyLevel::Monospaced) {
        "monospace"
    } else {
        "sans-serif"
    };
    format!("{weight_str}{size}px {family}")
}

/// Line height multiplier for a typography level
pub fn line_height(level: TypographyLevel) -> f64 {
    match level {
        TypographyLevel::LargeTitle => 41.0,
        TypographyLevel::Title1 => 34.0,
        TypographyLevel::Title2 => 28.0,
        TypographyLevel::Title3 => 25.0,
        TypographyLevel::Headline => 22.0,
        TypographyLevel::Body => 16.0,
        TypographyLevel::Callout => 16.0,
        TypographyLevel::Subheadline => 16.0,
        TypographyLevel::Footnote => 14.0,
        TypographyLevel::Caption1 => 14.0,
        TypographyLevel::Caption2 => 13.0,
        TypographyLevel::Monospaced => 16.0,
    }
}
