//! Keyboard and mouse as data. Apps bind keys; the host does not hard-code Esc.

/// Keys the host can bind. Not a full crossterm dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Esc,
    Enter,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Ctrl(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    Down,
    Up,
    Drag,
    Scroll { up: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouse {
    pub kind: MouseKind,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    Mouse(Mouse),
}

/// What the host reports to the app after handling an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Close,
    TopTab(u8),
    SideTab(u8),
    Button(u8),
    Custom(u16),
}

/// Key → command table. Empty by default — no hidden Esc.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: Vec<(Key, Command)>,
}

impl Keymap {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn bind(mut self, key: Key, cmd: Command) -> Self {
        self.bindings.push((key, cmd));
        self
    }

    /// Opt-in Esc → Close. Not installed unless the app asks.
    pub fn close_on_esc(self) -> Self {
        self.bind(Key::Esc, Command::Close)
    }

    pub fn lookup(&self, key: Key) -> Option<Command> {
        self.bindings
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, c)| *c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_has_no_esc() {
        assert_eq!(Keymap::new().lookup(Key::Esc), None);
        assert_eq!(
            Keymap::new().close_on_esc().lookup(Key::Esc),
            Some(Command::Close)
        );
    }
}
