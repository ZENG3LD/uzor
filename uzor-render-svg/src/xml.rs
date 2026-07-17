//! Small string-formatting helpers shared across `context.rs` — XML
//! escaping and compact numeric formatting for `d`/attribute values.

/// Escape the 5 XML-reserved characters for safe use inside text content or
/// a quoted attribute value.
pub(crate) fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Format a coordinate/opacity value rounded to 3 decimal places, with
/// trailing zeros (and a bare trailing `.`) trimmed — keeps generated `d`
/// strings compact without losing meaningful sub-pixel precision.
pub(crate) fn fmt_num(v: f64) -> String {
    let mut rounded = (v * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        rounded = 0.0; // normalize -0.0 to a plain "0"
    }
    let mut s = format!("{rounded:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_all_five_reserved_characters() {
        assert_eq!(escape_xml("<a&b>\"c'd\""), "&lt;a&amp;b&gt;&quot;c&apos;d&quot;");
    }

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(escape_xml("hello world"), "hello world");
    }

    #[test]
    fn trims_trailing_zeros_and_bare_integers() {
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(10.5), "10.5");
        assert_eq!(fmt_num(10.250), "10.25");
        assert_eq!(fmt_num(-0.0001), "0");
    }
}
