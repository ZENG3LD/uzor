//! Headless per-frame widget test harness.

use crate::a11y::A11yTree;
use crate::input::{InputCoordinator, WidgetResponse};
use crate::types::WidgetId;

use super::events::EventSynthesizer;

/// Drives an [`InputCoordinator`] through the real `begin_frame` /
/// `end_frame` lifecycle, one synthesized frame at a time.
///
/// # The warm-up frame
///
/// `InputCoordinator::begin_frame` bakes this frame's hover/press state by
/// hit-testing the pointer position against the widgets registered during
/// the PREVIOUS frame's `build` call (see that method's own doc comment —
/// this is deliberate: it fixes a z-order leak where mid-frame baking saw
/// an incomplete widget list). One consequence: a widget registered for
/// the FIRST time inside a `frame()` call is never `Hovered`/`Pressed` in
/// that SAME call, even if the pointer already sits over it — the
/// coordinator has nothing to hit-test against yet. Call `frame()` (or
/// [`Self::settle`]) once more with the same registrations before
/// asserting `Hovered`/`Pressed` for a widget that has only been
/// registered once so far.
pub struct TestHarness {
    /// The coordinator under test.
    pub coordinator: InputCoordinator,
    /// The event synthesizer driving `coordinator`'s input.
    pub events: EventSynthesizer,
    frame_a11y: A11yTree,
}

/// What one [`TestHarness::frame`] call produced.
pub struct FrameOutcome {
    /// Widget responses `end_frame` returned this frame.
    pub responses: Vec<(WidgetId, WidgetResponse)>,
    /// The accessibility tree `build` pushed nodes into this frame.
    pub a11y: A11yTree,
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl TestHarness {
    /// Create a harness with a fresh [`InputCoordinator`] and
    /// [`EventSynthesizer`].
    pub fn new() -> Self {
        Self {
            coordinator: InputCoordinator::new(),
            events: EventSynthesizer::new(),
            frame_a11y: A11yTree::new(),
        }
    }

    /// Drive exactly one frame: feed `events`'s current `InputState` into
    /// `begin_frame`, run `build` (the test's own register/draw closure —
    /// same shape a real per-frame `ui()` callback takes: register widgets
    /// on `coord`, push this frame's `A11yNode`s into the tree), call
    /// `end_frame`, then `events.end_frame()` to clear one-shot input
    /// state for the next call.
    ///
    /// See the struct-level doc for the warm-up-frame caveat around
    /// first-time registrations.
    pub fn frame<R>(
        &mut self,
        build: impl FnOnce(&mut InputCoordinator, &mut A11yTree) -> R,
    ) -> (R, FrameOutcome) {
        self.coordinator.begin_frame(self.events.input_state());
        self.frame_a11y.clear();
        let result = build(&mut self.coordinator, &mut self.frame_a11y);
        let responses = self.coordinator.end_frame();
        self.events.end_frame();
        (
            result,
            FrameOutcome {
                responses,
                a11y: self.frame_a11y.clone(),
            },
        )
    }

    /// Run one additional frame using the CURRENT input snapshot with no
    /// new events synthesized — the explicit warm-up call for the caveat
    /// documented on [`Self::frame`]. Named separately (rather than having
    /// `frame()` silently run twice) so a test's warm-up is visible at the
    /// call site.
    pub fn settle<R>(
        &mut self,
        build: impl FnOnce(&mut InputCoordinator, &mut A11yTree) -> R,
    ) -> (R, FrameOutcome) {
        self.frame(build)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::pointer::state::MouseButton;
    use crate::input::Sense;
    use crate::types::{Rect, WidgetState};

    fn register(coord: &mut InputCoordinator, id: WidgetId, rect: Rect) {
        coord.register(id, rect, Sense::CLICK);
    }

    #[test]
    fn click_through_a_trivial_widget_reports_clicked() {
        let id = WidgetId::from("trivial-click");
        let rect = Rect::new(0.0, 0.0, 40.0, 20.0);
        let mut harness = TestHarness::new();

        // Frame 1 — first-ever registration.
        harness.frame(|coord, _| register(coord, id.clone(), rect));

        // Frame 2 — press: frame 1 already registered this widget/rect, so
        // this frame's `begin_frame` bake reports `Pressed` immediately.
        harness.events.pointer_down(10.0, 10.0, MouseButton::Left);
        harness.frame(|coord, _| register(coord, id.clone(), rect));
        assert_eq!(harness.coordinator.widget_state(&id), WidgetState::Pressed);

        // Frame 3 — release: `end_frame`'s own THIS-frame hit test (not
        // the `begin_frame` bake) reports `clicked` straight off this
        // frame's registration, no extra warm-up needed.
        harness.events.pointer_up(10.0, 10.0, MouseButton::Left);
        let (_, outcome) = harness.frame(|coord, _| register(coord, id.clone(), rect));
        let (_, response) = outcome
            .responses
            .iter()
            .find(|(wid, _)| wid == &id)
            .expect("clicked response must exist");
        assert!(response.clicked);
    }

    #[test]
    fn drag_in_while_held_does_not_press_the_widget_dragged_onto() {
        let widget_a = WidgetId::from("drag-a");
        let widget_b = WidgetId::from("drag-b");
        let rect_a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let rect_b = Rect::new(100.0, 0.0, 50.0, 50.0);
        let mut harness = TestHarness::new();

        let build = |coord: &mut InputCoordinator, _: &mut A11yTree| {
            register(coord, widget_a.clone(), rect_a);
            register(coord, widget_b.clone(), rect_b);
        };

        // Frame 1 — first-ever registration of both widgets.
        harness.frame(build);

        // Frame 2 — press on widget A.
        harness.events.pointer_down(25.0, 25.0, MouseButton::Left);
        harness.frame(build);
        assert_eq!(harness.coordinator.widget_state(&widget_a), WidgetState::Pressed);

        // Frame 3 — drag the still-held pointer onto widget B.
        harness.events.pointer_move(125.0, 25.0);
        harness.frame(build);
        assert_eq!(
            harness.coordinator.widget_state(&widget_b),
            WidgetState::Hovered,
            "press-origin widget A must not transfer Pressed onto B on drag-in"
        );
        assert_eq!(harness.coordinator.widget_state(&widget_a), WidgetState::Normal);
    }

    #[test]
    fn settle_makes_a_first_time_registration_hoverable() {
        let id = WidgetId::from("first-timer");
        let rect = Rect::new(0.0, 0.0, 40.0, 20.0);
        let mut harness = TestHarness::new();
        harness.events.pointer_move(10.0, 10.0);

        // Frame 1 — first-ever registration: `begin_frame`'s bake ran
        // against the EMPTY previous-frame widget list, so the widget
        // cannot be Hovered yet even though the pointer already sits
        // inside its rect.
        harness.frame(|coord, _| register(coord, id.clone(), rect));
        assert_eq!(harness.coordinator.widget_state(&id), WidgetState::Normal);

        // settle(): one more frame with no NEW input re-registers the
        // same widget; `begin_frame`'s bake now sees frame 1's
        // registration.
        harness.settle(|coord, _| register(coord, id.clone(), rect));
        assert_eq!(harness.coordinator.widget_state(&id), WidgetState::Hovered);
    }
}
