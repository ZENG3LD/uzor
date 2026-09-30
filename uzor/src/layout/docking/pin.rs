//! Whether a docked panel may be torn off by dragging its header or tab.

/// Tear-off lock of a docked panel, returned by
/// [`DockPanel::pin`](super::DockPanel::pin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Pin {
    /// Can be dragged out (default).
    #[default]
    Free,
    /// The user decides: `pinned: true` locks it in place.
    User { pinned: bool },
    /// Fixed by the app; never tears off.
    System,
}

impl Pin {
    /// True when a header or tab drag must not start for this panel.
    pub fn locks_tear_off(&self) -> bool {
        match self {
            Pin::Free => false,
            Pin::User { pinned } => *pinned,
            Pin::System => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_tear_off_truth_table() {
        assert!(!Pin::Free.locks_tear_off());
        assert!(Pin::User { pinned: true }.locks_tear_off());
        assert!(!Pin::User { pinned: false }.locks_tear_off());
        assert!(Pin::System.locks_tear_off());
        assert_eq!(Pin::default(), Pin::Free);
    }
}
