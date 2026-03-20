//! Centralized CSS color parsing for uzor render backends.

/// Parse a CSS color string into an `(r, g, b, a)` tuple.
///
/// Supported formats:
/// - `rgba(r, g, b, a)` — alpha can be 0.0–1.0 or 0–255
/// - `rgb(r, g, b)`
/// - Named CSS colors (case-insensitive): white, black, red, green, blue, yellow,
///   cyan/aqua, magenta/fuchsia, orange, gray/grey, silver, maroon, olive, lime,
///   teal, navy, purple, transparent, none
/// - `#RRGGBB`, `#RRGGBBAA`, `#RGB`
/// - Bare `RRGGBB` (no leading `#`)
///
/// Returns `(0, 0, 0, 255)` (opaque black) for any unrecognized input.
pub fn parse_color(color: &str) -> (u8, u8, u8, u8) {
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
            return (r, g, b, a);
        }
    }

    // Handle rgb(r, g, b) format
    if let Some(inner) = color.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().unwrap_or(0);
            let g = parts[1].parse::<u8>().unwrap_or(0);
            let b = parts[2].parse::<u8>().unwrap_or(0);
            return (r, g, b, 255);
        }
    }

    // Named CSS colors
    match color.to_ascii_lowercase().as_str() {
        "white"               => return (255, 255, 255, 255),
        "black"               => return (0,   0,   0,   255),
        "red"                 => return (255, 0,   0,   255),
        "green"               => return (0,   128, 0,   255),
        "blue"                => return (0,   0,   255, 255),
        "yellow"              => return (255, 255, 0,   255),
        "cyan" | "aqua"       => return (0,   255, 255, 255),
        "magenta" | "fuchsia" => return (255, 0,   255, 255),
        "orange"              => return (255, 165, 0,   255),
        "gray" | "grey"       => return (128, 128, 128, 255),
        "silver"              => return (192, 192, 192, 255),
        "maroon"              => return (128, 0,   0,   255),
        "olive"               => return (128, 128, 0,   255),
        "lime"                => return (0,   255, 0,   255),
        "teal"                => return (0,   128, 128, 255),
        "navy"                => return (0,   0,   128, 255),
        "purple"              => return (128, 0,   128, 255),
        "transparent" | "none" => return (0,  0,   0,   0),
        _ => {}
    }

    // Handle hex formats (#RRGGBB, #RRGGBBAA, #RGB, bare RRGGBB)
    let hex = color.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            (r, g, b, a)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            (r, g, b, 255)
        }
        _ => (0, 0, 0, 255), // Default: opaque black
    }
}
