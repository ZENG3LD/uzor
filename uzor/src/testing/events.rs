//! Synthesizes `PlatformEvent`s through the real `EventProcessor` pipeline.

use crate::input::core::event_processor::{EventProcessor, ImeEvent, PlatformEvent};
use crate::input::keyboard::events::KeyCode;
use crate::input::pointer::state::{InputState, ModifierKeys, MouseButton};

/// Synthesizes [`PlatformEvent`]s through the SAME [`EventProcessor`] real
/// window backends use ("Platform Events → EventProcessor → InputState →
/// Widgets" — `event_processor`'s own documented pipeline), accumulating
/// into an owned [`InputState`] a test drives frame by frame via
/// [`super::TestHarness::frame`].
pub struct EventSynthesizer {
    processor: EventProcessor,
    input: InputState,
    time: f64,
}

impl Default for EventSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSynthesizer {
    /// Create a synthesizer with a fresh [`InputState`] and clock at `0.0`.
    pub fn new() -> Self {
        Self {
            processor: EventProcessor::new(),
            input: InputState::new(),
            time: 0.0,
        }
    }

    /// Run one [`PlatformEvent`] through the processor against the
    /// accumulated [`InputState`], at the synthesizer's current clock.
    fn apply(&mut self, event: PlatformEvent) -> &mut Self {
        self.processor.process(&event, &mut self.input, self.time);
        self
    }

    /// Move the pointer without pressing a button.
    pub fn pointer_move(&mut self, x: f64, y: f64) -> &mut Self {
        self.apply(PlatformEvent::PointerMoved { x, y })
    }

    /// Press a pointer button at `(x, y)`.
    pub fn pointer_down(&mut self, x: f64, y: f64, button: MouseButton) -> &mut Self {
        self.apply(PlatformEvent::PointerDown { x, y, button })
    }

    /// Release a pointer button at `(x, y)`.
    pub fn pointer_up(&mut self, x: f64, y: f64, button: MouseButton) -> &mut Self {
        self.apply(PlatformEvent::PointerUp { x, y, button })
    }

    /// `pointer_down` immediately followed by `pointer_up` at the same
    /// point, both applied to the SAME accumulated `InputState` before the
    /// next `TestHarness::frame` call — `PointerUp` only sets
    /// `pointer.clicked` when `button_down` was already `Some` (see
    /// `EventProcessor::process`'s own `PointerUp` handling), so this
    /// single call is enough to produce a `clicked` response on the NEXT
    /// frame. Use two separate `TestHarness::frame` calls around
    /// `pointer_down` / `pointer_up` instead when the test needs to
    /// observe the held-down (pressed) frame in between (e.g. a "pressed"
    /// golden).
    pub fn click(&mut self, x: f64, y: f64) -> &mut Self {
        self.click_button(x, y, MouseButton::Left)
    }

    /// [`Self::click`] with an explicit button.
    pub fn click_button(&mut self, x: f64, y: f64, button: MouseButton) -> &mut Self {
        self.pointer_down(x, y, button);
        self.pointer_up(x, y, button)
    }

    /// A sequence of `pointer_move` steps from `from` to `to`, bracketed by
    /// `pointer_down` / `pointer_up` — `steps` intermediate positions
    /// (clamped to at least `1`; the final move always lands exactly on
    /// `to`).
    pub fn drag(&mut self, from: (f64, f64), to: (f64, f64), steps: u32, button: MouseButton) -> &mut Self {
        self.pointer_down(from.0, from.1, button);
        let steps = steps.max(1);
        for step in 1..=steps {
            let t = f64::from(step) / f64::from(steps);
            let x = from.0 + (to.0 - from.0) * t;
            let y = from.1 + (to.1 - from.1) * t;
            self.pointer_move(x, y);
        }
        self.pointer_up(to.0, to.1, button)
    }

    /// Mouse wheel / trackpad scroll delta.
    pub fn wheel(&mut self, dx: f64, dy: f64) -> &mut Self {
        self.apply(PlatformEvent::Scroll { dx, dy })
    }

    /// Press a key.
    pub fn key_down(&mut self, key: KeyCode, modifiers: ModifierKeys) -> &mut Self {
        self.apply(PlatformEvent::KeyDown { key, modifiers })
    }

