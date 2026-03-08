//! UZOR central context and Immediate Mode API
//!
//! The Context is the primary entry point for the UZOR API. It manages
//! input processing, layout computation, and persistent widget state.
//!
//! This is a HEADLESS architecture - Context only handles geometry and interaction,
//! not rendering. Platforms are responsible for visual output.

use crate::animation::AnimationCoordinator;
use crate::input::InputState;
use crate::layout::tree::LayoutTree;
use crate::state::StateRegistry;
use crate::types::{Rect, WidgetId, WidgetState, ScrollState};
use crate::widgets;
use serde::{Deserialize, Serialize};

/// Button interaction response (used by Context::button)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ButtonResponse {
    /// Whether button was clicked this frame
    pub clicked: bool,
    /// Whether button is currently hovered
    pub hovered: bool,
    /// Whether button is currently pressed
    pub pressed: bool,
    /// Current widget state
    pub state: WidgetState,
    /// Button rectangle (for platform rendering)
    pub rect: Rect,
}

/// The central brain of the UZOR engine
pub struct Context {
    /// Transient input state for the current frame
    pub input: InputState,

    /// Calculated layout rectangles for all widgets
    pub layout: LayoutTree,

    /// Persistent behavioral state (scroll, focus, etc.)
    pub registry: StateRegistry,

    /// Animation coordinator for managing widget animations
    pub animations: AnimationCoordinator,

    /// Time since startup in seconds (for animations)
    pub time: f64,
}

impl Context {
    pub fn new(root_node: crate::layout::types::LayoutNode) -> Self {
        Self {
            input: InputState::default(),
            layout: LayoutTree::new(root_node),
            registry: StateRegistry::new(),
            animations: AnimationCoordinator::new(),
            time: 0.0,
        }
    }

    /// Begin a new frame with updated input
    pub fn begin_frame(&mut self, input: InputState, viewport: Rect) {
        self.input = input;
        self.time = self.input.time;

        // Update animations for this frame
        self.animations.update(self.time);

        // Re-compute layout based on current viewport
        self.layout.compute(viewport);
    }

    /// Access persistent state for a widget
    pub fn state<T: 'static + Send + Sync + Default>(&mut self, id: impl Into<WidgetId>) -> &mut T {
        self.registry.get_or_insert_with(id.into(), T::default)
    }

    /// Helper to get widget rectangle from computed layout
    pub fn widget_rect(&self, id: &WidgetId) -> Rect {
        self.layout.get_rect(id).unwrap_or_default()
    }

    // =========================================================================
    // Immediate Mode API (Headless - Interaction Detection Only)
    // =========================================================================

    /// Calculate button interaction state
    pub fn button(&mut self, id: impl Into<WidgetId>) -> ButtonResponse {
        let id = id.into();
        let rect = self.widget_rect(&id);
        let is_hovered = self.input.is_hovered(&rect);
        let clicked = is_hovered && self.input.is_clicked();

        let state = if clicked {
            WidgetState::Pressed
        } else if is_hovered {
            WidgetState::Hovered
        } else {
            WidgetState::Normal
        };

        ButtonResponse {
            clicked,
            hovered: is_hovered,
            pressed: clicked,
            state,
            rect,
        }
    }

    /// Calculate checkbox interaction state
    pub fn checkbox(&mut self, id: impl Into<WidgetId>, checked: bool) -> widgets::checkbox::CheckboxResponse {
        let id = id.into();
        let rect = self.widget_rect(&id);
        let is_hovered = self.input.is_hovered(&rect);
        let clicked = is_hovered && self.input.is_clicked();

        let state = if clicked {
            WidgetState::Pressed
        } else if is_hovered {
            WidgetState::Hovered
        } else {
            WidgetState::Normal
        };

        let toggled = clicked;
        let new_checked = if toggled { !checked } else { checked };

        widgets::checkbox::CheckboxResponse {
            toggled,
            new_checked,
            hovered: is_hovered,
            state,
            rect,
        }
    }

    /// Calculate scroll area geometry and physics
    pub fn scroll_area(&mut self, id: impl Into<WidgetId>, content_height: f64) -> (Rect, ScrollState) {
        let id = id.into();
        let viewport = self.widget_rect(&id);
        let is_hovered = self.input.is_hovered(&viewport);

        let dt = self.input.dt;
        let scroll_delta_y = if is_hovered { self.input.scroll_delta.1 } else { 0.0 };

        let state = self.state::<ScrollState>(id);
        state.content_size = content_height;

        if scroll_delta_y != 0.0 {
            state.velocity -= scroll_delta_y * 1500.0;
        }

        state.offset += state.velocity * dt;
        state.velocity *= 0.90;

        if state.velocity.abs() < 0.1 {
            state.velocity = 0.0;
        }

        let max_scroll = (content_height - viewport.height).max(0.0);
        if state.offset < 0.0 {
            state.offset = 0.0;
            state.velocity = 0.0;
        } else if state.offset > max_scroll {
            state.offset = max_scroll;
            state.velocity = 0.0;
        }

        (viewport, state.clone())
    }

    /// Calculate interaction state for an icon button
    pub fn icon_button(
        &mut self,
        id: impl Into<WidgetId>,
    ) -> widgets::icon_button::IconButtonResponse {
        let id = id.into();
        let rect = self.widget_rect(&id);
        let is_hovered = self.input.is_hovered(&rect);
        let clicked = is_hovered && self.input.is_clicked();

        let state = if is_hovered {
            if self.input.is_mouse_down() {
                WidgetState::Pressed
            } else {
                WidgetState::Hovered
            }
        } else {
            WidgetState::Normal
        };

        if clicked {
            println!("[UZOR Core] Button '{:?}' CLICKED at rect {:?}", id, rect);
        } else if is_hovered {
            println!("[UZOR Core] Button '{:?}' HOVERED at rect {:?}", id, rect);
        }

        widgets::icon_button::IconButtonResponse {
            clicked,
            hovered: is_hovered,
            state,
        }
    }
}
