//! Styled text primitives: Span, Line, Text.

use crate::style::Style;

/// A contiguous run of text with a single style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub content: String,
    pub style: Style,
}

impl Span {
    pub fn raw(content: impl Into<String>) -> Self {
        Self { content: content.into(), style: Style::default() }
    }

    pub fn styled(content: impl Into<String>, style: Style) -> Self {
        Self { content: content.into(), style }
    }

    /// Display width in terminal columns.
    pub fn width(&self) -> usize {
        unicode_width::UnicodeWidthStr::width(self.content.as_str())
    }
}

impl<S: Into<String>> From<S> for Span {
    fn from(s: S) -> Self {
        Self::raw(s)
    }
}

/// A single line of styled text (sequence of Spans).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn raw(content: impl Into<String>) -> Self {
        Self { spans: vec![Span::raw(content)] }
    }

    pub fn styled(content: impl Into<String>, style: Style) -> Self {
        Self { spans: vec![Span::styled(content, style)] }
    }

    pub fn from_spans(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    /// Total display width of all spans.
    pub fn width(&self) -> usize {
        self.spans.iter().map(|s| s.width()).sum()
    }
}

impl<S: Into<String>> From<S> for Line {
    fn from(s: S) -> Self {
        Self::raw(s)
    }
}

/// Multi-line text block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Text {
    pub lines: Vec<Line>,
}

impl Text {
    pub fn raw(content: impl Into<String>) -> Self {
        let s: String = content.into();
        Self {
            lines: s.lines().map(Line::raw).collect(),
        }
    }

    pub fn from_lines(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    /// Height in terminal rows.
    pub fn height(&self) -> usize {
        self.lines.len()
    }

    /// Maximum line width.
    pub fn max_width(&self) -> usize {
        self.lines.iter().map(|l| l.width()).max().unwrap_or(0)
    }
}

/// Horizontal text alignment within a rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Text wrapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    NoWrap,
    WordWrap,
    CharWrap,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn test_span_raw() {
        let s = Span::raw("hello");
        assert_eq!(s.content, "hello");
        assert_eq!(s.style, Style::default());
        assert_eq!(s.width(), 5);
    }

    #[test]
    fn test_span_styled() {
        let style = Style::default().fg(Color::Red);
        let s = Span::styled("hi", style);
        assert_eq!(s.content, "hi");
        assert_eq!(s.style.fg, Color::Red);
    }

    #[test]
    fn test_line_width() {
        let line = Line::from_spans(vec![
            Span::raw("hello"),
            Span::raw(" world"),
        ]);
        assert_eq!(line.width(), 11);
    }

    #[test]
    fn test_text_raw_multiline() {
        let text = Text::raw("line1\nline2\nline3");
        assert_eq!(text.height(), 3);
        assert_eq!(text.lines[0].spans[0].content, "line1");
        assert_eq!(text.lines[2].spans[0].content, "line3");
    }

    #[test]
    fn test_text_max_width() {
        let text = Text::from_lines(vec![
            Line::raw("short"),
            Line::raw("much longer line"),
        ]);
        assert_eq!(text.max_width(), 16);
    }

    #[test]
    fn test_span_from() {
        let s: Span = "test".into();
        assert_eq!(s.content, "test");
    }

    #[test]
    fn test_line_from() {
        let l: Line = "test line".into();
        assert_eq!(l.spans.len(), 1);
        assert_eq!(l.spans[0].content, "test line");
    }
}
