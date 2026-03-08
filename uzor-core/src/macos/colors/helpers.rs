//! Color manipulation helpers

/// Apply alpha transparency to a hex color string
pub fn color_with_alpha(color: &str, alpha: f64) -> String {
    // Remove leading # if present
    let hex = color.strip_prefix('#').unwrap_or(color);

    // Parse RGB components
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    // Return rgba format
    format!("rgba({}, {}, {}, {:.2})", r, g, b, alpha.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_with_alpha() {
        assert_eq!(color_with_alpha("#FFFFFF", 0.5), "rgba(255, 255, 255, 0.50)");
        assert_eq!(color_with_alpha("000000", 0.8), "rgba(0, 0, 0, 0.80)");
        assert_eq!(color_with_alpha("#FF0000", 1.0), "rgba(255, 0, 0, 1.00)");
    }
}
