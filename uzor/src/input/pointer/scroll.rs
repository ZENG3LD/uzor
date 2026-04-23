//! Centralized scroll input manager
//!
//! Process all scroll-related interactions (wheel, handle drag, track click)
//! in a single `process()` call instead of manual per-scrollbar wiring.

use crate::types::{Rect, ScrollState};
use crate::input::core::widget_state::WidgetId;
use crate::input::core::response::WidgetResponse;

/// Registration entry for a scrollable area
struct ScrollEntry<'a> {
    /// Base widget ID (viewport ID, e.g. "settings_scroll")
    id: WidgetId,
    /// Mutable reference to the scroll state to update
    state: &'a mut ScrollState,
    /// Total content height
    content_height: f64,
    /// Visible viewport height
    viewport_height: f64,
    /// Track rect (for drag ratio calculation)
    track_rect: Rect,
}

/// Centralized scroll input processor.
///
/// Register scrollable areas, then call `process()` with the coordinator's
/// responses to automatically handle wheel, drag, and track click events.
///
/// # Widget ID Convention
///
/// For a scrollable area with base ID `"my_scroll"`:
/// - `"my_scroll"` — viewport (register with `Sense::SCROLL`)
/// - `"my_scroll__handle"` — scrollbar handle (register with `Sense::DRAG`)
/// - `"my_scroll__track"` — scrollbar track (register with `Sense::CLICK`)
///
/// # Example
///
/// ```ignore
/// // During render, register widgets with coordinator:
/// coordinator.register("sidebar_scroll", viewport_rect, Sense::SCROLL);
/// coordinator.register("sidebar_scroll__handle", handle_rect, Sense::DRAG);
/// coordinator.register("sidebar_scroll__track", track_rect, Sense::CLICK);
///
/// // After end_frame:
/// let responses = coordinator.end_frame();
/// let mut sm = ScrollManager::new();
/// sm.register("sidebar_scroll", &mut scroll_state, content_h, viewport_h, track_rect);
/// sm.process(&responses);
/// // scroll_state is now updated — no manual event wiring needed
/// ```
pub struct ScrollManager<'a> {
    entries: Vec<ScrollEntry<'a>>,
}

impl<'a> ScrollManager<'a> {
    /// Create a new scroll manager for this frame.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a scrollable area.
    ///
    /// `id` is the base widget ID (same as registered with coordinator for the viewport).
    /// The handle and track IDs are derived as `"{id}__handle"` and `"{id}__track"`.
    pub fn register(
        &mut self,
        id: impl Into<WidgetId>,
        state: &'a mut ScrollState,
        content_height: f64,
        viewport_height: f64,
        track_rect: Rect,
    ) {
        self.entries.push(ScrollEntry {
            id: id.into(),
            state,
            content_height,
            viewport_height,
            track_rect,
        });
    }

    /// Process all scroll interactions from coordinator responses.
    ///
    /// Handles:
    /// - **Wheel**: routes `scrolled` response to matching viewport → `handle_wheel()`
    /// - **Handle drag start**: `drag_started` on `{id}__handle` → `start_drag(y)`
    /// - **Handle drag move**: `dragged` on `{id}__handle` → `handle_drag(y, ...)`
    /// - **Handle drag end**: `drag_stopped` on `{id}__handle` → `end_drag()`
    /// - **Track click**: not handled (no mouse position available); use `process_with_mouse()`
    pub fn process(self, responses: &[(WidgetId, WidgetResponse)]) {
        for entry in self.entries {
            let handle_id = WidgetId::new(format!("{}__handle", entry.id.0));

            for (resp_id, resp) in responses {
                // Wheel scroll on viewport
                if resp_id == &entry.id && resp.scrolled {
                    entry.state.handle_wheel(
                        resp.scroll_delta.1,
                        entry.content_height,
                        entry.viewport_height,
                    );
                }

                // Scrollbar handle drag
                if resp_id == &handle_id {
                    if resp.drag_started {
                        let start_y = resp.rect.y + resp.rect.height / 2.0;
                        entry.state.start_drag(start_y);
                    }
                    if resp.dragged {
                        if let Some(start_y) = entry.state.drag_start_y {
                            let current_y = start_y + resp.drag_total.1;
                            entry.state.handle_drag(
                                current_y,
                                entry.track_rect.height,
                                entry.content_height,
                                entry.viewport_height,
                            );
                        }
                    }
                    if resp.drag_stopped {
                        entry.state.end_drag();
                    }
                }
            }
        }
    }

