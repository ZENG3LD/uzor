//! Widget traits and built-in widgets: Block, Paragraph, List, Tabs, StatusBar.

use unicode_width::UnicodeWidthStr;

use crate::border::{render_border, render_border_with_title, BorderType, Borders};
use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::rect::Rect;
use crate::style::{Color, Style};
use crate::text::{Alignment, Line, Span, Text, Wrap};

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// A widget that can render itself into a buffer region.
pub trait Widget {
    fn render(self, area: Rect, buf: &mut TerminalBuffer);
}

/// A widget that carries mutable external state across renders.
pub trait StatefulWidget {
    type State;
    fn render(self, area: Rect, buf: &mut TerminalBuffer, state: &mut Self::State);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Merge two styles: span style overrides base where non-default.
fn merge_style(base: Style, over: Style) -> Style {
    Style {
        fg: if over.fg != Color::Reset { over.fg } else { base.fg },
        bg: if over.bg != Color::Reset { over.bg } else { base.bg },
        modifiers: base.modifiers | over.modifiers,
    }
}

/// Render a `Line` at position (x, y) with a maximum width, applying
/// `base_style` under each span's own style. Returns columns written.
fn render_line(
    buf: &mut TerminalBuffer,
    x: u16,
    y: u16,
    max_width: u16,
    line: &Line,
    base_style: Style,
) -> u16 {
    if y >= buf.height() {
        return 0;
    }
    let buf_w = buf.width();
    let mut col = x;
    let end = x.saturating_add(max_width).min(buf_w);

    for span in &line.spans {
        let style = merge_style(base_style, span.style);
        for ch in span.content.chars() {
            if col >= end {
                return col.saturating_sub(x);
            }
            let w = UnicodeWidthStr::width(ch.to_string().as_str()) as u16;
            if w == 0 {
                continue;
            }
            if col + w > end {
                break;
            }
            buf.set(col, y, Cell::styled(ch.to_string(), style));
            col += w;
        }
    }
    col.saturating_sub(x)
}

/// Fill every cell in `area` with the background (and fg/modifiers) of `style`.
fn fill_background(buf: &mut TerminalBuffer, area: Rect, style: Style) {
    let buf_w = buf.width();
    let buf_h = buf.height();
    for row in area.top()..area.bottom() {
        if row >= buf_h {
            break;
        }
        for col in area.left()..area.right() {
            if col >= buf_w {
                break;
            }
            let cell = buf.get_mut(col, row);
            cell.style = merge_style(cell.style, style);
        }
    }
}

// ---------------------------------------------------------------------------
// Block
// ---------------------------------------------------------------------------

/// A bordered box that can contain other widgets.
#[derive(Debug, Clone)]
pub struct Block {
    title: Option<String>,
    title_style: Style,
    borders: Borders,
    border_type: BorderType,
    border_style: Style,
    style: Style,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            title: None,
            title_style: Style::default(),
            borders: Borders::NONE,
            border_type: BorderType::Plain,
            border_style: Style::default(),
            style: Style::default(),
        }
    }
}

impl Block {
    /// Convenience: Block with `Borders::ALL` and `BorderType::Plain`.
    pub fn bordered() -> Self {
        Self {
            borders: Borders::ALL,
            ..Default::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }

    pub fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }

    pub fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_type = border_type;
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Compute the content area inside this block for the given outer area.
    pub fn inner(&self, area: Rect) -> Rect {
        let top = if self.borders.top { 1u16 } else { 0 };
        let bottom = if self.borders.bottom { 1u16 } else { 0 };
        let left = if self.borders.left { 1u16 } else { 0 };
        let right = if self.borders.right { 1u16 } else { 0 };
        area.inset(top, right, bottom, left)
    }

    /// Render the block (background + border) without consuming self.
    fn render_ref(&self, area: Rect, buf: &mut TerminalBuffer) {
        fill_background(buf, area, self.style);

        if let Some(ref title) = self.title {
            render_border_with_title(
                buf,
                area,
                self.border_type,
                self.borders,
                self.border_style,
                title,
                self.title_style,
            );
        } else {
            render_border(buf, area, self.border_type, self.borders, self.border_style);
        }
    }
}

impl Widget for Block {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        self.render_ref(area, buf);
    }
}

// ---------------------------------------------------------------------------
// Paragraph
// ---------------------------------------------------------------------------

/// A text paragraph with optional wrapping, alignment, and scrolling.
#[derive(Debug, Clone)]
pub struct Paragraph {
    text: Text,
    block: Option<Block>,
    style: Style,
    alignment: Alignment,
    wrap: Wrap,
    scroll: (u16, u16),
}

