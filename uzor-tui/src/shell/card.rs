//! Overlay card slots. No app commands — buttons are numeric ids.

/// Visual token. Other skins restyle the same slots later.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardSkin {
    #[default]
    Default,
}

/// Chip on the top header or the side rail.
#[derive(Clone, Debug)]
pub struct CardTab {
    pub id: u8,
    pub label: String,
}

impl CardTab {
    pub fn new(id: u8, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

/// Hairline above the footer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FooterRule {
    #[default]
    None,
    Line,
}

/// Footer button. `id` is the app's handle.
#[derive(Clone, Debug)]
pub struct CardButton {
    pub id: u8,
    pub label: String,
    pub pending: bool,
}

impl CardButton {
    pub fn new(id: u8, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            pending: false,
        }
    }

    pub fn pending(mut self, on: bool) -> Self {
        self.pending = on;
        self
    }
}

/// Footer slot. Default is absent.
#[derive(Clone, Debug, Default)]
pub enum CardFooter {
    #[default]
    None,
    Info {
        lines: Vec<String>,
        rule: FooterRule,
    },
    Buttons {
        buttons: Vec<CardButton>,
        rule: FooterRule,
    },
}

/// Layout slots the host fills.
#[derive(Clone, Debug)]
pub struct Card {
    pub skin: CardSkin,
    pub top_tabs: Vec<CardTab>,
    pub top_active: u8,
    pub side_tabs: Vec<CardTab>,
    pub side_active: u8,
    pub footer: CardFooter,
    pub prefer_w: u16,
    pub prefer_h: u16,
    pub min_w: u16,
    pub min_h: u16,
    /// Click outside the frame emits Close.
    pub dismiss_outside: bool,
}

impl Card {
    pub fn new(prefer_w: u16, prefer_h: u16) -> Self {
        Self {
            skin: CardSkin::Default,
            top_tabs: Vec::new(),
            top_active: 0,
            side_tabs: Vec::new(),
            side_active: 0,
            footer: CardFooter::None,
            prefer_w,
            prefer_h,
            min_w: 28,
            min_h: 8,
            dismiss_outside: true,
        }
    }

    pub fn single(title: impl Into<String>, prefer_w: u16, prefer_h: u16) -> Self {
        Self::new(prefer_w, prefer_h).top(vec![CardTab::new(0, title)], 0)
    }

    pub fn top(mut self, tabs: Vec<CardTab>, active: u8) -> Self {
        self.top_tabs = tabs;
        self.top_active = active;
        self
    }

    pub fn side(mut self, tabs: Vec<CardTab>, active: u8) -> Self {
        self.side_tabs = tabs;
        self.side_active = active;
        self
    }

    pub fn footer(mut self, footer: CardFooter) -> Self {
        self.footer = footer;
        self
    }

    pub fn dismiss_outside(mut self, on: bool) -> Self {
        self.dismiss_outside = on;
        self
    }

    pub fn footer_h(&self) -> u16 {
        match &self.footer {
            CardFooter::None => 0,
            CardFooter::Info { lines, rule } => {
                let n = lines.len().max(1) as u16;
                n + u16::from(*rule == FooterRule::Line)
            }
            CardFooter::Buttons { rule, .. } => 1 + u16::from(*rule == FooterRule::Line),
        }
    }

    pub fn side_w(&self) -> u16 {
        if self.side_tabs.is_empty() {
            return 0;
        }
        let w = self
            .side_tabs
            .iter()
            .map(|t| t.label.chars().count())
            .max()
            .unwrap_or(4)
            + 2;
        (w as u16).clamp(8, 18)
    }
}
