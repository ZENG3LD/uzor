//! Per-region render scheduling.
//!
//! A `RenderRegion` is a sub-rect of the window that has its own paint
//! cadence — independent of mouse events and of the window-level FPS cap.
//!
//! Three modes are expressed through the same `target_fps` field:
//!
//! | `target_fps`          | Mode                | When it repaints                    |
//! |-----------------------|---------------------|-------------------------------------|
//! | `0`                   | Dirty-driven        | Only when `dirty` is set by the app |
//! | `1..=u32::MAX-1`      | FPS-capped          | Every `1/target_fps` seconds        |
//! | `u32::MAX`            | Uncapped (continuous) | Every event-loop tick             |
//!
//! Multiple regions can coexist inside one window with different cadences:
//! a header that only redraws on events (`0`), a chart that ticks at 60 fps,
//! and a metrics readout that ticks at 240 fps.  The window-level event
//! loop wakes itself for the soonest due region.

use std::time::Instant;

use crate::types::Rect;

/// Sentinel value for "uncapped — repaint every event-loop tick".
pub const UNCAPPED_FPS: u32 = u32::MAX;

/// One paintable sub-region of a window.
#[derive(Debug, Clone)]
pub struct RenderRegion {
    /// Stable identifier — used by the scheduler to track per-region timing
    /// across frames.  Must be unique within a single window.
    pub id: &'static str,

    /// Logical-pixel rect of the region inside its window.
    pub rect: Rect,

    /// Target repaint cadence.  See [`RenderRegion`] docs for the three modes.
    pub target_fps: u32,

    /// App-set flag: `true` means "I changed something visible — repaint".
    /// Cleared automatically by the scheduler after the region is repainted.
    pub dirty: bool,
}

impl RenderRegion {
    /// Convenience constructor for the dirty-driven mode (`target_fps = 0`).
    pub fn dirty_driven(id: &'static str, rect: Rect) -> Self {
        Self { id, rect, target_fps: 0, dirty: true }
    }

    /// Convenience constructor for the FPS-capped mode.
    pub fn capped(id: &'static str, rect: Rect, fps: u32) -> Self {
        Self { id, rect, target_fps: fps, dirty: true }
    }

    /// Convenience constructor for the uncapped (continuous) mode.
    pub fn uncapped(id: &'static str, rect: Rect) -> Self {
        Self { id, rect, target_fps: UNCAPPED_FPS, dirty: true }
    }
}

/// Per-region scheduler state — owned by the runtime, not the app.
///
/// The runtime keeps one of these per `RenderRegion::id` per window and
/// uses it to decide whether the region needs repainting on the current
/// event-loop wake-up.
#[derive(Debug, Clone)]
pub struct RegionScheduleState {
    pub last_painted: Option<Instant>,
}

impl Default for RegionScheduleState {
    fn default() -> Self {
        Self { last_painted: None }
    }
}

impl RegionScheduleState {
    /// `true` when the region's paint cadence says it should rebuild this
    /// wake-up.  Combined with the region's `dirty` flag for mode `0`.
    pub fn due(&self, region: &RenderRegion, now: Instant) -> bool {
        match region.target_fps {
            0 => region.dirty,
            UNCAPPED_FPS => true,
            fps => match self.last_painted {
                None => true,
                Some(t) => {
                    let target = std::time::Duration::from_secs_f64(1.0 / fps as f64);
                    now.duration_since(t) >= target
                }
            },
        }
    }

    /// Earliest [`Instant`] at which the region wants its next paint.
    /// Returns `None` for dirty-driven regions that aren't currently dirty
    /// (the runtime schedules them only when an event arrives).
    pub fn next_due(&self, region: &RenderRegion, now: Instant) -> Option<Instant> {
        match region.target_fps {
            0 => if region.dirty { Some(now) } else { None },
            UNCAPPED_FPS => Some(now),
            fps => match self.last_painted {
                None => Some(now),
                Some(t) => {
                    let target = std::time::Duration::from_secs_f64(1.0 / fps as f64);
                    Some(t + target)
                }
            },
        }
    }
}
