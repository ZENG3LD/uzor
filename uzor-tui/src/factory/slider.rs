//! Slider rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor::widgets::slider::types::SliderType;
use uzor::types::WidgetState;
use super::defaults::*;

/// Render a slider to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    slider: &SliderType,
    area: Rect,
    _state: WidgetState,
) {
    let colors = TuiColors::default();

    match slider {
        SliderType::Single { value, min, max, .. } => {
            // [████░░░░░░] 75%
            let track_width = area.width.saturating_sub(2) as f64; // minus brackets
            let ratio = if max > min {
                ((value - min) / (max - min)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let filled = (track_width * ratio) as u16;
            let empty = (track_width as u16).saturating_sub(filled);

            let fill_style = Style { fg: colors.fg_accent, bg: Color::Reset, modifiers: Modifier::empty() };
            let empty_style = Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() };

            let mut x = area.x;
            set_string(buf, x, area.y, "[", empty_style);
            x += 1;

            for _ in 0..filled {
                if x < area.x + area.width {
                    set_string(buf, x, area.y, TuiIcons::SLIDER_FILL, fill_style);
                    x += 1;
                }
            }

            for _ in 0..empty {
                if x < area.x + area.width {
                    set_string(buf, x, area.y, TuiIcons::SLIDER_EMPTY, empty_style);
                    x += 1;
                }
            }

            if x < area.x + area.width {
                set_string(buf, x, area.y, "]", empty_style);
            }
        }
        SliderType::Dual { min_value, max_value, min, max, .. } => {
            // [░░████░░░░] - range slider
            let track_width = area.width.saturating_sub(2) as f64;
            let range = max - min;

            let low_ratio = if range > 0.0 {
                ((min_value - min) / range).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let high_ratio = if range > 0.0 {
                ((max_value - min) / range).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let low_pos = (track_width * low_ratio) as u16;
            let high_pos = (track_width * high_ratio) as u16;

            let fill_style = Style { fg: colors.fg_accent, bg: Color::Reset, modifiers: Modifier::empty() };
            let empty_style = Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() };

            let mut x = area.x;
            set_string(buf, x, area.y, "[", empty_style);
            x += 1;

            for i in 0..(track_width as u16) {
                if x < area.x + area.width {
                    if i >= low_pos && i <= high_pos {
                        set_string(buf, x, area.y, TuiIcons::SLIDER_FILL, fill_style);
                    } else {
                        set_string(buf, x, area.y, TuiIcons::SLIDER_EMPTY, empty_style);
                    }
                    x += 1;
                }
            }

            if x < area.x + area.width {
                set_string(buf, x, area.y, "]", empty_style);
            }
        }
    }
}

/// Helper: write a string to buffer at (col, row)
fn set_string(buf: &mut TerminalBuffer, col: u16, row: u16, text: &str, style: Style) {
    let mut x = col;
    for ch in text.chars() {
        if x >= buf.width() {
            break;
        }
        if row < buf.height() {
            buf.set(x, row, Cell::styled(ch.to_string(), style));
        }
        x += 1;
    }
}