impl Paragraph {
    pub fn new(text: impl Into<Text>) -> Self {
        Self {
            text: text.into(),
            block: None,
            style: Style::default(),
            alignment: Alignment::Left,
            wrap: Wrap::NoWrap,
            scroll: (0, 0),
        }
    }

    pub fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn scroll(mut self, scroll: (u16, u16)) -> Self {
        self.scroll = scroll;
        self
    }
}

/// Word-wrap a single `Line` to fit within `max_width` columns.
/// Returns a list of `Line`s each fitting within the limit.
fn wrap_line(line: &Line, max_width: usize, mode: Wrap) -> Vec<Line> {
    if max_width == 0 {
        return vec![];
    }

    // Flatten into a single string to make wrapping easier.
    let full: String = line.spans.iter().map(|s| s.content.as_str()).collect();
    let full_width = UnicodeWidthStr::width(full.as_str());
    if full_width <= max_width {
        return vec![line.clone()];
    }

    // Collect the style at each character boundary (simplification: use first
    // span style that covers each char).
    let mut char_styles: Vec<Style> = Vec::with_capacity(full.len());
    for span in &line.spans {
        for _ in span.content.chars() {
            char_styles.push(span.style);
        }
    }

    let chars: Vec<char> = full.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let mut col_width: usize = 0;
        let mut end = i;

        // Advance until we exceed max_width
        while end < chars.len() {
            let cw = UnicodeWidthStr::width(chars[end].to_string().as_str());
            if col_width + cw > max_width {
                break;
            }
            col_width += cw;
            end += 1;
        }

        if end == i {
            // Single char wider than max_width, force advance
            end = i + 1;
        }

        // For WordWrap, try to break at a space boundary
        if mode == Wrap::WordWrap && end < chars.len() {
            if let Some(space_pos) = chars[i..end].iter().rposition(|c| *c == ' ') {
                let break_at = i + space_pos + 1;
                if break_at > i {
                    end = break_at;
                }
            }
        }

        // Build spans for this wrapped line, grouping consecutive chars with same style
        let mut spans = Vec::new();
        let mut j = i;
        while j < end {
            let style = char_styles[j];
            let mut s = String::new();
            while j < end && char_styles[j] == style {
                s.push(chars[j]);
                j += 1;
            }
            spans.push(Span { content: s, style });
        }
        result.push(Line { spans });
        i = end;
    }

    result
}

impl Widget for Paragraph {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        if area.is_empty() {
            return;
        }

        let content_area = if let Some(ref block) = self.block {
            block.render_ref(area, buf);
            block.inner(area)
        } else {
            area
        };

        if content_area.is_empty() {
            return;
        }

        fill_background(buf, content_area, self.style);

        let cw = content_area.width as usize;

        // Produce the final list of lines (possibly wrapped)
        let lines: Vec<Line> = match self.wrap {
            Wrap::NoWrap => self.text.lines.clone(),
            Wrap::WordWrap | Wrap::CharWrap => {
                let mut wrapped = Vec::new();
                for line in &self.text.lines {
                    wrapped.extend(wrap_line(line, cw, self.wrap));
                }
                wrapped
            }
        };

        let v_scroll = self.scroll.0 as usize;
        let h_scroll = self.scroll.1;

        let visible = lines.iter().skip(v_scroll).take(content_area.height as usize);

        for (row_idx, line) in visible.enumerate() {
            let y = content_area.top() + row_idx as u16;
            if y >= content_area.bottom() {
                break;
            }

            // If h_scroll, build a trimmed line skipping first N columns
            let effective_line = if h_scroll > 0 {
                trim_line_left(line, h_scroll)
            } else {
                line.clone()
            };

            let line_w = effective_line.width() as u16;
            let remaining = content_area.width.saturating_sub(line_w);

            let x = match self.alignment {
                Alignment::Left => content_area.left(),
                Alignment::Center => content_area.left() + remaining / 2,
                Alignment::Right => content_area.left() + remaining,
            };

            render_line(buf, x, y, content_area.width, &effective_line, self.style);
        }
    }
}

/// Trim `skip` columns from the left of a line.
fn trim_line_left(line: &Line, skip: u16) -> Line {
    let mut remaining = skip as usize;
    let mut spans = Vec::new();
    for span in &line.spans {
        if remaining == 0 {
            spans.push(span.clone());
            continue;
        }
        let mut new_content = String::new();
        for ch in span.content.chars() {
            let w = UnicodeWidthStr::width(ch.to_string().as_str());
            if remaining >= w {
                remaining -= w;
            } else {
                remaining = 0;
                new_content.push(ch);
            }
        }
        if !new_content.is_empty() {
            spans.push(Span {
                content: new_content,
                style: span.style,
            });
        }
    }
    Line { spans }
}

