//! Keyboard shortcut system for uzor
//!
//! Provides a flexible keyboard shortcut registry with support for
//! modifier keys, platform-specific formatting, and consumption tracking.

use super::events::KeyCode;
use super::state::ModifierKeys;
use std::collections::{HashMap, HashSet};

// =============================================================================
// KeyboardShortcut
// =============================================================================

/// Keyboard shortcut (ModifierKeys + Key combination)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    /// Modifier keys that must be held
    pub modifiers: ModifierKeys,
    /// The key that must be pressed
    pub key: KeyCode,
}

impl KeyboardShortcut {
    /// Create a new keyboard shortcut
    pub fn new(modifiers: ModifierKeys, key: KeyCode) -> Self {
        Self { modifiers, key }
    }

    /// Create a shortcut with no modifiers
    pub fn key(key: KeyCode) -> Self {
        Self::new(ModifierKeys::default(), key)
    }

    /// Create a Ctrl/Cmd shortcut (platform-aware: Cmd on macOS, Ctrl elsewhere)
    pub fn command(key: KeyCode) -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::new(
                ModifierKeys {
                    meta: true,
                    ..Default::default()
                },
                key,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::new(
                ModifierKeys {
                    ctrl: true,
                    ..Default::default()
                },
                key,
            )
        }
    }

    /// Create a Shift shortcut
    pub fn shift(key: KeyCode) -> Self {
        Self::new(
            ModifierKeys {
                shift: true,
                ..Default::default()
            },
            key,
        )
    }

    /// Create a Ctrl+Shift shortcut
    pub fn ctrl_shift(key: KeyCode) -> Self {
        Self::new(
            ModifierKeys {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            key,
        )
    }

    /// Create a Ctrl+Alt shortcut
    pub fn ctrl_alt(key: KeyCode) -> Self {
        Self::new(
            ModifierKeys {
                ctrl: true,
                alt: true,
                ..Default::default()
            },
            key,
        )
    }

    /// Check if the given modifiers and key exactly match this shortcut
    pub fn matches(&self, modifiers: &ModifierKeys, key: &KeyCode) -> bool {
        self.key == *key && self.modifiers == *modifiers
    }

    /// Check if the given modifiers and key logically match (required modifiers present, extras ignored)
    pub fn matches_logically(&self, modifiers: &ModifierKeys, key: &KeyCode) -> bool {
        if self.key != *key {
            return false;
        }

        if self.modifiers.shift && !modifiers.shift {
            return false;
        }
        if self.modifiers.ctrl && !modifiers.ctrl {
            return false;
        }
        if self.modifiers.alt && !modifiers.alt {
            return false;
        }
        if self.modifiers.meta && !modifiers.meta {
            return false;
        }

        true
    }

    /// Format the shortcut as a human-readable string (platform-aware)
    pub fn format(&self) -> String {
        let mut parts = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if self.modifiers.ctrl {
                parts.push("⌃".to_string());
            }
            if self.modifiers.alt {
                parts.push("⌥".to_string());
            }
            if self.modifiers.shift {
                parts.push("⇧".to_string());
            }
            if self.modifiers.meta {
                parts.push("⌘".to_string());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            if self.modifiers.ctrl {
                parts.push("Ctrl".to_string());
            }
            if self.modifiers.alt {
                parts.push("Alt".to_string());
            }
            if self.modifiers.shift {
                parts.push("Shift".to_string());
            }
            if self.modifiers.meta {
                parts.push("Win".to_string());
            }
        }

        parts.push(format_key(&self.key));

        #[cfg(target_os = "macos")]
        {
            parts.join("")
        }

        #[cfg(not(target_os = "macos"))]
        {
            parts.join("+")
        }
    }
}

// =============================================================================
// ShortcutRegistry
// =============================================================================

/// Registry for managing keyboard shortcuts with consumption tracking
pub struct ShortcutRegistry {
    shortcuts: HashMap<String, KeyboardShortcut>,
    consumed: HashSet<String>,
}

