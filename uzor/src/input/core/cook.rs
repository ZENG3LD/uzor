//! Cook state — what the platform event stream does not carry: click count
//! (single / double / triple), drag arming past a threshold, and pointer grab.
//!
//! Pure data plus pure transitions, no clock and no I/O: every method takes
//! the caller's monotonic time `now` in **seconds**, so the same sequence of
//! calls always gives the same result. `InputCoordinator::end_frame` drives it
//! once per frame; multi-click counting needs the host to stamp
//! `InputState::time` (seconds, > 0).
//!
//! Constants match egui's defaults (`max_click_dist = 6`, double-click
//! window 0.35 s), the same values tessera's cook layer proved.

use crate::input::pointer::MouseButton;
use crate::types::WidgetId;

/// Pointer travel (logical px) past which a held press becomes a drag.
pub const DRAG_THRESHOLD_PX: f64 = 6.0;

/// Time (seconds) between a release and the next press for the press to
/// continue a multi-click sequence.
pub const DOUBLE_CLICK_WINDOW_S: f64 = 0.35;

/// Distance (logical px) between a release and the next press beyond which
/// the multi-click sequence restarts at 1.
pub const MULTI_CLICK_MAX_DIST_PX: f64 = 6.0;

/// Highest click count reported; a fourth quick click stays a triple click.
pub const MAX_CLICK_COUNT: u8 = 3;

/// Where and when a still-held press began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressOrigin {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    /// Press time, seconds.
    pub t: f64,
}

/// A finished press: where, when and with which button it was released.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Release {
    pub x: f64,
    pub y: f64,
    /// Release time, seconds.
    pub t: f64,
    pub button: MouseButton,
}

/// Click-count, drag-arm and grab state of one pointer.
///
/// `Default` is "no press held, no recent release, no grab".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CookState {
    /// The press currently held; `None` between presses.
    pub press_origin: Option<PressOrigin>,
    /// The held press has travelled at least [`DRAG_THRESHOLD_PX`] from its
    /// origin. Stays set until release.
    pub drag_armed: bool,
    /// The last release, for multi-click windowing.
    pub last_release: Option<Release>,
    /// Clicks in the current sequence: 1 single, 2 double, 3 triple; 0
    /// before the first press.
    pub click_count: u8,
    /// Widget holding an exclusive pointer grab: while set, pointer events
    /// go to it regardless of what is under the cursor.
    pub grabbed: Option<WidgetId>,
    /// Last pointer position seen, for motion deltas.
    pub last_pos: Option<(f64, f64)>,
}

/// Click count for a press at `(x, y)` with `button` at `now`, given the
/// previous count and the last release.
///
/// Continues the sequence (capped at [`MAX_CLICK_COUNT`]) when the last
/// release used the same button, happened at most
/// [`DOUBLE_CLICK_WINDOW_S`] ago and at most [`MULTI_CLICK_MAX_DIST_PX`]
/// away; otherwise starts again at 1.
pub fn next_click_count(
    previous: u8,
    last_release: Option<Release>,
    x: f64,
    y: f64,
    button: MouseButton,
    now: f64,
) -> u8 {
    let continues = match last_release {
        Some(r) => {
            r.button == button
                && now - r.t <= DOUBLE_CLICK_WINDOW_S
                && now >= r.t
                && (x - r.x).hypot(y - r.y) <= MULTI_CLICK_MAX_DIST_PX
        }
        None => false,
    };
    if continues {
        previous.saturating_add(1).clamp(1, MAX_CLICK_COUNT)
    } else {
        1
    }
}

/// Whether the pointer at `(x, y)` is at least [`DRAG_THRESHOLD_PX`] from
/// the press origin.
pub fn crosses_drag_threshold(origin: &PressOrigin, x: f64, y: f64) -> bool {
    (x - origin.x).hypot(y - origin.y) >= DRAG_THRESHOLD_PX
}

impl CookState {
    /// A press began. Updates the click count and returns it.
    pub fn press(&mut self, x: f64, y: f64, button: MouseButton, now: f64) -> u8 {
        self.click_count = next_click_count(self.click_count, self.last_release, x, y, button, now);
        self.press_origin = Some(PressOrigin { x, y, button, t: now });
        self.drag_armed = false;
        self.last_pos = Some((x, y));
        self.click_count
    }

    /// The pointer moved. Returns the delta from the last position seen and
    /// whether a held press is (now or already) armed as a drag.
    pub fn motion(&mut self, x: f64, y: f64) -> Motion {
        let (dx, dy) = match self.last_pos {
            Some((px, py)) => (x - px, y - py),
            None => (0.0, 0.0),
        };
        self.last_pos = Some((x, y));
        let was_armed = self.drag_armed;
        if let Some(origin) = &self.press_origin {
            if !self.drag_armed && crosses_drag_threshold(origin, x, y) {
                self.drag_armed = true;
            }
        }
        Motion { dx, dy, drag_armed: self.drag_armed, drag_started: self.drag_armed && !was_armed }
    }

    /// A press ended. Returns whether it ended a drag (the held press had
    /// been armed); a release that ends a drag is not a click.
    pub fn release(&mut self, x: f64, y: f64, button: MouseButton, now: f64) -> bool {
        let was_drag = self.drag_armed;
        self.last_release = Some(Release { x, y, t: now, button });
        self.press_origin = None;
        self.drag_armed = false;
        self.last_pos = Some((x, y));
        was_drag
    }

