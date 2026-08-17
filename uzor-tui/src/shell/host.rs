//! Overlay host: paint + mouse + keymap. The app owns what the body means.

use crate::frame::Frame;
use crate::rect::Rect;

use super::card::Card;
use super::hit::{Hit, HitLayer};
use super::input::{Command, Event, Keymap, MouseKind};
use super::paint::{paint_card, PaintedCard};
use super::theme::ShellTheme;

const DRAG_THRESHOLD: u16 = 2;

/// One overlay. A third app holds this instead of rolling its own drag/hits.
pub struct OverlayHost {
    pub open: bool,
    pub card: Card,
    pub theme: ShellTheme,
    pub frame: Option<Rect>,
    pub body: Option<Rect>,
    pub pos: Option<(u16, u16)>,
    hits: HitLayer,
    drag: Option<Drag>,
}

struct Drag {
    dx: i32,
    dy: i32,
    start: (u16, u16),
    moved: bool,
}

impl OverlayHost {
    pub fn new(card: Card) -> Self {
        Self {
            open: true,
            card,
            theme: ShellTheme::default(),
            frame: None,
            body: None,
            pos: None,
            hits: HitLayer::new(),
            drag: None,
        }
    }

    pub fn closed() -> Self {
        let mut h = Self::new(Card::new(40, 12));
        h.open = false;
        h
    }

    pub fn close(&mut self) {
        self.open = false;
        self.drag = None;
        self.hits.clear();
        self.frame = None;
        self.body = None;
    }

    /// Paint chrome. Returns the body rect so the app can fill it.
    pub fn paint(&mut self, f: &mut Frame<'_>, area: Rect) -> Option<Rect> {
        if !self.open {
            return None;
        }
        self.hits.clear();
        let painted: PaintedCard = paint_card(f, area, &self.card, self.pos, self.theme, &mut self.hits)?;
        self.frame = Some(painted.frame);
        self.body = Some(painted.body);
        Some(painted.body)
    }

    pub fn handle(&mut self, ev: Event, map: &Keymap) -> Option<Command> {
        if !self.open {
            return None;
        }
        match ev {
            Event::Key(k) => map.lookup(k),
            Event::Mouse(m) => self.mouse(m),
        }
    }

    fn mouse(&mut self, m: super::input::Mouse) -> Option<Command> {
        match m.kind {
            MouseKind::Down => self.mouse_down(m.x, m.y),
            MouseKind::Drag => {
                self.mouse_drag(m.x, m.y);
                None
            }
            MouseKind::Up => {
                self.drag = None;
                None
            }
            MouseKind::Scroll { .. } => None,
        }
    }

    fn mouse_down(&mut self, x: u16, y: u16) -> Option<Command> {
        let inside = self.frame.is_some_and(|r| r.contains(x, y));
        match self.hits.hit(x, y) {
            Some(Hit::OverlayClose) => {
                self.close();
                Some(Command::Close)
            }
            Some(Hit::TopTab(id)) => Some(Command::TopTab(id)),
            Some(Hit::SideTab(id)) => Some(Command::SideTab(id)),
            Some(Hit::FooterButton(id)) => Some(Command::Button(id)),
            Some(Hit::OverlayDrag) => {
                if let Some(r) = self.frame {
                    self.drag = Some(Drag {
                        dx: x as i32 - r.x as i32,
                        dy: y as i32 - r.y as i32,
                        start: (x, y),
                        moved: false,
                    });
                }
                None
            }
            Some(Hit::Body | Hit::Custom(_)) => None,
            None if inside => None,
            None if self.card.dismiss_outside => {
                self.close();
                Some(Command::Close)
            }
            None => None,
        }
    }

