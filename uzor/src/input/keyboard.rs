//! Unified key press enum for text and UI interaction.

/// Unified key press event for text and UI interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyPress {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Backspace,
    Enter,
    Escape,
    Tab,
    ShiftLeft,
    ShiftRight,
    ShiftHome,
    ShiftEnd,
    SelectAll,
    Copy,
    Paste(String),
    Undo,
    Redo,
    CtrlC,
}
