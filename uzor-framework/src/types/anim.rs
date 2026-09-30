//! Animation vocabulary: which framework-level values animate, and how long
//! each kind takes.
//!
//! Only framework animations live here (window expand, overlay fade).
//! Widget-internal easing stays in the library widgets.

use crate::types::ids::{OverlaySlot, WindowId};

/// Which expand of a window an animator drives.
///
/// Produced by the LayoutEngine (expand target) and consumed by the
/// AnimationEngine (animator key) and the LayoutEngine again (animated value).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpandKind {
    /// The native window grows outward along one edge (edge gutter expand).
    Outer,
    /// The dock content grows inside the window (inner expand).
    Inner,
}

/// Identity of one framework animator.
///
/// Every key is window-scoped, so closing a window drops all of its
/// animators. `Ord` gives the AnimationEngine a deterministic step order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnimKey {
    /// Expand progress of one window, `0.0` (retracted) to `1.0` (expanded).
    Expand {
        /// The window.
        win: WindowId,
        /// Outer or inner expand.
        kind: ExpandKind,
    },
    /// Opacity of one open overlay, `0.0` (transparent) to `1.0` (opaque).
    OverlayFade {
        /// The overlay's window.
        win: WindowId,
        /// The overlay instance.
        slot: OverlaySlot,
    },
}

impl AnimKey {
    /// The window this animator belongs to.
    pub const fn window(self) -> WindowId {
        match self {
            AnimKey::Expand { win, .. } | AnimKey::OverlayFade { win, .. } => win,
        }
    }
}

/// Durations of the framework's linear animators, in seconds for a full
/// `0 -> 1` sweep; `0` (or less) means instant.
///
/// Part of the runtime configuration; applied by the AnimationEngine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimPolicy {
    /// Window expand (outer and inner).
    pub expand_secs: f64,
    /// Overlay fade in / out.
    pub overlay_fade_secs: f64,
}

impl AnimPolicy {
    /// The sweep duration for one animator key.
    pub const fn secs_for(&self, key: AnimKey) -> f64 {
        match key {
            AnimKey::Expand { .. } => self.expand_secs,
            AnimKey::OverlayFade { .. } => self.overlay_fade_secs,
        }
    }
}

impl Default for AnimPolicy {
    /// `0.06 s` for both, the fade constant the previous kernel used for
    /// its expand animators.
    fn default() -> Self {
        Self {
            expand_secs: 0.06,
            overlay_fade_secs: 0.06,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_window_and_policy_lookup() {
        let e = AnimKey::Expand {
            win: WindowId(2),
            kind: ExpandKind::Outer,
        };
        let f = AnimKey::OverlayFade {
            win: WindowId(3),
            slot: OverlaySlot(9),
        };
        assert_eq!(e.window(), WindowId(2));
        assert_eq!(f.window(), WindowId(3));
        let p = AnimPolicy {
            expand_secs: 0.2,
            overlay_fade_secs: 0.1,
        };
        assert_eq!(p.secs_for(e), 0.2);
        assert_eq!(p.secs_for(f), 0.1);
        assert_eq!(AnimPolicy::default().expand_secs, 0.06);
    }
}