    /// Process scroll interactions with explicit mouse position.
    ///
    /// This variant also handles track click (jump to position) using the
    /// provided mouse position for precise positioning.
    pub fn process_with_mouse(
        self,
        responses: &[(WidgetId, WidgetResponse)],
        mouse_pos: Option<(f64, f64)>,
    ) {
        for entry in self.entries {
            let handle_id = WidgetId::new(format!("{}__handle", entry.id.0));
            let track_id = WidgetId::new(format!("{}__track", entry.id.0));

            for (resp_id, resp) in responses {
                // Wheel scroll on viewport
                if resp_id == &entry.id && resp.scrolled {
                    entry.state.handle_wheel(
                        resp.scroll_delta.1,
                        entry.content_height,
                        entry.viewport_height,
                    );
                }

                // Scrollbar handle drag
                if resp_id == &handle_id {
                    if resp.drag_started {
                        let start_y = resp.rect.y + resp.rect.height / 2.0;
                        entry.state.start_drag(start_y);
                    }
                    if resp.dragged {
                        if let Some(start_y) = entry.state.drag_start_y {
                            let current_y = start_y + resp.drag_total.1;
                            entry.state.handle_drag(
                                current_y,
                                entry.track_rect.height,
                                entry.content_height,
                                entry.viewport_height,
                            );
                        }
                    }
                    if resp.drag_stopped {
                        entry.state.end_drag();
                    }
                }

                // Track click — jump to position using mouse Y
                if resp_id == &track_id && resp.clicked {
                    if let Some((_, my)) = mouse_pos {
                        entry.state.handle_track_click(
                            my,
                            entry.track_rect.y,
                            entry.track_rect.height,
                            entry.content_height,
                            entry.viewport_height,
                        );
                    }
                }
            }
        }
    }
}

