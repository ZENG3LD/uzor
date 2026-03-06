//! CSS color parsing for vello backends.
//!
//! Converts CSS color strings to `peniko::Color` (AlphaColor<Srgb>).
//! Supports `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb(r,g,b)`, `rgba(r,g,b,a)`,
//! and a small set of named CSS colors.

use vello::peniko::color::palette;

/// Vello 0.6 color type alias
pub type Color = vello::peniko::color::AlphaColor<vello::peniko::color::Srgb>;

/// Parse a CSS color string to a vello `Color`.
///
/// Supported formats:
/// - `#RGB` — short hex (each nibble doubled)
/// - `#RRGGBB` — standard hex, fully opaque
/// - `#RRGGBBAA` — hex with alpha
/// - `rgb(r, g, b)` — integer 0-255 components
/// - `rgba(r, g, b, a)` — alpha as 0.0-1.0 or 0-255
/// - Named colors: `transparent`, `red`, `green`, `blue`, `white`, `black`
///
/// Returns `BLACK` for any unrecognised input.
pub fn parse_color(color: &str) -> Color {
    let color = color.trim();

    // Handle rgba(r, g, b, a) format
    if let Some(inner) = color.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<u8>().unwrap_or(0);
            let g = parts[1].parse::<u8>().unwrap_or(0);
            let b = parts[2].parse::<u8>().unwrap_or(0);
            // Alpha can be 0.0-1.0 or 0-255
            let a = if let Ok(alpha_f) = parts[3].parse::<f64>() {
                if alpha_f <= 1.0 {
                    (alpha_f * 255.0) as u8
                } else {
                    alpha_f as u8
                }
            } else {
                255
            };
            return Color::from_rgba8(r, g, b, a);
        }
    }

    // Handle rgb(r, g, b) format
    if let Some(inner) = color.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().unwrap_or(0);
            let g = parts[1].parse::<u8>().unwrap_or(0);
            let b = parts[2].parse::<u8>().unwrap_or(0);
            return Color::from_rgba8(r, g, b, 255);
        }
    }

    // Handle hex formats
    let hex = color.trim_start_matches('#');
    let len = hex.len();

    if len == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        return Color::from_rgba8(r, g, b, 255);
    } else if len == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
        return Color::from_rgba8(r, g, b, a);
    } else if len == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
        return Color::from_rgba8(r, g, b, 255);
    }

    // Named CSS colors (small subset commonly used in trading UIs)
    match color {
        "transparent" => palette::css::TRANSPARENT,
        "white" => palette::css::WHITE,
        "black" => palette::css::BLACK,
        "red" => palette::css::RED,
        "green" => palette::css::GREEN,
        "blue" => palette::css::BLUE,
        "yellow" => palette::css::YELLOW,
        "orange" => palette::css::ORANGE,
        "gray" | "grey" => palette::css::GRAY,
        _ => palette::css::BLACK,
    }
}

/// Parse a CSS color string and apply an additional global alpha multiplier.
///
/// `alpha` is in the 0.0–1.0 range. This is useful when a context has a
/// `global_alpha` that modulates all drawing.
pub fn parse_color_with_alpha(color_str: &str, alpha: f64) -> Color {
    let base = parse_color(color_str);
    if alpha >= 1.0 {
        base
    } else {
        base.with_alpha(alpha as f32)
    }
}

/// Parse a CSS color string to `[f32; 4]` (r, g, b, a all in 0.0–1.0) for
/// shader/wgpu use.
pub fn parse_color_to_rgba_f32(color: &str) -> [f32; 4] {
    let color = color.trim();

    // rgba(r,g,b,a)
    if color.starts_with("rgba(") && color.ends_with(')') {
        let inner = &color[5..color.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 4 {
            let r = parts[0].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let g = parts[1].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let b = parts[2].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let a = parts[3].trim().parse::<f32>().unwrap_or(1.0);
            return [r, g, b, a];
        }
    }

    // rgb(r,g,b)
    if color.starts_with("rgb(") && color.ends_with(')') {
        let inner = &color[4..color.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let g = parts[1].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let b = parts[2].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            return [r, g, b, 1.0];
        }
    }

    // Hex (#RRGGBB or #RRGGBBAA)
    let hex = color.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
            [r, g, b, 1.0]
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0;
            [r, g, b, a]
        }
        _ => [1.0, 1.0, 1.0, 0.0], // transparent white fallback
    }
}