    /// Route every pointer event to `id` until [`Self::release_grab`].
    pub fn grab(&mut self, id: WidgetId) {
        self.grabbed = Some(id);
    }

    /// End the pointer grab, if any.
    pub fn release_grab(&mut self) {
        self.grabbed = None;
    }

    /// Whether the current sequence is a double click (exactly 2).
    pub fn is_double(&self) -> bool {
        self.click_count == 2
    }

    /// Whether the current sequence is a triple click.
    pub fn is_triple(&self) -> bool {
        self.click_count == MAX_CLICK_COUNT
    }
}

/// Result of [`CookState::motion`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub dx: f64,
    pub dy: f64,
    /// A press is held and has crossed the drag threshold.
    pub drag_armed: bool,
    /// This motion is the one that crossed the threshold.
    pub drag_started: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: MouseButton = MouseButton::Left;

    fn click(state: &mut CookState, x: f64, y: f64, t: f64) -> u8 {
        let n = state.press(x, y, L, t);
        state.release(x, y, L, t + 0.05);
        n
    }

    #[test]
    fn first_press_is_a_single_click() {
        let mut s = CookState::default();
        assert_eq!(s.click_count, 0);
        assert_eq!(s.press(10.0, 10.0, L, 0.0), 1);
    }

    #[test]
    fn quick_presses_count_up_to_three_and_stay_there() {
        let mut s = CookState::default();
        assert_eq!(click(&mut s, 10.0, 10.0, 0.0), 1);
        assert_eq!(click(&mut s, 11.0, 10.0, 0.2), 2);
        assert!(s.is_double());
        assert_eq!(click(&mut s, 11.0, 11.0, 0.4), 3);
        assert!(s.is_triple());
        assert_eq!(click(&mut s, 11.0, 11.0, 0.6), 3);
    }

    #[test]
    fn a_slow_second_press_restarts_the_sequence() {
        let mut s = CookState::default();
        click(&mut s, 10.0, 10.0, 0.0);
        // release at 0.05; 0.05 + 0.35 < 0.5
        assert_eq!(s.press(10.0, 10.0, L, 0.5), 1);
    }

    #[test]
    fn the_window_edge_is_inclusive() {
        let r = Release { x: 0.0, y: 0.0, t: 0.0, button: L };
        assert_eq!(next_click_count(1, Some(r), 0.0, 0.0, L, DOUBLE_CLICK_WINDOW_S), 2);
        assert_eq!(next_click_count(1, Some(r), 0.0, 0.0, L, DOUBLE_CLICK_WINDOW_S + 1e-9), 1);
    }

    #[test]
    fn a_far_or_other_button_press_restarts_the_sequence() {
        let mut s = CookState::default();
        click(&mut s, 10.0, 10.0, 0.0);
        assert_eq!(s.press(10.0 + MULTI_CLICK_MAX_DIST_PX + 0.1, 10.0, L, 0.1), 1);
        s.release(16.1, 10.0, L, 0.15);
        assert_eq!(s.press(16.1, 10.0, MouseButton::Right, 0.2), 1);
    }

    #[test]
    fn a_clock_going_backwards_does_not_continue_a_sequence() {
        let r = Release { x: 0.0, y: 0.0, t: 5.0, button: L };
        assert_eq!(next_click_count(1, Some(r), 0.0, 0.0, L, 4.9), 1);
    }

    #[test]
    fn drag_arms_once_at_the_threshold_and_release_reports_it() {
        let mut s = CookState::default();
        s.press(0.0, 0.0, L, 0.0);
        let m = s.motion(3.0, 0.0);
        assert!(!m.drag_armed && !m.drag_started);
        assert_eq!((m.dx, m.dy), (3.0, 0.0));
        let m = s.motion(DRAG_THRESHOLD_PX, 0.0);
        assert!(m.drag_armed && m.drag_started);
        // back inside the threshold: stays armed, does not re-start
        let m = s.motion(1.0, 0.0);
        assert!(m.drag_armed && !m.drag_started);
        assert!(s.release(1.0, 0.0, L, 0.3), "release after a drag reports the drag");
        assert!(!s.drag_armed);
        assert!(s.press_origin.is_none());
    }

    #[test]
    fn motion_without_a_press_never_arms() {
        let mut s = CookState::default();
        let m = s.motion(100.0, 100.0);
        assert!(!m.drag_armed);
        assert_eq!((m.dx, m.dy), (0.0, 0.0));
        let m = s.motion(110.0, 100.0);
        assert_eq!((m.dx, m.dy), (10.0, 0.0));
        assert!(!m.drag_armed);
    }

    #[test]
    fn a_plain_click_release_is_not_a_drag() {
        let mut s = CookState::default();
        s.press(0.0, 0.0, L, 0.0);
        s.motion(2.0, 2.0);
        assert!(!s.release(2.0, 2.0, L, 0.1));
    }

    #[test]
    fn grab_is_set_and_cleared() {
        let mut s = CookState::default();
        s.grab(WidgetId::new("splitter"));
        assert_eq!(s.grabbed, Some(WidgetId::new("splitter")));
        s.release_grab();
        assert_eq!(s.grabbed, None);
    }
}
