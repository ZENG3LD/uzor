//! Toast stack lifecycle state.
//!
//! `ToastStackState` owns the live list of toasts and drives expiry.
//! Mirrors mlc's `active_toasts` Vec + drain/retain cycle, with a small API layer.

use super::types::ToastType;

/// One live toast entry: the notification data plus the timestamp when it was pushed.
/// The `created_at_ms` field stamps `ToastType::timestamp_ms` at push time so the
/// caller does not have to fill it in advance.
#[derive(Debug, Clone)]
pub struct ToastEntry {
    pub toast: ToastType,
}

/// Runtime state for the whole toast stack.
///
/// # mlc correspondence
/// - `active: Vec<ToastEntry>` ← `active_toasts: Vec<ToastNotification>`
/// - `push`                    ← `active_toasts.push(toast)` in drain loop
/// - `tick`                    ← `active_toasts.retain(|t| !t.is_expired(now_ms))`
/// - `clear`                   ← manual clear (no mlc equivalent; useful for tests)
/// - `is_overflow`             ← the `if y + toast_height > window_height { break }` guard
#[derive(Debug, Default, Clone)]
pub struct ToastStackState {
    active: Vec<ToastEntry>,
}

impl ToastStackState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new toast, stamping `timestamp_ms = now_ms`.
    /// Mirrors mlc's `active_toasts.push(toast)`.
    pub fn push(&mut self, mut toast: ToastType, now_ms: u64) {
        toast.timestamp_ms = now_ms;
        self.active.push(ToastEntry { toast });
    }

    /// Remove all expired toasts and return a slice of survivors.
    /// Mirrors mlc's `active_toasts.retain(|t| !t.is_expired(now_ms_val))`.
    pub fn tick(&mut self, now_ms: u64) -> &[ToastEntry] {
        self.active.retain(|e| !e.toast.is_expired(now_ms));
        &self.active
    }

    /// Immediate read of the active list without expiry check.
    pub fn entries(&self) -> &[ToastEntry] {
        &self.active
    }

    /// Remove all toasts unconditionally.
    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// Returns `true` when the stack would overflow `window_height`.
    ///
    /// Uses the same formula as mlc's `break` guard:
    /// `start_y + n * (toast_height + margin) + toast_height > window_height`
    /// where `start_y = TOP_ANCHOR` and geometry comes from `ToastGeometry`.
    pub fn is_overflow(&self, window_height: f64) -> bool {
        use super::style::ToastGeometry as G;
        let n = self.active.len() as f64;
        G::TOP_ANCHOR + n * G::STACK_PITCH > window_height
    }

    /// How many toasts are currently live.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}