// Conversions for Paragraph::new convenience
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::raw(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::raw(s)
    }
}

// ---------------------------------------------------------------------------
// List + ListState
// ---------------------------------------------------------------------------

/// A scrollable list of items.
#[derive(Debug, Clone)]
pub struct List {
    items: Vec<Line>,
    block: Option<Block>,
    style: Style,
    highlight_style: Style,
    highlight_symbol: Option<String>,
}

impl List {
    pub fn new(items: Vec<Line>) -> Self {
        Self {
            items,
            block: None,
            style: Style::default(),
            highlight_style: Style::default(),
            highlight_symbol: None,
        }
    }

    pub fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    pub fn highlight_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.highlight_symbol = Some(symbol.into());
        self
    }
}

/// Persistent scroll/selection state for a `List`.
#[derive(Debug, Clone, Default)]
pub struct ListState {
    pub offset: usize,
    pub selected: Option<usize>,
}

impl ListState {
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn select_next(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) if i + 1 < total => i + 1,
            Some(_) => 0,
            None => 0,
        });
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.map(|i| i.saturating_sub(1));
    }

    pub fn select_first(&mut self) {
        self.selected = Some(0);
    }

    pub fn select_last(&mut self, total: usize) {
        if total > 0 {
            self.selected = Some(total - 1);
        }
    }
}

impl StatefulWidget for List {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut TerminalBuffer, state: &mut ListState) {
        if area.is_empty() {
            return;
        }

        let content_area = if let Some(ref block) = self.block {
            block.render_ref(area, buf);
            block.inner(area)
        } else {
            area
        };

        if content_area.is_empty() {
            return;
        }

        fill_background(buf, content_area, self.style);

        let visible_height = content_area.height as usize;
        let total = self.items.len();

        // Auto-scroll so the selected item is visible
        if let Some(sel) = state.selected {
            if sel < state.offset {
                state.offset = sel;
            } else if sel >= state.offset + visible_height {
                state.offset = sel.saturating_sub(visible_height - 1);
            }
        }

        let sym_width = self
            .highlight_symbol
            .as_ref()
            .map(|s| UnicodeWidthStr::width(s.as_str()) as u16)
            .unwrap_or(0);

