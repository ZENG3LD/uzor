//! Text field store — manages text/cursor/selection for all registered text fields.

use std::collections::HashMap;
use crate::types::WidgetId;
use super::keyboard::KeyPress;

/// Which interaction classes a text field supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCapability {
    /// Keyboard + mouse
    Both,
    /// Typing only, no mouse positioning
    Keyboard,
    /// Click/drag select only, no typing
    Mouse,
    /// PTY pass-through
    Raw,
}

/// Static configuration for a text field.
#[derive(Clone, Debug)]
pub struct TextFieldConfig {
    pub capability: InputCapability,
    pub char_filter: Option<fn(char) -> bool>,
    pub max_len: Option<usize>,
    pub masked: bool,
    pub read_only: bool,
}

impl TextFieldConfig {
    pub fn text() -> Self {
        Self { capability: InputCapability::Both, char_filter: None, max_len: None, masked: false, read_only: false }
    }
    pub fn password() -> Self {
        Self { capability: InputCapability::Both, char_filter: None, max_len: None, masked: true, read_only: false }
    }
    pub fn search() -> Self {
        Self { capability: InputCapability::Both, char_filter: None, max_len: None, masked: false, read_only: false }
    }
    pub fn read_only() -> Self {
        Self { capability: InputCapability::Mouse, char_filter: None, max_len: None, masked: false, read_only: true }
    }
    pub fn keyboard_only() -> Self {
        Self { capability: InputCapability::Keyboard, char_filter: None, max_len: None, masked: false, read_only: false }
    }
    pub fn raw() -> Self {
        Self { capability: InputCapability::Raw, char_filter: None, max_len: None, masked: false, read_only: false }
    }
    pub fn with_filter(mut self, f: fn(char) -> bool) -> Self { self.char_filter = Some(f); self }
    pub fn with_max_len(mut self, n: usize) -> Self { self.max_len = Some(n); self }
}

/// Action returned by text input processing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAction {
    None,
    Commit(String),
    Cancel,
    TextChanged(String),
    RawInput(Vec<u8>),
}

/// Runtime state for one text field.
#[derive(Clone, Debug)]
pub struct TextFieldState {
    pub text: String,
    pub original_text: String,
    pub cursor: usize,
    pub selection_start: Option<usize>,
    pub last_rect: Option<(f64, f64, f64, f64)>,
    pub last_char_positions: Vec<f64>,
    pub last_frame: u64,
    pub config: TextFieldConfig,
}

impl TextFieldState {
    pub fn new(config: TextFieldConfig) -> Self {
        Self {
            text: String::new(),
            original_text: String::new(),
            cursor: 0,
            selection_start: None,
            last_rect: None,
            last_char_positions: Vec::new(),
            last_frame: 0,
            config,
        }
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_start?;
        if anchor == self.cursor { return None; }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    pub fn delete_selection(&mut self) {
        if let Some((lo, hi)) = self.selection_range() {
            let byte_lo = self.char_to_byte(lo);
            let byte_hi = self.char_to_byte(hi);
            self.text.drain(byte_lo..byte_hi);
            self.cursor = lo;
            self.selection_start = None;
        }
    }

    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(self.text.len())
    }

    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }
}

/// Central text field store. Manages text/cursor/selection for all registered text fields.
pub struct TextFieldStore {
    fields: HashMap<WidgetId, TextFieldState>,
    focused: Option<WidgetId>,
    drag_field: Option<WidgetId>,
    current_frame: u64,
    blink_reset_time: u64,
}

