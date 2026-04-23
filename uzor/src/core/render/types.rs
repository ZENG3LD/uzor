/// Text alignment for rendering
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Text baseline for rendering
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextBaseline {
    Top,
    #[default]
    Middle,
    Bottom,
    Alphabetic,
}
