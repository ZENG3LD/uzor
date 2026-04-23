//! Keyboard input events (key codes)
//!
//! This module defines platform-agnostic key codes used across uzor
//! for keyboard shortcuts and key event handling.

/// Keyboard key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum KeyCode {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // Numbers
    Num0, Num1, Num2, Num3, Num4,
    Num5, Num6, Num7, Num8, Num9,

    // Function keys
    F1, F2, F3, F4, F5, F6,
    F7, F8, F9, F10, F11, F12,

    // Navigation
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,

    // Editing
    Backspace,
    Delete,
    Insert,
    Enter,
    Tab,
    Space,
    Escape,

    // Symbols
    Plus,
    Minus,
    BracketLeft,
    BracketRight,

    /// Unknown or unmapped key
    #[default]
    Unknown,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_code_default() {
        assert_eq!(KeyCode::default(), KeyCode::Unknown);
    }

    #[test]
    fn test_key_code_categories() {
        let _letter = KeyCode::A;
        let _number = KeyCode::Num1;
        let _func = KeyCode::F1;
        let _nav = KeyCode::ArrowUp;
        let _edit = KeyCode::Backspace;
        let _symbol = KeyCode::Plus;
    }
}