impl TextFieldStore {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            focused: None,
            drag_field: None,
            current_frame: 0,
            blink_reset_time: 0,
        }
    }

    pub fn register(&mut self, id: WidgetId, config: TextFieldConfig) {
        self.fields.entry(id).or_insert_with(|| TextFieldState::new(config));
    }

    pub fn begin_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    pub fn update_field(&mut self, id: &WidgetId, rect: (f64, f64, f64, f64), char_positions: Vec<f64>) {
        if let Some(state) = self.fields.get_mut(id) {
            state.last_rect = Some(rect);
            state.last_char_positions = char_positions;
            state.last_frame = self.current_frame;
        }
    }

    // --- Query methods ---

    pub fn text(&self, id: &WidgetId) -> &str {
        self.fields.get(id).map(|s| s.text.as_str()).unwrap_or("")
    }

    pub fn cursor(&self, id: &WidgetId) -> usize {
        self.fields.get(id).map(|s| s.cursor).unwrap_or(0)
    }

    pub fn selection_range(&self, id: &WidgetId) -> Option<(usize, usize)> {
        self.fields.get(id)?.selection_range()
    }

    pub fn is_focused(&self, id: &WidgetId) -> bool {
        self.focused.as_ref() == Some(id)
    }

    pub fn focused(&self) -> Option<&WidgetId> {
        self.focused.as_ref()
    }

    pub fn cursor_visible(&self, now_ms: u64) -> bool {
        let elapsed = now_ms.wrapping_sub(self.blink_reset_time);
        (elapsed / 500) % 2 == 0
    }

    pub fn field_state(&self, id: &WidgetId) -> Option<&TextFieldState> {
        self.fields.get(id)
    }

    // --- Focus management ---

    pub fn focus(&mut self, id: &WidgetId) -> bool {
        if self.focused.as_ref() == Some(id) { return false; }
        if let Some(prev) = &self.focused {
            if let Some(state) = self.fields.get_mut(prev) {
                state.selection_start = None;
            }
        }
        self.focused = Some(id.clone());
        self.blink_reset_time = 0;
        true
    }

    pub fn blur(&mut self) {
        if let Some(id) = &self.focused {
            if let Some(state) = self.fields.get_mut(id) {
                state.selection_start = None;
            }
        }
        self.focused = None;
        self.drag_field = None;
    }

    pub fn set_text(&mut self, id: &WidgetId, text: &str) {
        if let Some(state) = self.fields.get_mut(id) {
            state.text = text.to_string();
            state.cursor = state.char_count();
            state.selection_start = None;
        }
    }

    pub fn clear(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.text.clear();
            state.cursor = 0;
            state.selection_start = None;
        }
    }

    pub fn begin_edit(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.original_text = state.text.clone();
        }
    }

    pub fn set_blink_time(&mut self, now_ms: u64) {
        self.blink_reset_time = now_ms;
    }

    // --- Input dispatch ---

    pub fn on_char(&mut self, ch: char) -> TextAction {
        let id = match &self.focused {
            Some(id) => id.clone(),
            None => return TextAction::None,
        };

        let capability = match self.fields.get(&id) {
            Some(s) => s.config.capability,
            None => return TextAction::None,
        };

        if capability == InputCapability::Raw {
            return TextAction::RawInput(raw_char_to_bytes(ch));
        }

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return TextAction::None,
        };

        if state.config.capability == InputCapability::Mouse || state.config.read_only {
            return TextAction::None;
        }

        match ch {
            '\r' | '\n' => TextAction::Commit(state.text.clone()),
            '\x1b' => {
                let original = state.original_text.clone();
                state.text = original;
                state.cursor = state.char_count();
                state.selection_start = None;
                TextAction::Cancel
            }
            '\x08' => {
                if state.selection_range().is_some() {
                    state.delete_selection();
                } else if state.cursor > 0 {
                    let byte_pos = state.char_to_byte(state.cursor - 1);
                    let byte_end = state.char_to_byte(state.cursor);
                    state.text.drain(byte_pos..byte_end);
                    state.cursor -= 1;
                }
                TextAction::TextChanged(state.text.clone())
            }
            c if c.is_control() => TextAction::None,
            c => {
                if let Some(filter) = state.config.char_filter {
                    if !filter(c) { return TextAction::None; }
                }
                let text_len = if state.selection_range().is_some() {
                    let (lo, hi) = state.selection_range().unwrap();
                    state.char_count() - (hi - lo)
                } else {
                    state.char_count()
                };
                if let Some(max) = state.config.max_len {
                    if text_len >= max { return TextAction::None; }
                }
                if state.selection_range().is_some() {
                    state.delete_selection();
                }
                let byte_pos = state.char_to_byte(state.cursor);
                state.text.insert(byte_pos, c);
                state.cursor += 1;
                TextAction::TextChanged(state.text.clone())
            }
        }
    }

    pub fn on_key(&mut self, key: KeyPress) -> TextAction {
        let id = match &self.focused {
            Some(id) => id.clone(),
            None => return TextAction::None,
        };

        let capability = match self.fields.get(&id) {
            Some(s) => s.config.capability,
            None => return TextAction::None,
        };

        if capability == InputCapability::Raw {
            if let Some(bytes) = key_to_pty_bytes(&key) {
                return TextAction::RawInput(bytes);
            }
            return TextAction::None;
        }

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return TextAction::None,
        };

        let restricted = state.config.capability == InputCapability::Mouse || state.config.read_only;
        if restricted {
            match &key {
                KeyPress::Copy | KeyPress::SelectAll => {}
                _ => return TextAction::None,
            }
        }

        Self::apply_key(state, key);
        TextAction::None
    }

    pub fn on_drag_start(&mut self, x: f64, y: f64) -> bool {
        let mut hit_id: Option<WidgetId> = None;
        for (id, state) in &self.fields {
            if state.config.capability == InputCapability::Keyboard { continue; }
            let frame_lag = self.current_frame.wrapping_sub(state.last_frame);
            if frame_lag > 1 { continue; }
            if let Some((rx, ry, rw, rh)) = state.last_rect {
                if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                    hit_id = Some(id.clone());
                    break;
                }
            }
        }

        let id = match hit_id {
            Some(id) => id,
            None => return false,
        };

        self.focus(&id);

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return false,
        };

        let cursor = cursor_from_x(&state.last_char_positions, x);
        state.cursor = cursor;
        state.selection_start = Some(cursor);
        self.drag_field = Some(id);
        true
    }

    pub fn on_drag_move(&mut self, x: f64) {
        let id = match &self.drag_field {
            Some(id) => id.clone(),
            None => return,
        };
        if let Some(state) = self.fields.get_mut(&id) {
            state.cursor = cursor_from_x(&state.last_char_positions, x);
        }
    }

    pub fn on_drag_end(&mut self) {
        if let Some(id) = &self.drag_field {
            if let Some(state) = self.fields.get_mut(id) {
                if state.selection_start == Some(state.cursor) {
                    state.selection_start = None;
                }
            }
        }
        self.drag_field = None;
    }

    pub fn copy_selection(&self) -> Option<String> {
        let id = self.focused.as_ref().or(self.drag_field.as_ref())?;
        let state = self.fields.get(id)?;
        let (lo, hi) = state.selection_range()?;
        let byte_lo = state.char_to_byte(lo);
        let byte_hi = state.char_to_byte(hi);
        Some(state.text[byte_lo..byte_hi].to_string())
    }

    // --- Private helpers ---

    fn apply_key(state: &mut TextFieldState, key: KeyPress) -> bool {
        let char_count = state.char_count();
        match key {
            KeyPress::Delete => {
                if state.selection_range().is_some() {
                    state.delete_selection();
                } else if state.cursor < char_count {
                    let byte_idx = state.char_to_byte(state.cursor);
                    state.text.remove(byte_idx);
                }
                true
            }
            KeyPress::ArrowLeft => {
                if state.selection_range().is_some() {
                    let (lo, _) = state.selection_range().unwrap();
                    state.cursor = lo;
                    state.selection_start = None;
                } else {
                    state.cursor = state.cursor.saturating_sub(1);
                }
                true
            }
            KeyPress::ArrowRight => {
                if state.selection_range().is_some() {
                    let (_, hi) = state.selection_range().unwrap();
                    state.cursor = hi;
                    state.selection_start = None;
                } else if state.cursor < char_count {
                    state.cursor += 1;
                }
                true
            }
            KeyPress::Home => {
                state.cursor = 0;
                state.selection_start = None;
                true
            }
            KeyPress::End => {
                state.cursor = char_count;
                state.selection_start = None;
                true
            }
            KeyPress::SelectAll => {
                state.selection_start = Some(0);
                state.cursor = char_count;
                true
            }
            KeyPress::ShiftLeft => {
                if state.selection_start.is_none() { state.selection_start = Some(state.cursor); }
                state.cursor = state.cursor.saturating_sub(1);
                if state.selection_start == Some(state.cursor) { state.selection_start = None; }
                true
            }
            KeyPress::ShiftRight => {
                if state.selection_start.is_none() { state.selection_start = Some(state.cursor); }
                if state.cursor < char_count { state.cursor += 1; }
                if state.selection_start == Some(state.cursor) { state.selection_start = None; }
                true
            }
            KeyPress::ShiftHome => {
                if state.selection_start.is_none() { state.selection_start = Some(state.cursor); }
                state.cursor = 0;
                if state.selection_start == Some(state.cursor) { state.selection_start = None; }
                true
            }
            KeyPress::ShiftEnd => {
                if state.selection_start.is_none() { state.selection_start = Some(state.cursor); }
                state.cursor = char_count;
                if state.selection_start == Some(state.cursor) { state.selection_start = None; }
                true
            }
            KeyPress::Copy => false,
            KeyPress::Paste(ref text) => {
                if state.config.read_only { return false; }
                if state.selection_range().is_some() { state.delete_selection(); }
                for ch in text.chars() {
                    if ch.is_control() { continue; }
                    if let Some(filter) = state.config.char_filter {
                        if !filter(ch) { continue; }
                    }
                    if let Some(max) = state.config.max_len {
                        if state.char_count() >= max { break; }
                    }
                    let byte_pos = state.char_to_byte(state.cursor);
                    state.text.insert(byte_pos, ch);
                    state.cursor += 1;
                }
                true
            }
            KeyPress::Undo | KeyPress::Redo => false,
            KeyPress::ArrowUp | KeyPress::ArrowDown | KeyPress::Enter
            | KeyPress::Escape | KeyPress::Tab | KeyPress::Backspace
            | KeyPress::CtrlC | KeyPress::PageUp | KeyPress::PageDown => false,
        }
    }
}

