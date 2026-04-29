//! UZOR central context — retained-mode coordinator wrapper.
//!
//! `ContextManager` is the primary entry point for the UZOR API. It wraps an
//! `InputCoordinator` (retained-mode input routing) together with layout, state,
//! and animation subsystems.
//!
//! This is a HEADLESS architecture — ContextManager only handles geometry and
//! interaction, not rendering. Platforms are responsible for visual output.

use crate::ui::animation::AnimationCoordinator;
use crate::input::{InputState, InputCoordinator, WidgetResponse};
use super::layout::tree::LayoutTree;
use super::state::StateRegistry;
use crate::types::{Rect, WidgetId};

/// The central brain of the UZOR engine.
///
/// Wraps `InputCoordinator` for retained-mode input routing plus layout,
/// persistent state, and animation.
pub struct ContextManager {
    /// Retained-mode input coordinator (hit-testing, z-order, event routing).
    pub input: InputCoordinator,

    /// Calculated layout rectangles for all widgets.
    pub layout: LayoutTree,

    /// Persistent behavioral state (scroll, focus, etc.).
    pub registry: StateRegistry,

    /// Animation coordinator for managing widget animations.
    pub animations: AnimationCoordinator,

    /// Time since startup in seconds (for animations).
    pub time: f64,
}

impl ContextManager {
    pub fn new(root_node: super::layout::types::LayoutNode) -> Self {
        Self {
            input: InputCoordinator::new(),
            layout: LayoutTree::new(root_node),
            registry: StateRegistry::new(),
            animations: AnimationCoordinator::new(),
            time: 0.0,
        }
    }

    /// Begin a new frame with updated input state and viewport.
    ///
    /// Delegates to `InputCoordinator::begin_frame`, then recomputes layout
    /// and advances animations.
    pub fn begin_frame(&mut self, input: InputState, viewport: Rect) {
        self.time = input.time;
        self.input.begin_frame(input);

        // Advance animations for this frame.
        self.animations.update(self.time);

        // Re-compute layout based on current viewport.
        self.layout.compute(viewport);
    }

    /// End the current frame and collect widget responses.
    ///
    /// Delegates to `InputCoordinator::end_frame`.
    pub fn end_frame(&mut self) -> Vec<(WidgetId, WidgetResponse)> {
        self.input.end_frame()
    }

    /// Access persistent state for a widget.
    pub fn state<T: 'static + Send + Sync + Default>(&mut self, id: impl Into<WidgetId>) -> &mut T {
        self.registry.get_or_insert_with(id.into(), T::default)
    }

    /// Get the computed layout rectangle for a widget.
    pub fn widget_rect(&self, id: &WidgetId) -> Rect {
        self.layout.get_rect(id).unwrap_or_default()
    }
}