        for (i, item_idx) in (state.offset..total)
            .take(visible_height)
            .enumerate()
        {
            let y = content_area.top() + i as u16;
            if y >= content_area.bottom() {
                break;
            }

            let is_selected = state.selected == Some(item_idx);
            let item = &self.items[item_idx];

            let mut x = content_area.left();
            let mut available = content_area.width;

            if is_selected {
                // Render highlight symbol
                if let Some(ref sym) = self.highlight_symbol {
                    let sym_line = Line::raw(sym.as_str());
                    let written = render_line(buf, x, y, available, &sym_line, self.highlight_style);
                    x += written;
                    available = available.saturating_sub(written);
                }
                // Fill rest of row with highlight style bg
                for c in x..content_area.right() {
                    if c < buf.width() && y < buf.height() {
                        let cell = buf.get_mut(c, y);
                        cell.style = merge_style(cell.style, self.highlight_style);
                    }
                }
                render_line(buf, x, y, available, item, self.highlight_style);
            } else {
                if sym_width > 0 {
                    // Indent non-selected items by the same width for alignment
                    x += sym_width;
                    available = available.saturating_sub(sym_width);
                }
                render_line(buf, x, y, available, item, self.style);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// Horizontal tab bar.
#[derive(Debug, Clone)]
pub struct Tabs {
    titles: Vec<String>,
    selected: usize,
    block: Option<Block>,
    style: Style,
    highlight_style: Style,
    divider: String,
}

impl Tabs {
    pub fn new(titles: Vec<String>) -> Self {
        Self {
            titles,
            selected: 0,
            block: None,
            style: Style::default(),
            highlight_style: Style::default(),
            divider: " | ".to_string(),
        }
    }

    pub fn select(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    pub fn divider(mut self, divider: impl Into<String>) -> Self {
        self.divider = divider.into();
        self
    }
}

impl Widget for Tabs {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        if area.is_empty() {
            return;
        }

        let content_area = if let Some(ref block) = self.block {
            block.render_ref(area, buf);
            block.inner(area)
        } else {
            area
        };

        if content_area.is_empty() {
            return;
        }

        fill_background(buf, content_area, self.style);

        let y = content_area.top();
        let mut x = content_area.left();
        let end_x = content_area.right();

        for (i, title) in self.titles.iter().enumerate() {
            if x >= end_x {
                break;
            }

            // Render divider before non-first titles
            if i > 0 {
                let div_line = Line::raw(self.divider.as_str());
                let written = render_line(buf, x, y, end_x.saturating_sub(x), &div_line, self.style);
                x += written;
                if x >= end_x {
                    break;
                }
            }

            let style = if i == self.selected {
                self.highlight_style
            } else {
                self.style
            };

            let title_line = Line::raw(title.as_str());
            let written = render_line(buf, x, y, end_x.saturating_sub(x), &title_line, style);
            x += written;
        }
    }
}

// ---------------------------------------------------------------------------
// StatusBar
// ---------------------------------------------------------------------------

/// A single-row status bar with left, center, and right sections.
#[derive(Debug, Clone, Default)]
pub struct StatusBar {
    left: Option<Line>,
    center: Option<Line>,
    right: Option<Line>,
    style: Style,
}

impl StatusBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn left(mut self, line: impl Into<Line>) -> Self {
        self.left = Some(line.into());
        self
    }

    pub fn center(mut self, line: impl Into<Line>) -> Self {
        self.center = Some(line.into());
        self
    }

    pub fn right(mut self, line: impl Into<Line>) -> Self {
        self.right = Some(line.into());
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        if area.is_empty() {
            return;
        }

        fill_background(buf, area, self.style);

        let y = area.top();
        let total_width = area.width;

        // Left section
        if let Some(ref line) = self.left {
            render_line(buf, area.left(), y, total_width, line, self.style);
        }

        // Center section
        if let Some(ref line) = self.center {
            let lw = line.width() as u16;
            let x = area.left() + total_width.saturating_sub(lw) / 2;
            render_line(buf, x, y, total_width, line, self.style);
        }

        // Right section
        if let Some(ref line) = self.right {
            let lw = line.width() as u16;
            let x = area.right().saturating_sub(lw);
            render_line(buf, x, y, lw, line, self.style);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TerminalBuffer;
    use crate::style::{Color, Style};

    /// Helper: collect row of symbols from buffer as a string.
    fn row_text(buf: &TerminalBuffer, y: u16, x_start: u16, x_end: u16) -> String {
        (x_start..x_end).map(|x| buf.get(x, y).symbol.as_str()).collect()
    }

    // -- Block ---------------------------------------------------------------

    #[test]
    fn test_block_inner() {
        let block = Block::bordered();
        let area = Rect::new(0, 0, 20, 10);
        let inner = block.inner(area);
        assert_eq!(inner, Rect::new(1, 1, 18, 8));
    }

    #[test]
    fn test_block_no_borders_inner() {
        let block = Block::default();
        let area = Rect::new(5, 5, 20, 10);
        let inner = block.inner(area);
        assert_eq!(inner, area);
    }

    #[test]
    fn test_block_render_bordered() {
        let mut buf = TerminalBuffer::new(10, 5);
        let area = Rect::new(0, 0, 10, 5);
        Block::bordered().render(area, &mut buf);

        assert_eq!(buf.get(0, 0).symbol, "┌");
        assert_eq!(buf.get(9, 0).symbol, "┐");
        assert_eq!(buf.get(0, 4).symbol, "└");
        assert_eq!(buf.get(9, 4).symbol, "┘");
        assert_eq!(buf.get(5, 0).symbol, "─");
        assert_eq!(buf.get(0, 2).symbol, "│");
    }

    // -- Paragraph -----------------------------------------------------------

    #[test]
    fn test_paragraph_simple() {
        let mut buf = TerminalBuffer::new(20, 5);
        let area = Rect::new(0, 0, 20, 5);
        let p = Paragraph::new(Text::raw("Hello"));
        p.render(area, &mut buf);

        assert_eq!(row_text(&buf, 0, 0, 5), "Hello");
    }

    #[test]
    fn test_paragraph_with_block() {
        let mut buf = TerminalBuffer::new(20, 5);
        let area = Rect::new(0, 0, 20, 5);
        let p = Paragraph::new(Text::raw("Hi")).block(Block::bordered());
        p.render(area, &mut buf);

        // Border on row 0
        assert_eq!(buf.get(0, 0).symbol, "┌");
        // Text at (1, 1) inside border
        assert_eq!(buf.get(1, 1).symbol, "H");
        assert_eq!(buf.get(2, 1).symbol, "i");
    }

    #[test]
    fn test_paragraph_alignment_center() {
        let mut buf = TerminalBuffer::new(20, 3);
        let area = Rect::new(0, 0, 20, 3);
        let p = Paragraph::new(Text::raw("AB")).alignment(Alignment::Center);
        p.render(area, &mut buf);

        // "AB" is 2 wide, area is 20 wide, centered => starts at col 9
        assert_eq!(buf.get(9, 0).symbol, "A");
        assert_eq!(buf.get(10, 0).symbol, "B");
    }

    #[test]
    fn test_paragraph_scroll() {
        let mut buf = TerminalBuffer::new(20, 2);
        let area = Rect::new(0, 0, 20, 2);
        let text = Text::from_lines(vec![
            Line::raw("line0"),
            Line::raw("line1"),
            Line::raw("line2"),
        ]);
        let p = Paragraph::new(text).scroll((1, 0));
        p.render(area, &mut buf);

        // Should skip line0, show line1 at row 0
        assert_eq!(row_text(&buf, 0, 0, 5), "line1");
        assert_eq!(row_text(&buf, 1, 0, 5), "line2");
    }

    // -- List ----------------------------------------------------------------

    #[test]
    fn test_list_render() {
        let mut buf = TerminalBuffer::new(20, 5);
        let area = Rect::new(0, 0, 20, 5);
        let list = List::new(vec![
            Line::raw("Item A"),
            Line::raw("Item B"),
            Line::raw("Item C"),
        ]);
        let mut state = ListState::default();
        list.render(area, &mut buf, &mut state);

        assert_eq!(row_text(&buf, 0, 0, 6), "Item A");
        assert_eq!(row_text(&buf, 1, 0, 6), "Item B");
        assert_eq!(row_text(&buf, 2, 0, 6), "Item C");
    }

    #[test]
    fn test_list_highlight() {
        let mut buf = TerminalBuffer::new(20, 5);
        let area = Rect::new(0, 0, 20, 5);
        let list = List::new(vec![
            Line::raw("One"),
            Line::raw("Two"),
        ])
        .highlight_symbol("> ")
        .highlight_style(Style::default().fg(Color::Yellow));

        let mut state = ListState { offset: 0, selected: Some(1) };
        list.render(area, &mut buf, &mut state);

        // Selected item (index 1) at row 1 should have "> " prefix
        assert_eq!(buf.get(0, 1).symbol, ">");
        assert_eq!(buf.get(1, 1).symbol, " ");
        assert_eq!(buf.get(2, 1).symbol, "T");
        // Non-selected (index 0) at row 0 should be indented by 2
        assert_eq!(buf.get(2, 0).symbol, "O");
    }

    // -- Tabs ----------------------------------------------------------------

    #[test]
    fn test_tabs_render() {
        let mut buf = TerminalBuffer::new(40, 1);
        let area = Rect::new(0, 0, 40, 1);
        let tabs = Tabs::new(vec!["Tab1".into(), "Tab2".into(), "Tab3".into()])
            .select(1)
            .highlight_style(Style::default().fg(Color::Green));
        tabs.render(area, &mut buf);

        // "Tab1 | Tab2 | Tab3"
        assert_eq!(row_text(&buf, 0, 0, 4), "Tab1");
        assert_eq!(row_text(&buf, 0, 4, 7), " | ");
        assert_eq!(row_text(&buf, 0, 7, 11), "Tab2");
        // Tab2 should have Green fg (selected)
        assert_eq!(buf.get(7, 0).style.fg, Color::Green);
    }

    // -- StatusBar -----------------------------------------------------------

    #[test]
    fn test_status_bar() {
        let mut buf = TerminalBuffer::new(40, 1);
        let area = Rect::new(0, 0, 40, 1);
        let bar = StatusBar::new()
            .left("LEFT")
            .center("MID")
            .right("RIGHT");
        bar.render(area, &mut buf);

        // Left at col 0
        assert_eq!(row_text(&buf, 0, 0, 4), "LEFT");
        // Center: "MID" is 3 wide, (40-3)/2 = 18
        assert_eq!(row_text(&buf, 0, 18, 21), "MID");
        // Right: "RIGHT" is 5 wide, 40-5 = 35
        assert_eq!(row_text(&buf, 0, 35, 40), "RIGHT");
    }

    // -- fill_background -----------------------------------------------------

    #[test]
    fn test_fill_background() {
        let mut buf = TerminalBuffer::new(10, 5);
        let area = Rect::new(1, 1, 3, 2);
        let style = Style::default().bg(Color::Blue);
        fill_background(&mut buf, area, style);

        assert_eq!(buf.get(1, 1).style.bg, Color::Blue);
        assert_eq!(buf.get(3, 2).style.bg, Color::Blue);
        // Outside area unchanged
        assert_eq!(buf.get(0, 0).style.bg, Color::Reset);
    }
}