impl Default for TextFieldStore {
    fn default() -> Self { Self::new() }
}

// --- PTY byte helpers ---

fn raw_char_to_bytes(ch: char) -> Vec<u8> {
    if ch == '\r' || ch == '\n' { return vec![b'\r']; }
    if ch == '\x08' { return vec![0x7f]; }
    if ch == '\x1b' { return vec![0x1b]; }
    if (ch as u32) < 0x20 { return vec![ch as u8]; }
    let mut buf = [0u8; 4];
    ch.encode_utf8(&mut buf).as_bytes().to_vec()
}

fn key_to_pty_bytes(key: &KeyPress) -> Option<Vec<u8>> {
    match key {
        KeyPress::ArrowLeft => Some(b"\x1b[D".to_vec()),
        KeyPress::ArrowRight => Some(b"\x1b[C".to_vec()),
        KeyPress::ArrowUp => Some(b"\x1b[A".to_vec()),
        KeyPress::ArrowDown => Some(b"\x1b[B".to_vec()),
        KeyPress::Home => Some(b"\x1b[H".to_vec()),
        KeyPress::End => Some(b"\x1b[F".to_vec()),
        KeyPress::Delete => Some(b"\x1b[3~".to_vec()),
        KeyPress::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyPress::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyPress::Enter => Some(b"\r".to_vec()),
        KeyPress::Escape => Some(b"\x1b".to_vec()),
        KeyPress::Tab => Some(b"\t".to_vec()),
        KeyPress::Backspace => Some(b"\x7f".to_vec()),
        KeyPress::CtrlC => Some(b"\x03".to_vec()),
        KeyPress::ShiftLeft => Some(b"\x1b[1;2D".to_vec()),
        KeyPress::ShiftRight => Some(b"\x1b[1;2C".to_vec()),
        _ => None,
    }
}