impl Default for ScrollManager<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;

    fn make_resp(id: &str) -> (WidgetId, WidgetResponse) {
        (WidgetId::new(id), WidgetResponse::default())
    }

    fn scrolled_resp(id: &str, delta_y: f64) -> (WidgetId, WidgetResponse) {
        let mut resp = WidgetResponse::default();
        resp.scrolled = true;
        resp.scroll_delta = (0.0, delta_y);
        (WidgetId::new(id), resp)
    }

    fn drag_started_resp(id: &str, rect_y: f64, rect_h: f64) -> (WidgetId, WidgetResponse) {
        let mut resp = WidgetResponse::default();
        resp.drag_started = true;
        resp.rect = Rect::new(0.0, rect_y, 12.0, rect_h);
        (WidgetId::new(id), resp)
    }

    fn dragged_resp(id: &str, total_y: f64) -> (WidgetId, WidgetResponse) {
        let mut resp = WidgetResponse::default();
        resp.dragged = true;
        resp.drag_total = (0.0, total_y);
        (WidgetId::new(id), resp)
    }

    fn drag_stopped_resp(id: &str) -> (WidgetId, WidgetResponse) {
        let mut resp = WidgetResponse::default();
        resp.drag_stopped = true;
        (WidgetId::new(id), resp)
    }

    fn clicked_resp(id: &str) -> (WidgetId, WidgetResponse) {
        let mut resp = WidgetResponse::default();
        resp.clicked = true;
        (WidgetId::new(id), resp)
    }

    #[test]
    fn test_new_and_default() {
        let sm1 = ScrollManager::new();
        let sm2 = ScrollManager::default();
        assert!(sm1.entries.is_empty());
        assert!(sm2.entries.is_empty());
    }

    #[test]
    fn test_wheel_scroll_updates_offset() {
        let mut state = ScrollState::new();
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![scrolled_resp("my_scroll", 1.0)];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 500.0, 200.0, track_rect);
        sm.process(&responses);

        assert!(state.offset > 0.0, "scroll offset should increase after wheel down");
    }

    #[test]
    fn test_wheel_no_scroll_when_content_fits() {
        let mut state = ScrollState::new();
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![scrolled_resp("my_scroll", 1.0)];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 100.0, 200.0, track_rect);
        sm.process(&responses);

        assert_eq!(state.offset, 0.0, "no scroll when content fits viewport");
    }

    #[test]
    fn test_unrelated_response_ignored() {
        let mut state = ScrollState::new();
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![
            scrolled_resp("other_scroll", 1.0),
            make_resp("random_widget"),
        ];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 500.0, 200.0, track_rect);
        sm.process(&responses);

        assert_eq!(state.offset, 0.0, "responses for other IDs should be ignored");
    }

    #[test]
    fn test_drag_start_sets_drag_state() {
        let mut state = ScrollState::new();
        let track_rect = Rect::new(0.0, 50.0, 12.0, 200.0);
        // handle rect: y=100, height=30 → center_y = 115
        let responses = vec![drag_started_resp("my_scroll__handle", 100.0, 30.0)];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 800.0, 300.0, track_rect);
        sm.process(&responses);

        assert!(state.is_dragging);
        assert_eq!(state.drag_start_y, Some(115.0));
    }

    #[test]
    fn test_drag_end_clears_drag_state() {
        let mut state = ScrollState::new();
        state.start_drag(115.0);
        let track_rect = Rect::new(0.0, 50.0, 12.0, 200.0);
        let responses = vec![drag_stopped_resp("my_scroll__handle")];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 800.0, 300.0, track_rect);
        sm.process(&responses);

        assert!(!state.is_dragging);
        assert_eq!(state.drag_start_y, None);
    }

    #[test]
    fn test_drag_move_updates_offset() {
        let mut state = ScrollState::new();
        // content=600, viewport=200, max_scroll=400
        // track_height=200, visible_ratio=200/600≈0.333, handle_height=max(0.333*200,30)=66.6
        // available=200-66.6=133.4
        // drag total_y=50 → scroll_delta = (50/133.4)*400 ≈ 149.9
        state.start_drag(100.0); // start_y=100, start_offset=0
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![dragged_resp("my_scroll__handle", 50.0)];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 600.0, 200.0, track_rect);
        sm.process(&responses);

        assert!(state.offset > 0.0, "drag should move the scroll offset");
    }

    #[test]
    fn test_track_click_without_mouse_pos_does_nothing() {
        let mut state = ScrollState::new();
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![clicked_resp("my_scroll__track")];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 600.0, 200.0, track_rect);
        sm.process_with_mouse(&responses, None);

        assert_eq!(state.offset, 0.0);
    }

    #[test]
    fn test_track_click_with_mouse_pos_jumps() {
        let mut state = ScrollState::new();
        // track: y=0, height=200, content=600, viewport=200 → max_scroll=400
        // click at y=100 → relative=0.5 → offset=200
        let track_rect = Rect::new(0.0, 0.0, 12.0, 200.0);
        let responses = vec![clicked_resp("my_scroll__track")];

        let mut sm = ScrollManager::new();
        sm.register("my_scroll", &mut state, 600.0, 200.0, track_rect);
        sm.process_with_mouse(&responses, Some((0.0, 100.0)));

        assert!((state.offset - 200.0).abs() < 1.0, "track click should jump to 50% position");
    }

    #[test]
    fn test_multiple_entries_independent() {
        let mut state_a = ScrollState::new();
        let mut state_b = ScrollState::new();
        let track = Rect::new(0.0, 0.0, 12.0, 200.0);

        let responses = vec![
            scrolled_resp("scroll_a", 1.0),
            scrolled_resp("scroll_b", 2.0),
        ];

        let mut sm = ScrollManager::new();
        sm.register("scroll_a", &mut state_a, 500.0, 200.0, track);
        sm.register("scroll_b", &mut state_b, 500.0, 200.0, track);
        sm.process(&responses);

        // Both should have scrolled, b more than a (delta 2 vs 1)
        assert!(state_a.offset > 0.0);
        assert!(state_b.offset > state_a.offset);
    }
}