    /// Release a key.
    pub fn key_up(&mut self, key: KeyCode, modifiers: ModifierKeys) -> &mut Self {
        self.apply(PlatformEvent::KeyUp { key, modifiers })
    }

    /// Text input (e.g. direct character entry).
    pub fn text(&mut self, text: &str) -> &mut Self {
        self.apply(PlatformEvent::TextInput { text: text.to_owned() })
    }

    /// IME preedit (composition) update.
    pub fn ime_preedit(&mut self, text: &str, sel: Option<(usize, usize)>) -> &mut Self {
        self.apply(PlatformEvent::Ime(ImeEvent::Preedit(text.to_owned(), sel)))
    }

    /// IME commit — composition finished, text is final.
    pub fn ime_commit(&mut self, text: &str) -> &mut Self {
        self.apply(PlatformEvent::Ime(ImeEvent::Commit(text.to_owned())))
    }

    /// Window gained/lost OS focus.
    pub fn focus_window(&mut self, focused: bool) -> &mut Self {
        self.apply(PlatformEvent::WindowFocused(focused))
    }

    /// Advance the synthesizer's clock without emitting an event — call
    /// between e.g. `pointer_down` and `pointer_up` so they land in
    /// different synthesized frames rather than the same one.
    pub fn tick(&mut self, dt: f64) -> &mut Self {
        self.time += dt;
        self
    }

    /// Snapshot for this frame's `begin_frame` call.
    pub fn input_state(&self) -> InputState {
        self.input.clone()
    }

    /// Clears one-shot fields for the next frame — delegates to
    /// [`InputState::end_frame`] plus an explicit scroll-delta reset (that
    /// method does not clear `scroll_delta`; only `InputState::consume_scroll`
    /// does).
    pub fn end_frame(&mut self) {
        self.input.end_frame();
        self.input.scroll_delta = (0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_sets_clicked_on_the_accumulated_input_state() {
        let mut synth = EventSynthesizer::new();
        synth.click(10.0, 10.0);
        assert_eq!(synth.input_state().pointer.clicked, Some(MouseButton::Left));
    }

    #[test]
    fn click_button_uses_the_requested_button() {
        let mut synth = EventSynthesizer::new();
        synth.click_button(10.0, 10.0, MouseButton::Right);
        assert_eq!(synth.input_state().pointer.clicked, Some(MouseButton::Right));
    }

    #[test]
    fn drag_lands_on_target_after_the_right_number_of_intermediate_moves() {
        let mut synth = EventSynthesizer::new();
        synth.drag((0.0, 0.0), (100.0, 0.0), 4, MouseButton::Left);
        let state = synth.input_state();
        assert_eq!(state.pointer.pos, Some((100.0, 0.0)));
        // The move immediately before the final one lands at 3/4 of the
        // path — only reachable if exactly 4 intermediate `PointerMoved`
        // events were applied (1 step would leave `prev_pos` at the drag's
        // own start instead).
        assert_eq!(state.pointer.prev_pos, Some((75.0, 0.0)));
    }

    #[test]
    fn drag_with_one_step_moves_directly_from_start_to_target() {
        let mut synth = EventSynthesizer::new();
        synth.drag((0.0, 0.0), (100.0, 0.0), 1, MouseButton::Left);
        let state = synth.input_state();
        assert_eq!(state.pointer.pos, Some((100.0, 0.0)));
        assert_eq!(state.pointer.prev_pos, Some((0.0, 0.0)));
    }

    #[test]
    fn end_frame_clears_clicked_and_scroll_delta() {
        let mut synth = EventSynthesizer::new();
        synth.click(10.0, 10.0);
        synth.wheel(0.0, 5.0);
        synth.end_frame();
        let state = synth.input_state();
        assert_eq!(state.pointer.clicked, None);
        assert_eq!(state.scroll_delta, (0.0, 0.0));
    }

    #[test]
    fn tick_advances_the_clock_without_emitting_an_event() {
        let mut synth = EventSynthesizer::new();
        synth.tick(0.5);
        assert_eq!(synth.time, 0.5);
        assert_eq!(synth.input_state().pointer.pos, None);
    }
}
