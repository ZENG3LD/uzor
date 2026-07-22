//! Measure-fn-driven word wrap + ellipsis truncate — a small, pure,
//! zero-dependency text-layout utility.
//!
//! Lifted from `foxhound-app-shell-native`'s own hand-rolled `wrap_text`/
//! `truncate` (`nemo/foxhound/crates/foxhound-app-shell-native/src/main.rs:1302-1333`),
//! which that app had to write itself because no equivalent existed
//! anywhere in `uzor` (confirmed by grep — see
//! `nemo/docs/uzor-engines/research_foxhound_lift_candidates.md` §3/§5.4).
//! Ported WITH a polish, not verbatim: the source version wrapped by
//! character COUNT (`line_limit: usize`), which only looks right in a
//! monospace font; this version takes a caller-supplied `measure`
//! closure (typically `|s| ctx.measure_text(s)`) so wrapping is correct
//! against a real proportional font, following this crate's own
//! measure-before-layout convention (`guide::legend`, `guide::tooltip`).
//! Sibling of [`crate::guide::tooltip::draw_tooltip`], which already
//! takes pre-wrapped `lines: &[(String, String)]` — a caller wraps its
//! own long values with [`wrap_text`] before handing them to it.

/// Word-wrap `text` into lines whose measured width (via `measure`) fits
/// within `max_width`, capped at `max_lines`. Words are never split
/// mid-word (a single word wider than `max_width` still gets its own
/// line — the standard word-wrap convention). If the full wrap would
/// need more than `max_lines` lines, the last kept line is UNCONDITIONALLY
/// marked with a trailing ellipsis (shrinking it further if needed to
/// make room) — signaling that content was dropped even when that kept
/// line's own text would otherwise fit `max_width` on its own; mirrors
/// the source app's own `wrap_text`, which always appended `"..."` to
/// the last line whenever the wrapped word count fell short of the
/// original.
///
/// Returns an empty `Vec` for empty/whitespace-only `text` or
/// `max_lines == 0`.
pub fn wrap_text(text: &str, max_width: f64, max_lines: usize, measure: impl Fn(&str) -> f64) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
        if current.is_empty() || measure(&candidate) <= max_width {
            current = candidate;
        } else {
            lines.push(std::mem::replace(&mut current, word.to_owned()));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = force_ellipsis(last, max_width, &measure);
        }
    }
    lines
}

/// Truncate `text` (from the end, by character) until it — plus a
/// trailing ellipsis (`"…"`) — measures within `max_width` via
/// `measure`, appending the ellipsis only if truncation was actually
/// needed. Returns `text` unchanged if it already fits. The general-
/// purpose counterpart of the source app's own standalone `truncate`
/// (used there for entity/confidence/source/caveat fields).
pub fn truncate_ellipsis(text: &str, max_width: f64, measure: impl Fn(&str) -> f64) -> String {
    if measure(text) <= max_width {
        return text.to_owned();
    }
    force_ellipsis(text, max_width, measure)
}

/// Shrink `text` to the largest prefix that fits `max_width` once a
/// trailing ellipsis (`"…"`) is appended, and ALWAYS appends it — unlike
/// [`truncate_ellipsis`], this never returns `text` unmarked, even when
/// `text` itself already fits `max_width` (used by [`wrap_text`]'s own
/// overflow path, where the ellipsis is the "more text follows" signal,
/// not a width-overflow signal).
fn force_ellipsis(text: &str, max_width: f64, measure: impl Fn(&str) -> f64) -> String {
    const ELLIPSIS: &str = "…";
    let chars: Vec<char> = text.chars().collect();
    for len in (0..=chars.len()).rev() {
        let candidate: String = chars[..len].iter().collect::<String>() + ELLIPSIS;
        if len == 0 || measure(&candidate) <= max_width {
            return candidate;
        }
    }
    ELLIPSIS.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial "monospace" stand-in: one unit per character — makes
    /// `max_width` behave exactly like the source app's own
    /// character-count `line_limit`, so these tests can assert on plain
    /// character counts without a real font/`RenderContext`.
    fn char_width(s: &str) -> f64 {
        s.chars().count() as f64
    }

    #[test]
    fn wrap_text_keeps_short_text_on_one_line() {
        let lines = wrap_text("short summary", 40.0, 3, char_width);
        assert_eq!(lines, vec!["short summary".to_owned()]);
    }

    #[test]
    fn wrap_text_breaks_at_word_boundaries_when_a_line_would_overflow() {
        let lines = wrap_text("the quick brown fox jumps", 10.0, 10, char_width);
        for line in &lines {
            assert!(char_width(line) <= 10.0, "line {line:?} exceeds max_width");
        }
        // Every original word must still appear, in order, across the
        // wrapped lines — wrapping must never drop or reorder words.
        let rejoined: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
        assert_eq!(rejoined, vec!["the", "quick", "brown", "fox", "jumps"]);
    }

    #[test]
    fn wrap_text_never_splits_a_single_word_wider_than_max_width() {
        let lines = wrap_text("supercalifragilisticexpialidocious short", 10.0, 10, char_width);
        assert_eq!(lines[0], "supercalifragilisticexpialidocious");
    }

    #[test]
    fn wrap_text_ellipsis_truncates_the_last_line_when_it_exceeds_max_lines() {
        let lines = wrap_text("one two three four five six seven eight", 6.0, 2, char_width);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'), "overflow must be marked with a trailing ellipsis, got {:?}", lines[1]);
    }

    #[test]
    fn wrap_text_returns_empty_for_blank_input_or_zero_max_lines() {
        assert!(wrap_text("", 40.0, 3, char_width).is_empty());
        assert!(wrap_text("   ", 40.0, 3, char_width).is_empty());
        assert!(wrap_text("some text", 40.0, 0, char_width).is_empty());
    }

    #[test]
    fn wrap_text_is_deterministic() {
        let a = wrap_text("alpha beta gamma delta epsilon", 12.0, 4, char_width);
        let b = wrap_text("alpha beta gamma delta epsilon", 12.0, 4, char_width);
        assert_eq!(a, b);
    }

    #[test]
    fn truncate_ellipsis_leaves_text_that_already_fits_unchanged() {
        assert_eq!(truncate_ellipsis("fits fine", 40.0, char_width), "fits fine");
    }

    #[test]
    fn truncate_ellipsis_shortens_and_appends_an_ellipsis_when_it_overflows() {
        let result = truncate_ellipsis("a rather long piece of text", 10.0, char_width);
        assert!(result.ends_with('…'));
        assert!(char_width(&result) <= 10.0);
    }

    #[test]
    fn truncate_ellipsis_degenerates_to_a_bare_ellipsis_when_max_width_cannot_fit_anything_else() {
        let result = truncate_ellipsis("anything", 0.5, char_width);
        assert_eq!(result, "…");
    }

    #[test]
    fn truncate_ellipsis_handles_empty_input_without_panicking() {
        assert_eq!(truncate_ellipsis("", 10.0, char_width), "");
    }
}
