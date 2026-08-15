//! Flush [`uzor_text::ascii::AsciiGrid`] into a [`TerminalBuffer`].
//!
//! The shader (`step`) is already a cell program. `AsciiGrid::render`
//! paints through `RenderContext` (pixel fill_text) — that path stays
//! on the desktop/proof side. This module is the terminal path:
//! character + RGB → one buffer cell. `alpha`/`scale` degrade to skip
//! or `DIM`; they have no sub-cell size in a real terminal.

use uzor_text::ascii::AsciiGrid;

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::rect::Rect;
use crate::style::{Color, Modifier, Style};

const ALPHA_SKIP: f32 = 0.01;
const ALPHA_DIM: f32 = 0.5;
const SCALE_DIM: f32 = 0.7;

/// Write ink cells of `grid` into `buf`, origin = `dest` top-left.
/// Spaces and near-zero alpha are skipped so a field can overlay a panel.
pub fn blit_ascii_grid(buf: &mut TerminalBuffer, dest: Rect, grid: &AsciiGrid) {
    if dest.is_empty() {
        return;
    }
    grid.for_each(|x, y, c| {
        if c.ch == ' ' || c.alpha <= ALPHA_SKIP {
            return;
        }
        let dx = dest.x.saturating_add(x as u16);
        let dy = dest.y.saturating_add(y as u16);
        if !dest.contains(dx, dy) {
            return;
        }
        let mut style = Style::default().fg(Color::Rgb(c.color[0], c.color[1], c.color[2]));
        if c.alpha < ALPHA_DIM || c.scale < SCALE_DIM {
            style = style.add_modifier(Modifier::DIM);
        }
        buf.set(dx, dy, Cell::styled(c.ch.to_string(), style));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_text::ascii::{AsciiGrid, Cursor, GlitchLetter};

    #[test]
    fn letter_m_at_rest_lands_in_the_cell_buffer() {
        let shader = GlitchLetter::letter_m();
        let mut grid = AsciiGrid::new(shader.cols, shader.rows);
        grid.set_cursor(Cursor {
            x: 0.0,
            y: 0.0,
            pressed: false,
            inside: false,
            intensity: 0.0,
        });
        grid.step(&shader, 0.0, 1.0);

        let mut buf = TerminalBuffer::new(8, 8);
        blit_ascii_grid(&mut buf, Rect::new(1, 1, 6, 7), &grid);

        // Mask row 0 is X...X → '#' at dest (1,1) and (5,1).
        assert_eq!(buf.get(1, 1).symbol, "#");
        assert_eq!(buf.get(5, 1).symbol, "#");
        assert_eq!(buf.get(2, 1).symbol, " ");
        match buf.get(1, 1).style.fg {
            Color::Rgb(251, 178, 106) => {}
            other => panic!("expected amber, got {other:?}"),
        }
    }
}
