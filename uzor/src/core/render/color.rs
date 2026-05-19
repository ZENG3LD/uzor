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

    // Handle hex formats (#RRGGBB, #RRGGBBAA, #RGB, bare RRGGBB).
    //
    // Strict: every character in the hex tail must be a valid ASCII
    // hex digit.  Without this guard a string like `"$bg"` (3 chars)
    // or `"unknown"` (any length we happen to match) used to parse
    // through the radix path, with each non-hex char silently
    // becoming 0 via `unwrap_or(0)`, producing an arbitrary colour
    // (e.g. `"$bg"` → `(0, 187, 0)` green).  Now any unrecognised
    // input falls through to opaque black per the docstring.
    let hex = color.trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return (0, 0, 0, 255);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrecognized_strings_fall_back_to_opaque_black() {
        // 3-char non-hex tokens used to map to arbitrary colours
        // because each non-hex char silently became 0 in the 3-digit
        // hex shorthand path.
        assert_eq!(parse_color("$bg"),  (0, 0, 0, 255));
        assert_eq!(parse_color("$fg"),  (0, 0, 0, 255));
        assert_eq!(parse_color("foo"),  (0, 0, 0, 255));
        assert_eq!(parse_color("unknown"),  (0, 0, 0, 255));
        // 6-char with invalid characters also falls through now.
        assert_eq!(parse_color("zzzzzz"), (0, 0, 0, 255));
        // 8-char with invalid characters falls through.
        assert_eq!(parse_color("not_hex!"), (0, 0, 0, 255));
    }

    #[test]
    fn valid_hex_still_parses() {
        assert_eq!(parse_color("#1a1a1f"), (0x1a, 0x1a, 0x1f, 0xff));
        assert_eq!(parse_color("1a1a1f"),  (0x1a, 0x1a, 0x1f, 0xff));
        assert_eq!(parse_color("#abc"),    (0xaa, 0xbb, 0xcc, 0xff));
        assert_eq!(parse_color("#11223344"), (0x11, 0x22, 0x33, 0x44));
    }

    #[test]
    fn named_colors_still_work() {
        assert_eq!(parse_color("red"),         (255, 0, 0, 255));
        assert_eq!(parse_color("transparent"), (0, 0, 0, 0));
    }

    #[test]
    fn rgb_and_rgba_still_work() {
        assert_eq!(parse_color("rgb(10, 20, 30)"),       (10, 20, 30, 255));
        assert_eq!(parse_color("rgba(10, 20, 30, 0.5)"), (10, 20, 30, 127));
    }
}