impl ShortcutRegistry {
    /// Create a new shortcut registry
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            consumed: HashSet::new(),
        }
    }

    /// Register a shortcut with a name (replaces existing)
    pub fn register(&mut self, name: &str, shortcut: KeyboardShortcut) {
        self.shortcuts.insert(name.to_string(), shortcut);
    }

    /// Unregister a shortcut by name
    pub fn unregister(&mut self, name: &str) {
        self.shortcuts.remove(name);
    }

    /// Get a shortcut by name
    pub fn get(&self, name: &str) -> Option<&KeyboardShortcut> {
        self.shortcuts.get(name)
    }

    /// Check if a named shortcut is pressed (does not consume)
    pub fn is_pressed(&self, name: &str, modifiers: &ModifierKeys, key: &KeyCode) -> bool {
        self.shortcuts
            .get(name)
            .map(|s| s.matches(modifiers, key))
            .unwrap_or(false)
    }

    /// Check if a named shortcut is pressed and consume it (prevents duplicate handling)
    pub fn consume(&mut self, name: &str, modifiers: &ModifierKeys, key: &KeyCode) -> bool {
        if self.consumed.contains(name) {
            return false;
        }

        let matches = self
            .shortcuts
            .get(name)
            .map(|s| s.matches(modifiers, key))
            .unwrap_or(false);

        if matches {
            self.consumed.insert(name.to_string());
            true
        } else {
            false
        }
    }

    /// Clear all consumed shortcuts (call at end of frame)
    pub fn clear_consumed(&mut self) {
        self.consumed.clear();
    }

    /// Check if a shortcut has been consumed this frame
    pub fn is_consumed(&self, name: &str) -> bool {
        self.consumed.contains(name)
    }

    /// Format a shortcut by name
    pub fn format(&self, name: &str) -> Option<String> {
        self.shortcuts.get(name).map(|s| s.format())
    }

    /// Get all registered shortcut names
    pub fn names(&self) -> Vec<String> {
        self.shortcuts.keys().cloned().collect()
    }

    /// Clear all shortcuts
    pub fn clear(&mut self) {
        self.shortcuts.clear();
        self.consumed.clear();
    }

    /// Get number of registered shortcuts
    pub fn len(&self) -> usize {
        self.shortcuts.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Format a KeyCode as a string
fn format_key(key: &KeyCode) -> String {
    match key {
        KeyCode::A => "A".to_string(),
        KeyCode::B => "B".to_string(),
        KeyCode::C => "C".to_string(),
        KeyCode::D => "D".to_string(),
        KeyCode::E => "E".to_string(),
        KeyCode::F => "F".to_string(),
        KeyCode::G => "G".to_string(),
        KeyCode::H => "H".to_string(),
        KeyCode::I => "I".to_string(),
        KeyCode::J => "J".to_string(),
        KeyCode::K => "K".to_string(),
        KeyCode::L => "L".to_string(),
        KeyCode::M => "M".to_string(),
        KeyCode::N => "N".to_string(),
        KeyCode::O => "O".to_string(),
        KeyCode::P => "P".to_string(),
        KeyCode::Q => "Q".to_string(),
        KeyCode::R => "R".to_string(),
        KeyCode::S => "S".to_string(),
        KeyCode::T => "T".to_string(),
        KeyCode::U => "U".to_string(),
        KeyCode::V => "V".to_string(),
        KeyCode::W => "W".to_string(),
        KeyCode::X => "X".to_string(),
        KeyCode::Y => "Y".to_string(),
        KeyCode::Z => "Z".to_string(),

        KeyCode::Num0 => "0".to_string(),
        KeyCode::Num1 => "1".to_string(),
        KeyCode::Num2 => "2".to_string(),
        KeyCode::Num3 => "3".to_string(),
        KeyCode::Num4 => "4".to_string(),
        KeyCode::Num5 => "5".to_string(),
        KeyCode::Num6 => "6".to_string(),
        KeyCode::Num7 => "7".to_string(),
        KeyCode::Num8 => "8".to_string(),
        KeyCode::Num9 => "9".to_string(),

        KeyCode::F1 => "F1".to_string(),
        KeyCode::F2 => "F2".to_string(),
        KeyCode::F3 => "F3".to_string(),
        KeyCode::F4 => "F4".to_string(),
        KeyCode::F5 => "F5".to_string(),
        KeyCode::F6 => "F6".to_string(),
        KeyCode::F7 => "F7".to_string(),
        KeyCode::F8 => "F8".to_string(),
        KeyCode::F9 => "F9".to_string(),
        KeyCode::F10 => "F10".to_string(),
        KeyCode::F11 => "F11".to_string(),
        KeyCode::F12 => "F12".to_string(),

        KeyCode::ArrowUp => "↑".to_string(),
        KeyCode::ArrowDown => "↓".to_string(),
        KeyCode::ArrowLeft => "←".to_string(),
        KeyCode::ArrowRight => "→".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),

        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Insert => "Ins".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Space => "Space".to_string(),
        KeyCode::Escape => "Esc".to_string(),

        KeyCode::Plus => "+".to_string(),
        KeyCode::Minus => "-".to_string(),
        KeyCode::BracketLeft => "[".to_string(),
        KeyCode::BracketRight => "]".to_string(),

        KeyCode::Unknown => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_creation() {
        let shortcut = KeyboardShortcut::new(ModifierKeys::default(), KeyCode::S);
        assert_eq!(shortcut.key, KeyCode::S);
        assert!(!shortcut.modifiers.ctrl);
    }

    #[test]
    fn test_shortcut_command() {
        let shortcut = KeyboardShortcut::command(KeyCode::S);

        #[cfg(target_os = "macos")]
        {
            assert!(shortcut.modifiers.meta);
            assert!(!shortcut.modifiers.ctrl);
        }

        #[cfg(not(target_os = "macos"))]
        {
            assert!(shortcut.modifiers.ctrl);
            assert!(!shortcut.modifiers.meta);
        }
    }

    #[test]
    fn test_shortcut_matches() {
        let shortcut = KeyboardShortcut::new(
            ModifierKeys {
                ctrl: true,
                ..Default::default()
            },
            KeyCode::S,
        );

        let modifiers = ModifierKeys {
            ctrl: true,
            ..Default::default()
        };

        assert!(shortcut.matches(&modifiers, &KeyCode::S));
        assert!(!shortcut.matches(&modifiers, &KeyCode::A));

        let wrong_modifiers = ModifierKeys {
            shift: true,
            ..Default::default()
        };
        assert!(!shortcut.matches(&wrong_modifiers, &KeyCode::S));
    }

    #[test]
    fn test_shortcut_matches_logically() {
        let shortcut = KeyboardShortcut::new(
            ModifierKeys {
                ctrl: true,
                ..Default::default()
            },
            KeyCode::S,
        );

        let modifiers = ModifierKeys {
            ctrl: true,
            ..Default::default()
        };
        assert!(shortcut.matches_logically(&modifiers, &KeyCode::S));

        let extra_modifiers = ModifierKeys {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        assert!(shortcut.matches_logically(&extra_modifiers, &KeyCode::S));

        let missing = ModifierKeys {
            shift: true,
            ..Default::default()
        };
        assert!(!shortcut.matches_logically(&missing, &KeyCode::S));

        assert!(!shortcut.matches_logically(&modifiers, &KeyCode::A));
    }

    #[test]
    fn test_shortcut_format() {
        let shortcut = KeyboardShortcut::new(
            ModifierKeys {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            KeyCode::S,
        );

        let formatted = shortcut.format();

        #[cfg(target_os = "macos")]
        {
            assert_eq!(formatted, "⌃⇧S");
        }

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(formatted, "Ctrl+Shift+S");
        }
    }

    #[test]
    fn test_registry_basic() {
        let mut registry = ShortcutRegistry::new();

        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        registry.register("save", KeyboardShortcut::command(KeyCode::S));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let shortcut = registry.get("save").unwrap();
        assert_eq!(shortcut.key, KeyCode::S);

        registry.unregister("save");
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_is_pressed() {
        let mut registry = ShortcutRegistry::new();
        registry.register(
            "save",
            KeyboardShortcut::new(
                ModifierKeys {
                    ctrl: true,
                    ..Default::default()
                },
                KeyCode::S,
            ),
        );

        let modifiers = ModifierKeys {
            ctrl: true,
            ..Default::default()
        };

        assert!(registry.is_pressed("save", &modifiers, &KeyCode::S));
        assert!(!registry.is_pressed("save", &modifiers, &KeyCode::A));
        assert!(!registry.is_pressed("nonexistent", &modifiers, &KeyCode::S));
    }

    #[test]
    fn test_registry_consume() {
        let mut registry = ShortcutRegistry::new();
        registry.register(
            "save",
            KeyboardShortcut::new(
                ModifierKeys {
                    ctrl: true,
                    ..Default::default()
                },
                KeyCode::S,
            ),
        );

        let modifiers = ModifierKeys {
            ctrl: true,
            ..Default::default()
        };

        assert!(registry.consume("save", &modifiers, &KeyCode::S));
        assert!(registry.is_consumed("save"));

        assert!(!registry.consume("save", &modifiers, &KeyCode::S));

        registry.clear_consumed();
        assert!(!registry.is_consumed("save"));
        assert!(registry.consume("save", &modifiers, &KeyCode::S));
    }

    #[test]
    fn test_registry_format() {
        let mut registry = ShortcutRegistry::new();
        registry.register("save", KeyboardShortcut::command(KeyCode::S));

        let formatted = registry.format("save").unwrap();

        #[cfg(target_os = "macos")]
        {
            assert_eq!(formatted, "⌘S");
        }

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(formatted, "Ctrl+S");
        }

        assert_eq!(registry.format("nonexistent"), None);
    }

    #[test]
    fn test_format_key() {
        assert_eq!(format_key(&KeyCode::A), "A");
        assert_eq!(format_key(&KeyCode::Num5), "5");
        assert_eq!(format_key(&KeyCode::F1), "F1");
        assert_eq!(format_key(&KeyCode::ArrowUp), "↑");
        assert_eq!(format_key(&KeyCode::Enter), "Enter");
        assert_eq!(format_key(&KeyCode::Space), "Space");
    }

    #[test]
    fn test_shortcut_builders() {
        let shift = KeyboardShortcut::shift(KeyCode::A);
        assert!(shift.modifiers.shift);
        assert!(!shift.modifiers.ctrl);

        let ctrl_shift = KeyboardShortcut::ctrl_shift(KeyCode::B);
        assert!(ctrl_shift.modifiers.ctrl);
        assert!(ctrl_shift.modifiers.shift);

        let ctrl_alt = KeyboardShortcut::ctrl_alt(KeyCode::C);
        assert!(ctrl_alt.modifiers.ctrl);
        assert!(ctrl_alt.modifiers.alt);
    }

    #[test]
    fn test_registry_names() {
        let mut registry = ShortcutRegistry::new();
        registry.register("save", KeyboardShortcut::command(KeyCode::S));
        registry.register("copy", KeyboardShortcut::command(KeyCode::C));

        let names = registry.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"save".to_string()));
        assert!(names.contains(&"copy".to_string()));
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = ShortcutRegistry::new();
        registry.register("save", KeyboardShortcut::command(KeyCode::S));
        registry.register("copy", KeyboardShortcut::command(KeyCode::C));

        assert_eq!(registry.len(), 2);

        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }
}