fn cursor_from_x(positions: &[f64], x: f64) -> usize {
    if positions.is_empty() { return 0; }
    let char_count = positions.len().saturating_sub(1);
    for i in 0..char_count {
        let mid = (positions[i] + positions[i + 1]) * 0.5;
        if x < mid { return i; }
    }
    char_count
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_focus() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("field1");
        store.register(id.clone(), TextFieldConfig::text());
        assert!(!store.is_focused(&id));
        store.focus(&id);
        assert!(store.is_focused(&id));
    }

    #[test]
    fn test_char_input() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("field1");
        store.register(id.clone(), TextFieldConfig::text());
        store.focus(&id);
        store.on_char('h');
        store.on_char('i');
        assert_eq!(store.text(&id), "hi");
        assert_eq!(store.cursor(&id), 2);
    }

    #[test]
    fn test_char_filter() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("hex");
        store.register(id.clone(), TextFieldConfig::text().with_filter(|c| c.is_ascii_hexdigit()).with_max_len(4));
        store.focus(&id);
        store.on_char('a');
        store.on_char('Z'); // rejected
        store.on_char('f');
        assert_eq!(store.text(&id), "af");
    }

    #[test]
    fn test_commit_cancel() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("field1");
        store.register(id.clone(), TextFieldConfig::text());
        store.focus(&id);
        store.set_text(&id, "original");
        store.begin_edit(&id);
        store.on_char('X');
        assert_ne!(store.text(&id), "original");
        let action = store.on_char('\x1b');
        assert_eq!(action, TextAction::Cancel);
        assert_eq!(store.text(&id), "original");
    }

    #[test]
    fn test_blink() {
        let store = TextFieldStore::new();
        assert!(store.cursor_visible(0));
        assert!(!store.cursor_visible(600));
        assert!(store.cursor_visible(1100));
    }

    #[test]
    fn test_selection_and_delete() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("field1");
        store.register(id.clone(), TextFieldConfig::text());
        store.focus(&id);
        store.set_text(&id, "hello");
        store.on_key(KeyPress::SelectAll);
        assert_eq!(store.selection_range(&id), Some((0, 5)));
        store.on_char('x');
        assert_eq!(store.text(&id), "x");
    }

    #[test]
    fn test_raw_mode() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("pty");
        store.register(id.clone(), TextFieldConfig::raw());
        store.focus(&id);
        let action = store.on_char('a');
        match action {
            TextAction::RawInput(bytes) => assert_eq!(bytes, vec![b'a']),
            _ => panic!("expected RawInput"),
        }
    }

    #[test]
    fn test_read_only() {
        let mut store = TextFieldStore::new();
        let id = WidgetId::new("ro");
        store.register(id.clone(), TextFieldConfig::read_only());
        store.focus(&id);
        store.set_text(&id, "frozen");
        let action = store.on_char('X');
        assert_eq!(action, TextAction::None);
        assert_eq!(store.text(&id), "frozen");
        // But SelectAll works
        store.on_key(KeyPress::SelectAll);
        assert_eq!(store.selection_range(&id), Some((0, 6)));
    }
}
