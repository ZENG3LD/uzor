//! Toast lifecycle state — when it was shown, current alpha.

#[derive(Debug, Default, Clone, Copy)]
pub struct ToastState {
    /// `now_ms` when the toast became visible.
    pub created_at_ms: u64,
    /// 1.0 = fully visible, 0.0 = invisible.
    pub alpha: f64,
}
