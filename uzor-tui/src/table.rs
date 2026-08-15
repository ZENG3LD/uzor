//! Table widget: header + rows + column constraints.

use unicode_width::UnicodeWidthStr;

use crate::buffer::TerminalBuffer;
use crate::layout::Constraint;
use crate::rect::Rect;
use crate::style::Style;
use crate::text::Line;
use crate::widget::{Block, Widget};

/// One table cell (content + style). Distinct from `cell::Cell` (a buffer glyph).
#[derive(Debug, Clone, Default)]
pub struct TableCell {
    pub content: Line,
    pub style: Style,
}

impl TableCell {
    pub fn from_line(content: Line) -> Self {
        Self {
            content,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl From<&str> for TableCell {
    fn from(s: &str) -> Self {
        Self::from_line(Line::raw(s))
    }
}

impl From<String> for TableCell {
    fn from(s: String) -> Self {
        Self::from_line(Line::raw(s))
    }
}

impl From<Line> for TableCell {
    fn from(content: Line) -> Self {
        Self::from_line(content)
    }
}

/// One table row.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub cells: Vec<TableCell>,
    pub style: Style,
}

impl Row {
    pub fn new<I, C>(cells: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<TableCell>,
    {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

/// Columnar table with optional header and surrounding block.
#[derive(Debug, Clone)]
pub struct Table {
    header: Option<Row>,
    rows: Vec<Row>,
    widths: Vec<Constraint>,
    block: Option<Block>,
    style: Style,
}

impl Table {
    pub fn new<R, W>(rows: R, widths: W) -> Self
    where
        R: IntoIterator<Item = Row>,
        W: IntoIterator<Item = Constraint>,
    {
        Self {
            header: None,
            rows: rows.into_iter().collect(),
            widths: widths.into_iter().collect(),
            block: None,
            style: Style::default(),
        }
    }

    pub fn header(mut self, header: Row) -> Self {
        self.header = Some(header);
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
}

fn col_widths(constraints: &[Constraint], total: u16) -> Vec<u16> {
    if constraints.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(constraints.len());
    let mut used: u32 = 0;
    let total = total as u32;
    for c in constraints {
        let w = match c {
            Constraint::Fixed(n) | Constraint::Length(n) => *n as u32,
            Constraint::Percentage(p) => total * (*p as u32).min(100) / 100,
            Constraint::Min(n) => (*n as u32).max(1),
            Constraint::Max(n) => *n as u32,
            Constraint::Ratio(num, den) => {
                if *den == 0 {
                    0
                } else {
                    total * *num as u32 / *den as u32
                }
            }
        };
        let w = w.min(total.saturating_sub(used));
        out.push(w as u16);
        used += w;
    }
    out
}

fn paint_line(buf: &mut TerminalBuffer, x: u16, y: u16, max_w: u16, line: &Line, style: Style) {
    if y >= buf.height() || max_w == 0 {
        return;
    }
    let mut col = x;
    let end = x.saturating_add(max_w).min(buf.width());
    for span in &line.spans {
        let st = Style {
            fg: if span.style.fg != crate::style::Color::Reset {
                span.style.fg
            } else {
                style.fg
            },
            bg: if span.style.bg != crate::style::Color::Reset {
                span.style.bg
            } else {
                style.bg
            },
            modifiers: style.modifiers | span.style.modifiers,
        };
        for ch in span.content.chars() {
            if col >= end {
                return;
            }
            let w = UnicodeWidthStr::width(ch.to_string().as_str()) as u16;
            if w == 0 {
                continue;
            }
            if col + w > end {
                break;
            }
            buf.set(col, y, crate::cell::Cell::styled(ch.to_string(), st));
            col += w;
        }
    }
}

fn paint_row(
    buf: &mut TerminalBuffer,
    area: Rect,
    y: u16,
    widths: &[u16],
    row: &Row,
    base: Style,
) {
    let style = Style {
        fg: if row.style.fg != crate::style::Color::Reset {
            row.style.fg
        } else {
            base.fg
        },
        bg: if row.style.bg != crate::style::Color::Reset {
            row.style.bg
        } else {
            base.bg
        },
        modifiers: base.modifiers | row.style.modifiers,
    };
    let mut x = area.left();
    for (i, width) in widths.iter().copied().enumerate() {
        if x >= area.right() {
            break;
        }
        let w = width.min(area.right().saturating_sub(x));
        if let Some(cell) = row.cells.get(i) {
            let cs = Style {
                fg: if cell.style.fg != crate::style::Color::Reset {
                    cell.style.fg
                } else {
                    style.fg
                },
                bg: if cell.style.bg != crate::style::Color::Reset {
                    cell.style.bg
                } else {
                    style.bg
                },
                modifiers: style.modifiers | cell.style.modifiers,
            };
            paint_line(buf, x, y, w, &cell.content, cs);
        }
        x = x.saturating_add(w);
    }
}

impl Widget for Table {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        if area.is_empty() {
            return;
        }
        let content = if let Some(ref block) = self.block {
            block.clone().render(area, buf);
            block.inner(area)
        } else {
            area
        };
        if content.is_empty() {
            return;
        }
        let widths = col_widths(&self.widths, content.width);
        let mut y = content.top();
        if let Some(ref header) = self.header {
            paint_row(buf, content, y, &widths, header, self.style);
            y = y.saturating_add(1);
        }
        for row in &self.rows {
            if y >= content.bottom() {
                break;
            }
            paint_row(buf, content, y, &widths, row, self.style);
            y = y.saturating_add(1);
        }
    }
}

/// Fill `area` with default cells so an overlay does not leak the layer below.
#[derive(Debug, Clone, Copy, Default)]
pub struct Clear;

impl Widget for Clear {
    fn render(self, area: Rect, buf: &mut TerminalBuffer) {
        if area.is_empty() {
            return;
        }
        let w = buf.width();
        let h = buf.height();
        for y in area.top()..area.bottom() {
            if y >= h {
                break;
            }
            for x in area.left()..area.right() {
                if x >= w {
                    break;
                }
                buf.set(x, y, crate::cell::Cell::default());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Color, Modifier};

    #[test]
    fn table_header_and_rows() {
        let mut buf = TerminalBuffer::new(20, 4);
        let table = Table::new(
            vec![Row::new(["a", "1"]), Row::new(["bb", "22"])],
            [Constraint::Length(4), Constraint::Length(4)],
        )
        .header(Row::new(["K", "V"]).style(Style::default().add_modifier(Modifier::BOLD)));
        table.render(Rect::new(0, 0, 20, 4), &mut buf);
        assert_eq!(buf.get(0, 0).symbol, "K");
        assert_eq!(buf.get(4, 0).symbol, "V");
        assert_eq!(buf.get(0, 1).symbol, "a");
        assert_eq!(buf.get(4, 1).symbol, "1");
        assert_eq!(buf.get(0, 2).symbol, "b");
        assert!(buf.get(0, 0).style.modifiers.contains(Modifier::BOLD));
    }

    #[test]
    fn table_inside_block() {
        let mut buf = TerminalBuffer::new(12, 5);
        Table::new(vec![Row::new(["hi"])], [Constraint::Length(8)])
            .block(Block::bordered())
            .render(Rect::new(0, 0, 12, 5), &mut buf);
        assert_eq!(buf.get(0, 0).symbol, "┌");
        assert_eq!(buf.get(1, 1).symbol, "h");
        assert_eq!(buf.get(2, 1).symbol, "i");
    }

    #[test]
    fn clear_resets_cells() {
        let mut buf = TerminalBuffer::new(8, 3);
        buf.set(1, 1, crate::cell::Cell::styled("X", Style::default().fg(Color::Red)));
        Clear.render(Rect::new(1, 1, 2, 1), &mut buf);
        assert_eq!(buf.get(1, 1).symbol, " ");
        assert_eq!(buf.get(1, 1).style.fg, Color::Reset);
    }
}
