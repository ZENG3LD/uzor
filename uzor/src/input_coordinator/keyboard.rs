//! Unified key press event enum for text and UI interaction.

/// Unified key press event for text and UI interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyPress {
    // Navigation
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,

    // Editing
    Delete,
    Backspace,
    Enter,
    Escape,
    Tab,

    // Selection (Shift+movement)
    ShiftLeft,
    ShiftRight,
    ShiftHome,
    ShiftEnd,

    // Commands
    /// Ctrl+A
    SelectAll,
    /// Ctrl+C
    Copy,
    /// Ctrl+V with clipboard content
    Paste(String),
    /// Ctrl+Z
    Undo,
    /// Ctrl+Shift+Z / Ctrl+Y
    Redo,

    // Terminal
    /// Raw Ctrl+C for PTY
    CtrlC,
}