    fn mouse_drag(&mut self, x: u16, y: u16) {
        let Some(d) = &mut self.drag else {
            return;
        };
        if x.abs_diff(d.start.0) + y.abs_diff(d.start.1) > DRAG_THRESHOLD {
            d.moved = true;
        }
        if !d.moved {
            return;
        }
        let nx = (x as i32 - d.dx).max(0) as u16;
        let ny = (y as i32 - d.dy).max(0) as u16;
        self.pos = Some((nx, ny));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Terminal, TestBackend};
    use crate::shell::card::{CardButton, CardFooter, CardTab, FooterRule};
    use crate::shell::input::{Event, Key, Mouse};

    fn painted(host: &mut OverlayHost) -> (u16, u16, u16, u16) {
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| {
            let _ = host.paint(f, f.area());
        })
        .unwrap();
        let r = host.frame.expect("frame after paint");
        (r.x, r.y, r.width, r.height)
    }

    #[test]
    fn click_x_closes() {
        let mut host = OverlayHost::new(Card::single("demo", 40, 12));
        let (x, y, w, _) = painted(&mut host);
        let cmd = host.handle(
            Event::Mouse(Mouse {
                kind: MouseKind::Down,
                x: x + w - 3,
                y,
            }),
            &Keymap::new(),
        );
        assert_eq!(cmd, Some(Command::Close));
        assert!(!host.open);
    }

    #[test]
    fn esc_only_if_mapped() {
        let mut host = OverlayHost::new(Card::single("demo", 40, 12));
        let _ = painted(&mut host);
        assert_eq!(
            host.handle(Event::Key(Key::Esc), &Keymap::new()),
            None
        );
        assert!(host.open);
        assert_eq!(
            host.handle(Event::Key(Key::Esc), &Keymap::new().close_on_esc()),
            Some(Command::Close)
        );
    }

    #[test]
    fn footer_button_and_side_tab() {
        let card = Card::single("demo", 50, 14)
            .side(vec![CardTab::new(1, "one"), CardTab::new(2, "two")], 1)
            .footer(CardFooter::Buttons {
                buttons: vec![CardButton::new(7, "ok")],
                rule: FooterRule::Line,
            });
        let mut host = OverlayHost::new(card);
        let _ = painted(&mut host);
        let side = host
            .hits
            .regions()
            .iter()
            .find(|r| r.hit == Hit::SideTab(2))
            .expect("side tab");
        assert_eq!(
            host.handle(
                Event::Mouse(Mouse {
                    kind: MouseKind::Down,
                    x: side.rect.x,
                    y: side.rect.y,
                }),
                &Keymap::new()
            ),
            Some(Command::SideTab(2))
        );
        let btn = host
            .hits
            .regions()
            .iter()
            .find(|r| r.hit == Hit::FooterButton(7))
            .expect("button");
        assert_eq!(
            host.handle(
                Event::Mouse(Mouse {
                    kind: MouseKind::Down,
                    x: btn.rect.x,
                    y: btn.rect.y,
                }),
                &Keymap::new()
            ),
            Some(Command::Button(7))
        );
    }

    #[test]
    fn drag_header_moves() {
        let mut host = OverlayHost::new(Card::single("demo", 40, 12));
        let (x, y, _, _) = painted(&mut host);
        let _ = host.handle(
            Event::Mouse(Mouse {
                kind: MouseKind::Down,
                x: x + 2,
                y,
            }),
            &Keymap::new(),
        );
        let _ = host.handle(
            Event::Mouse(Mouse {
                kind: MouseKind::Drag,
                x: x + 10,
                y: y + 4,
            }),
            &Keymap::new(),
        );
        let pos = host.pos.expect("dragged");
        assert!(pos.0 > x || pos.1 > y, "pos={pos:?} start=({x},{y})");
    }

    #[test]
    fn no_footer_has_no_button_hits() {
        let mut host = OverlayHost::new(Card::single("plain", 36, 10));
        let _ = painted(&mut host);
        assert!(host
            .hits
            .regions()
            .iter()
            .all(|r| !matches!(r.hit, Hit::FooterButton(_))));
    }
}
