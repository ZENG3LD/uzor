//! Paint a [`Card`] into a frame. Returns the body rect for the app.

use crate::cell::Cell;
use crate::frame::Frame;
use crate::rect::Rect;
use crate::style::{Modifier, Style};
use crate::text::Line;
use crate::widget::Paragraph;

use super::card::{Card, CardFooter, CardTab, FooterRule};
use super::hit::{Hit, HitLayer};
use super::theme::ShellTheme;

const CLOSE_W: u16 = 5;

/// Painted overlay: frame, body, hits for this pass.
pub struct PaintedCard {
    pub frame: Rect,
    pub body: Rect,
}

pub fn paint_card(
    f: &mut Frame<'_>,
    area: Rect,
    card: &Card,
    pos: Option<(u16, u16)>,
    theme: ShellTheme,
    hits: &mut HitLayer,
) -> Option<PaintedCard> {
    if area.width < 10 || area.height < 6 {
        return None;
    }
    let max_w = area.width.saturating_sub(2).max(1);
    let max_h = area.height.saturating_sub(2).max(1);
    let side_w = card.side_w();
    let foot_h = card.footer_h();
    let header_h: u16 = 1;
    let chrome_h = 1 + header_h + 1 + foot_h;
    let w = card.prefer_w.clamp(card.min_w, max_w).max(12);
    let h = card
        .prefer_h
        .clamp(card.min_h.max(chrome_h + 2), max_h)
        .max(chrome_h + 2);
    let (x, y) = pos.unwrap_or_else(|| {
        (
            area.x + (area.width.saturating_sub(w)) / 2,
            area.y + (area.height.saturating_sub(h)) / 2,
        )
    });
    let frame = Rect {
        x: x.min(area.x + area.width.saturating_sub(w)),
        y: y.min(area.y + area.height.saturating_sub(h)),
        width: w,
        height: h,
    };

    let border = Style::default().fg(theme.border).bg(theme.modal);
    fill_rect(f, frame, Style::default().fg(theme.text).bg(theme.modal));
    stroke(f, frame, border);

    let close = Rect {
        x: frame.x + frame.width.saturating_sub(CLOSE_W + 1),
        y: frame.y,
        width: CLOSE_W,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from("[x]")).style(Style::default().fg(theme.muted).bg(theme.modal)),
        Rect {
            x: close.x + 1,
            y: close.y,
            width: 3,
            height: 1,
        },
    );

    let header = Rect {
        x: frame.x + 1,
        y: frame.y + 1,
        width: frame.width.saturating_sub(2),
        height: header_h,
    };
    let top_hits = paint_top_tabs(f, header, &card.top_tabs, card.top_active, theme);

    let inner_top = frame.y + 1 + header_h;
    let inner_bot = frame.y + frame.height.saturating_sub(1 + foot_h);
    let side = if side_w > 0 {
        Some(Rect {
            x: frame.x + 1,
            y: inner_top,
            width: side_w.min(frame.width.saturating_sub(4)),
            height: inner_bot.saturating_sub(inner_top),
        })
    } else {
        None
    };
    let side_hits = if let Some(sr) = side {
        vline(f, sr.x + sr.width, sr.y, sr.height, border);
        paint_side_tabs(f, sr, &card.side_tabs, card.side_active, theme)
    } else {
        Vec::new()
    };

    let body_x = match side {
        Some(sr) => sr.x + sr.width + 2,
        None => frame.x + 2,
    };
    let body = Rect {
        x: body_x,
        y: inner_top,
        width: (frame.x + frame.width).saturating_sub(body_x + 1),
        height: inner_bot.saturating_sub(inner_top),
    };

    if foot_h > 0 {
        let fy = inner_bot;
        if footer_has_rule(&card.footer) {
            hline(f, frame.x + 1, fy, frame.width.saturating_sub(2), border);
        }
        let foot = Rect {
            x: frame.x + 1,
            y: fy + u16::from(footer_has_rule(&card.footer)),
            width: frame.width.saturating_sub(2),
            height: foot_h.saturating_sub(u16::from(footer_has_rule(&card.footer))),
        };
        paint_footer(f, foot, &card.footer, theme, hits);
    }

    hits.push(body, Hit::Body);
    hits.push(
        Rect {
            x: frame.x,
            y: frame.y,
            width: frame.width.saturating_sub(CLOSE_W + 1),
            height: 1 + header_h,
        },
        Hit::OverlayDrag,
    );
    for (r, id) in top_hits {
        hits.push(r, Hit::TopTab(id));
    }
    for (r, id) in side_hits {
        hits.push(r, Hit::SideTab(id));
    }
    hits.push(close, Hit::OverlayClose);

    if body.is_empty() {
        return None;
    }
    Some(PaintedCard { frame, body })
}

fn footer_has_rule(f: &CardFooter) -> bool {
    matches!(
        f,
        CardFooter::Info {
            rule: FooterRule::Line,
            ..
        } | CardFooter::Buttons {
            rule: FooterRule::Line,
            ..
        }
    )
}

