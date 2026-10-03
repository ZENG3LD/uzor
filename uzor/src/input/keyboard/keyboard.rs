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
    /// Move the cursor to the start of the previous word (Ctrl+Left).
    WordLeft,
    /// Move the cursor past the end of the next word (Ctrl+Right).
    WordRight,
    /// Delete the selection, else back to the start of the previous word
    /// (Ctrl+Backspace).
    DeleteWordBack,
    /// Delete the selection, else forward past the next word (Ctrl+Delete).
    DeleteWordForward,
}