fn paint_top_tabs(
    f: &mut Frame<'_>,
    header: Rect,
    tabs: &[CardTab],
    active: u8,
    theme: ShellTheme,
) -> Vec<(Rect, u8)> {
    let mut hits = Vec::new();
    let mut cx = header.x;
    for tab in tabs {
        let room = header.width.saturating_sub(cx.saturating_sub(header.x));
        let label = truncate(&tab.label, room.saturating_sub(1) as usize);
        if label.is_empty() {
            break;
        }
        let tw = label.chars().count() as u16;
        if cx + tw > header.x + header.width {
            break;
        }
        let style = if tab.id == active {
            Style::default()
                .fg(theme.text)
                .bg(theme.modal)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted).bg(theme.modal)
        };
        let r = Rect {
            x: cx,
            y: header.y,
            width: tw,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(label)).style(style), r);
        hits.push((r, tab.id));
        cx = cx.saturating_add(tw).saturating_add(2);
    }
    hits
}

fn paint_side_tabs(
    f: &mut Frame<'_>,
    rail: Rect,
    tabs: &[CardTab],
    active: u8,
    theme: ShellTheme,
) -> Vec<(Rect, u8)> {
    let mut hits = Vec::new();
    for (i, tab) in tabs.iter().enumerate() {
        let y = rail.y + i as u16;
        if y >= rail.y + rail.height {
            break;
        }
        let style = if tab.id == active {
            Style::default()
                .fg(theme.text)
                .bg(theme.modal)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted).bg(theme.modal)
        };
        let r = Rect {
            x: rail.x,
            y,
            width: rail.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(truncate(&tab.label, rail.width as usize))).style(style),
            r,
        );
        hits.push((r, tab.id));
    }
    hits
}

fn paint_footer(
    f: &mut Frame<'_>,
    area: Rect,
    footer: &CardFooter,
    theme: ShellTheme,
    hits: &mut HitLayer,
) {
    if area.is_empty() {
        return;
    }
    match footer {
        CardFooter::None => {}
        CardFooter::Info { lines, .. } => {
            let muted = Style::default().fg(theme.muted).bg(theme.modal);
            for (i, line) in lines.iter().enumerate() {
                let y = area.y + i as u16;
                if y >= area.y + area.height {
                    break;
                }
                paint_center(f, area.x, y, area.width, line, muted);
            }
        }
        CardFooter::Buttons { buttons, .. } => {
            let labels: Vec<String> = buttons
                .iter()
                .map(|b| format!(" {} ", b.label))
                .collect();
            let total: u16 = labels
                .iter()
                .map(|s| s.chars().count() as u16)
                .sum::<u16>()
                + labels.len().saturating_sub(1) as u16;
            let mut x = area.x + area.width.saturating_sub(total) / 2;
            let y = area.y + area.height.saturating_sub(1);
            for (b, label) in buttons.iter().zip(labels) {
                let tw = label.chars().count() as u16;
                if x + tw > area.x + area.width {
                    break;
                }
                let style = if b.pending {
                    Style::default().fg(theme.button_fg).bg(theme.pending_bg)
                } else {
                    Style::default().fg(theme.button_fg).bg(theme.button_bg)
                };
                let r = Rect {
                    x,
                    y,
                    width: tw,
                    height: 1,
                };
                f.render_widget(Paragraph::new(Line::from(label)).style(style), r);
                hits.push(r, Hit::FooterButton(b.id));
                x = x.saturating_add(tw).saturating_add(1);
            }
        }
    }
}

fn paint_center(f: &mut Frame<'_>, x: u16, y: u16, w: u16, text: &str, style: Style) {
    let n = text.chars().count() as u16;
    let tx = x + w.saturating_sub(n) / 2;
    let tw = n.min(w);
    if tw == 0 {
        return;
    }
    let shown: String = text.chars().take(tw as usize).collect();
    f.render_widget(
        Paragraph::new(Line::from(shown)).style(style),
        Rect {
            x: tx,
            y,
            width: tw,
            height: 1,
        },
    );
}

fn fill_rect(f: &mut Frame<'_>, rect: Rect, style: Style) {
    let buf = f.buffer_mut();
    let x2 = rect.right().min(buf.width());
    let y2 = rect.bottom().min(buf.height());
    for y in rect.y..y2 {
        for x in rect.x..x2 {
            buf.set(x, y, Cell::styled(" ", style));
        }
    }
}

fn stroke(f: &mut Frame<'_>, rect: Rect, style: Style) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let buf = f.buffer_mut();
    let x1 = rect.x;
    let y1 = rect.y;
    let x2 = rect.x + rect.width - 1;
    let y2 = rect.y + rect.height - 1;
    let mut put = |x: u16, y: u16, ch: &str| {
        if x < buf.width() && y < buf.height() {
            buf.set(x, y, Cell::styled(ch, style));
        }
    };
    put(x1, y1, "┌");
    put(x2, y1, "┐");
    put(x1, y2, "└");
    put(x2, y2, "┘");
    for x in x1 + 1..x2 {
        put(x, y1, "─");
        put(x, y2, "─");
    }
    for y in y1 + 1..y2 {
        put(x1, y, "│");
        put(x2, y, "│");
    }
}

fn hline(f: &mut Frame<'_>, x: u16, y: u16, w: u16, style: Style) {
    let buf = f.buffer_mut();
    for i in 0..w {
        let px = x + i;
        if px < buf.width() && y < buf.height() {
            buf.set(px, y, Cell::styled("─", style));
        }
    }
}

fn vline(f: &mut Frame<'_>, x: u16, y: u16, h: u16, style: Style) {
    let buf = f.buffer_mut();
    for i in 0..h {
        let py = y + i;
        if x < buf.width() && py < buf.height() {
            buf.set(x, py, Cell::styled("│", style));
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".into();
    }
    s.chars().take(max - 1).collect::<String>() + "…"
}
