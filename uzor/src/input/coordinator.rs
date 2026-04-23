//! Input coordinator for widget event routing and Z-order management
//!
//! This module provides `InputCoordinator` - the central orchestrator that connects
//! platform events to widgets through hit testing, Z-order layering, and event routing.
//!
//! It also provides `ScopedRegion` - a nested input coordinator that handles
//! widgets within a bounded screen region (e.g. a chart panel toolbar) without
//! polluting the global widget namespace.

use crate::types::{Rect, WidgetId};
use crate::input::sense::Sense;
use crate::input::response::WidgetResponse;
use crate::input::state::InputState;
use crate::input::widget_state::WidgetInputState;
use super::text_field::{TextFieldStore, TextFieldConfig, TextAction};

/// Layer ID for z-order management
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(pub String);

impl LayerId {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    /// Main application layer (z=0)
    pub fn main() -> Self {
        Self::new("main")
    }

    /// Modal layer (blocks lower layers)
    pub fn modal() -> Self {
        Self::new("modal")
    }

    /// Popup layer (above modal)
    pub fn popup() -> Self {
        Self::new("popup")
    }

    /// Tooltip layer (highest)
    pub fn tooltip() -> Self {
        Self::new("tooltip")
    }
}

impl From<&str> for LayerId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for LayerId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A registered widget in the coordinator
#[derive(Clone, Debug)]
struct RegisteredWidget {
    id: WidgetId,
    rect: Rect,
    sense: Sense,
    layer: LayerId,
}

/// A layer in the z-order stack
#[derive(Clone, Debug)]
struct Layer {
    id: LayerId,
    z_order: u32,
    modal: bool, // blocks events to lower layers
}

/// A scoped input region that manages widgets within a bounded screen area.
///
/// `ScopedRegion` wraps its own `InputCoordinator` and translates parent (screen)
/// coordinates to region-local coordinates before dispatching events. This lets
/// panels such as chart toolbars own their widget namespace without polluting the
/// global coordinator.
///
/// Widget IDs returned from scoped-region operations are prefixed with
/// `"{region_id}:"` so callers can identify which region produced an event.
pub struct ScopedRegion {
    /// Bounding rect in parent (screen) coordinates
    pub rect: Rect,
    /// Child coordinator managing widgets inside this region
    pub coordinator: InputCoordinator,
    /// Unique identifier for this region
    pub id: String,
}

impl ScopedRegion {
    /// Create a new scoped region with the given id and bounding rect.
    pub fn new(id: &str, rect: Rect) -> Self {
        Self {
            rect,
            coordinator: InputCoordinator::new(),
            id: id.to_string(),
        }
    }

    /// Returns `true` if the screen-space point `(x, y)` falls inside this region.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.rect.contains(x, y)
    }

    /// Convert a parent (screen) coordinate to region-local coordinates.
    ///
    /// The child coordinator's widgets are registered with local coordinates
    /// (origin at the top-left of the region), so all hit-tests must be done
    /// with local coords.
    pub fn to_local(&self, x: f64, y: f64) -> (f64, f64) {
        (x - self.rect.x, y - self.rect.y)
    }

    /// Build the prefixed widget ID: `"{region_id}:{widget_id}"`.
    pub fn prefix_id(&self, widget_id: &WidgetId) -> WidgetId {
        WidgetId::new(format!("{}:{}", self.id, widget_id.0))
    }
}

/// Central input coordinator — connects platform events to widgets
pub struct InputCoordinator {
    /// Registered widgets for current frame
    widgets: Vec<RegisteredWidget>,
    /// Z-order layers
    layers: Vec<Layer>,
    /// Per-widget interaction state (persists across frames)
    widget_state: WidgetInputState,
    /// Current frame input state
    input: InputState,
    /// Frame counter
    frame: u64,
    /// Nested scoped regions (e.g. chart panel toolbars)
    scoped_regions: Vec<ScopedRegion>,
    /// Text field store — owns text/cursor/selection state for all text fields
    text_fields: TextFieldStore,
}

impl InputCoordinator {
    /// Create new InputCoordinator
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            layers: vec![Layer {
                id: LayerId::main(),
                z_order: 0,
                modal: false,
            }],
            widget_state: WidgetInputState::new(),
            input: InputState::new(),
            frame: 0,
            scoped_regions: Vec::new(),
            text_fields: TextFieldStore::new(),
        }
    }

    /// Start new frame — clear widget registrations and layers, keep persistent state.
    ///
    /// Propagates the input state to all registered scoped regions, converting
    /// the pointer position to region-local coordinates.  If the pointer is
    /// outside a region its position is set to `None` in the child's
    /// `InputState` so widgets inside do not spuriously report hover.
    pub fn begin_frame(&mut self, input: InputState) {
        self.widgets.clear();
        self.layers.clear();
        self.layers.push(Layer {
            id: LayerId::main(),
            z_order: 0,
            modal: false,
        });
        self.input = input.clone();
        self.frame += 1;
        self.text_fields.begin_frame();

        // Propagate to scoped regions with coordinate conversion.
        for region in &mut self.scoped_regions {
            let mut local_input = input.clone();
            if let Some((px, py)) = input.pointer.pos {
                if region.rect.contains(px, py) {
                    let (lx, ly) = (px - region.rect.x, py - region.rect.y);
                    local_input.pointer.pos = Some((lx, ly));
                    if let Some((ppx, ppy)) = input.pointer.prev_pos {
                        local_input.pointer.prev_pos = Some((ppx - region.rect.x, ppy - region.rect.y));
                    }
                } else {
                    // Pointer is outside — hide it from the child coordinator.
                    local_input.pointer.pos = None;
                    local_input.pointer.prev_pos = None;
                }
            }
            region.coordinator.begin_frame(local_input);
        }
    }

    /// Register widget for this frame on main layer
    pub fn register(&mut self, id: impl Into<WidgetId>, rect: Rect, sense: Sense) {
        self.register_on_layer(id, rect, sense, &LayerId::main());
    }

    /// Register on specific layer
    pub fn register_on_layer(
        &mut self,
        id: impl Into<WidgetId>,
        rect: Rect,
        sense: Sense,
        layer: &LayerId,
    ) {
        self.widgets.push(RegisteredWidget {
            id: id.into(),
            rect,
            sense,
            layer: layer.clone(),
        });
    }

    /// Push a new layer (for modals/popups)
    pub fn push_layer(&mut self, id: LayerId, z_order: u32, modal: bool) {
        self.layers.push(Layer { id, z_order, modal });
    }

    /// Pop a layer — no-op, layers persist until begin_frame clears them.
    ///
    /// Layers must remain alive for click handling after render completes.
    /// They will be cleared on the next begin_frame().
    pub fn pop_layer(&mut self, _id: &LayerId) {
        // No-op: layers accumulate during render and are cleared in begin_frame
    }

    // -------------------------------------------------------------------------
    // Scoped-region management
    // -------------------------------------------------------------------------

    /// Register a scoped region and return a mutable reference to its child
    /// `InputCoordinator` so the caller can immediately register widgets on it.
    ///
    /// If a region with the same `id` already exists its coordinator is reused
    /// (and its bounding rect is updated), so persistent widget state is kept
    /// across frames.
    ///
    /// # Example
    /// ```ignore
    /// let child = coord.push_scoped_region("chart_toolbar", toolbar_rect);
    /// child.register("btn_zoom_in", zoom_btn_rect, Sense::CLICK);
    /// ```
    pub fn push_scoped_region(&mut self, id: &str, rect: Rect) -> &mut InputCoordinator {
        // Check if a region with this id already exists.
        if let Some(pos) = self.scoped_regions.iter().position(|r| r.id == id) {
            self.scoped_regions[pos].rect = rect;
            return &mut self.scoped_regions[pos].coordinator;
        }
        self.scoped_regions.push(ScopedRegion::new(id, rect));
        let last = self.scoped_regions.len() - 1;
        &mut self.scoped_regions[last].coordinator
    }

    /// Remove a scoped region by id.  A no-op if no region with that id exists.
    pub fn remove_scoped_region(&mut self, id: &str) {
        self.scoped_regions.retain(|r| r.id != id);
    }

    /// Return an immutable reference to a scoped region by id.
    pub fn scoped_region(&self, id: &str) -> Option<&ScopedRegion> {
        self.scoped_regions.iter().find(|r| r.id == id)
    }

    /// Return a mutable reference to a scoped region's child coordinator by id.
    pub fn scoped_region_coordinator_mut(&mut self, id: &str) -> Option<&mut InputCoordinator> {
        self.scoped_regions.iter_mut()
            .find(|r| r.id == id)
            .map(|r| &mut r.coordinator)
    }

    /// Process all registered widgets against current input state.
    ///
    /// Returns responses for widgets that had interactions.  Responses from
    /// scoped regions are included first with their widget IDs prefixed by
    /// `"{region_id}:"`.
    pub fn end_frame(&mut self) -> Vec<(WidgetId, WidgetResponse)> {
        let mut responses = Vec::new();

        // Collect responses from scoped regions first (highest Z = last region).
        // We iterate in order so the caller receives them before global widgets.
        // Widget IDs are prefixed so the caller can route them to the right panel.
        for region in &mut self.scoped_regions {
            let region_id = region.id.clone();
            let child_responses = region.coordinator.end_frame();
            for (wid, resp) in child_responses {
                let prefixed_id = WidgetId::new(format!("{}:{}", region_id, wid.0));
                // Rebuild response with prefixed id (keep all other fields).
                let mut prefixed_resp = resp;
                prefixed_resp.id = prefixed_id.clone();
                responses.push((prefixed_id, prefixed_resp));
            }
        }

        let mouse_pos = self.input.pointer.pos;
        let clicked = self.input.pointer.clicked;
        let button_down = self.input.pointer.button_down;

        // 1. Determine hovered widget (Z-order aware hit test)
        let hovered_id = if let Some((mx, my)) = mouse_pos {
            self.hit_test_at(mx, my).map(|w| w.id.clone())
        } else {
            None
        };

        // Track if we already generated a response for drag start
        let mut drag_started_this_frame = false;

        // 2. Update hover state for all widgets (except those being dragged)
        for widget in &self.widgets {
            // Skip if this widget is currently being dragged (handled in section 3)
            if self.widget_state.drag.dragging.as_ref() == Some(&widget.id) {
                continue;
            }

            let is_hovered = hovered_id.as_ref() == Some(&widget.id);
            let was_hovered = self.widget_state.hover.is_hovered(&widget.id);

            if (widget.sense.hover || widget.sense.click || widget.sense.drag)
                && (is_hovered || was_hovered) {
                    let mut response = WidgetResponse::new(widget.id.clone(), widget.rect, widget.sense);
                    response.hovered = is_hovered;
                    response.hover_started = is_hovered && !was_hovered;
                    response.hover_ended = !is_hovered && was_hovered;

                    // Check click
                    if is_hovered && clicked.is_some() && widget.sense.click {
                        response.clicked = true;
                        if widget.sense.text {
                            self.text_fields.focus(widget.id.clone());
                        }
                    }

                    // Check drag start
                    if is_hovered && button_down.is_some() && widget.sense.drag
                        && self.widget_state.drag.dragging.is_none() {
                            response.drag_started = true;
                            drag_started_this_frame = true;
                        }

                    if response.clicked
                        || response.hovered
                        || response.hover_started
                        || response.hover_ended
                        || response.drag_started
                    {
                        responses.push((widget.id.clone(), response));
                    }
                }
        }

        // 3. Handle ongoing drag (but not if we just started it this frame)
        if !drag_started_this_frame {
            if let Some(drag_id) = self.widget_state.drag.dragging.clone() {
                if button_down.is_some() {
                    // Drag continues
                    if mouse_pos.is_some() {
                        if let Some(widget) = self.widgets.iter().find(|w| w.id == drag_id) {
                            let mut response = WidgetResponse::new(drag_id.clone(), widget.rect, widget.sense);
                            response.dragged = true;
                            response.drag_delta = self.widget_state.drag.delta();
                            responses.push((drag_id.clone(), response));
                        }
                    }
                } else {
                    // Drag ended (button released)
                    if let Some(widget) = self.widgets.iter().find(|w| w.id == drag_id) {
                        let mut response = WidgetResponse::new(drag_id.clone(), widget.rect, widget.sense);
                        response.drag_stopped = true;
                        responses.push((drag_id.clone(), response));
                        self.widget_state.drag.end();
                    }
                }
            }
        }

        // 4. Handle scroll — route wheel delta to scroll-sensitive widgets
        let (scroll_dx, scroll_dy) = self.input.scroll_delta;
        if scroll_dx != 0.0 || scroll_dy != 0.0 {
            if let Some(hovered) = &hovered_id {
                if let Some(widget) = self.widgets.iter().find(|w| &w.id == hovered) {
                    if widget.sense.scroll {
                        let mut response = WidgetResponse::new(widget.id.clone(), widget.rect, widget.sense);
                        response.scrolled = true;
                        response.scroll_delta = (scroll_dx, scroll_dy);
                        responses.push((widget.id.clone(), response));
                    }
                }
            }
        }

        // 5. Update persistent state
        self.widget_state.hover.set_hovered(hovered_id);

        responses
    }

    /// Hit test at point (Z-order aware)
    fn hit_test_at(&self, x: f64, y: f64) -> Option<&RegisteredWidget> {
        // Sort layers by z-order (highest first)
        let mut sorted_layers = self.layers.clone();
        sorted_layers.sort_by(|a, b| b.z_order.cmp(&a.z_order));

        for layer in &sorted_layers {
            // Find widgets in this layer that contain the point
            let hits: Vec<_> = self
                .widgets
                .iter()
                .filter(|w| w.layer == layer.id && w.rect.contains(x, y))
                .collect();

            if let Some(widget) = hits.last() {
                // last registered = on top within layer
                return Some(widget);
            }

            // If this layer is modal, don't check lower layers
            if layer.modal {
                return None;
            }
        }
        None
    }

    /// Check if widget is hovered.
    ///
    /// Widget IDs from scoped regions must be prefixed (e.g. `"chart:btn_zoom"`).
    /// Unprefixed IDs are checked against the global coordinator only.
    pub fn is_hovered(&self, id: &WidgetId) -> bool {
        // Parse region prefix: "region_id:widget_id"
        if let Some(colon_pos) = id.0.find(':') {
            let region_id = &id.0[..colon_pos];
            let widget_part = &id.0[colon_pos + 1..];
            if let Some(region) = self.scoped_regions.iter().find(|r| r.id == region_id) {
                return region.coordinator.is_hovered(&WidgetId::new(widget_part));
            }
        }
        self.widget_state.hover.is_hovered(id)
    }

    /// Check if widget is focused
    pub fn is_focused(&self, id: &WidgetId) -> bool {
        self.widget_state.focus.is_focused(id)
    }

    /// Check if widget is being dragged
    pub fn is_dragging(&self, id: &WidgetId) -> bool {
        self.widget_state.drag.is_dragging(id)
    }

    /// Get the currently hovered widget.
    ///
    /// Checks scoped regions in reverse registration order before the global
    /// coordinator.  Returns the global hovered widget if no scoped region has
    /// a hovered widget.
    ///
    /// Note: the returned reference points into the child coordinator's state
    /// for scoped-region widgets.  The ID will **not** carry the region prefix
    /// from this method; use `is_hovered` with the prefixed ID for reliable
    /// cross-region checks.
    pub fn hovered_widget(&self) -> Option<&WidgetId> {
        // Check scoped regions (last = top-most).
        for region in self.scoped_regions.iter().rev() {
            if let Some(hovered) = region.coordinator.hovered_widget() {
                return Some(hovered);
            }
        }
        self.widget_state.hover.hovered.as_ref()
    }

    /// Returns the z_order of the layer that the currently hovered widget belongs to.
    ///
    /// Returns `None` if no widget is hovered (e.g. the cursor is over the chart
    /// canvas, which is not registered as a widget).
    pub fn hovered_widget_z_order(&self) -> Option<u32> {
        let (mx, my) = self.input.pointer.pos?;
        let widget = self.hit_test_at(mx, my)?;
        self.layers.iter()
            .find(|l| l.id == widget.layer)
            .map(|l| l.z_order)
    }

    /// Returns the `LayerId` of the currently hovered widget's layer.
    ///
    /// Returns `None` if no widget is hovered.
    pub fn hovered_widget_layer_id(&self) -> Option<LayerId> {
        let (mx, my) = self.input.pointer.pos?;
        let widget = self.hit_test_at(mx, my)?;
        Some(widget.layer.clone())
    }

    /// Returns `true` if the cursor is over any registered UI widget.
    ///
    /// Because the chart canvas is **not** registered as a widget, this method
    /// returns `false` when the cursor is over chart canvas and `true` when it
    /// is over any UI element (button, panel, modal, etc.).  This provides a
    /// single, authoritative check instead of many hardcoded widget-id tests.
    ///
    /// Backdrop widgets (IDs ending with `:bg`) are intentionally excluded.
    /// Returns true when the pointer is over any registered UI widget.
    ///
    /// Since the chart canvas is NOT registered as a widget (only UI elements
    /// are), this effectively means "cursor is on UI, not on chart".
    pub fn is_over_ui(&self) -> bool {
        self.hovered_widget().is_some()
    }

    /// Get focused widget
    pub fn focused_widget(&self) -> Option<&WidgetId> {
        self.widget_state.focus.focused.as_ref()
    }

    /// Get the rect of a registered widget (from last frame)
    pub fn widget_rect(&self, id: &WidgetId) -> Option<Rect> {
        self.widgets.iter().find(|w| &w.id == id).map(|w| w.rect)
    }

    /// Set focus to a specific widget.
    ///
    /// If the target is not a registered text field, any focused text field is blurred.
    pub fn set_focus(&mut self, id: impl Into<WidgetId>) {
        let id = id.into();
        if !self.text_fields.has_field(&id) {
            self.text_fields.blur();
        }
        self.widget_state.focus.set_focus(id);
    }

    /// Clear focus from all widgets
    pub fn clear_focus(&mut self) {
        self.widget_state.focus.clear_focus();
        self.text_fields.blur();
    }

    /// Register a widget as a text field on the main layer.
    ///
    /// Registers with `Sense::TEXT_INPUT` and stores the text field config.
    pub fn register_text_field(&mut self, id: impl Into<WidgetId>, rect: Rect, config: TextFieldConfig) {
        let id = id.into();
        self.register_on_layer(id.clone(), rect, Sense::TEXT_INPUT, &LayerId::main());
        self.text_fields.register(id, config);
    }

    /// Unregister a widget's text field state (if any).
    pub fn unregister(&mut self, id: &WidgetId) {
        self.text_fields.unregister(id);
    }

    /// Focus a text field and sync widget focus.
    ///
    /// Returns `true` if the field exists and focus was applied.
    pub fn focus_text_field(&mut self, id: &WidgetId) -> bool {
        if self.text_fields.focus(id.clone()) {
            self.widget_state.focus.set_focus(id.clone());
            true
        } else {
            false
        }
    }

    /// Forward a printable character to the focused text field.
    pub fn on_char(&mut self, ch: char) -> TextAction {
        self.text_fields.on_char(ch)
    }

    /// Forward a named key press to the focused text field.
    pub fn on_key(&mut self, key: super::keyboard::KeyPress) -> TextAction {
        self.text_fields.on_key(key)
    }

    /// Read-only access to the text field store.
    pub fn text_fields(&self) -> &TextFieldStore {
        &self.text_fields
    }

    /// Mutable access to the text field store.
    pub fn text_fields_mut(&mut self) -> &mut TextFieldStore {
        &mut self.text_fields
    }

    /// Focus next widget (Tab)
    pub fn focus_next(&mut self) {
        let focusable: Vec<_> = self.widgets.iter().filter(|w| w.sense.focus).collect();

        if focusable.is_empty() {
            return;
        }

        let current_idx = if let Some(ref focused) = self.widget_state.focus.focused {
            focusable.iter().position(|w| &w.id == focused)
        } else {
            None
        };

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % focusable.len(),
            None => 0,
        };

        self.widget_state
            .focus
            .set_focus(focusable[next_idx].id.clone());
    }

    /// Focus previous widget (Shift+Tab)
    pub fn focus_prev(&mut self) {
        let focusable: Vec<_> = self.widgets.iter().filter(|w| w.sense.focus).collect();

        if focusable.is_empty() {
            return;
        }

        let current_idx = if let Some(ref focused) = self.widget_state.focus.focused {
            focusable.iter().position(|w| &w.id == focused)
        } else {
            None
        };

        let prev_idx = match current_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) => focusable.len() - 1,
            None => focusable.len() - 1,
        };

        self.widget_state
            .focus
            .set_focus(focusable[prev_idx].id.clone());
    }

    /// Process a click at `(x, y)` against registered widgets.
    ///
    /// Returns the top-most widget ID that contains the point (Z-order + modal
    /// aware).  Call this from an `on_click()` handler — widgets are already
    /// registered from the last render frame.
    ///
    /// Scoped regions are checked in **reverse registration order** (last = highest
    /// Z) before falling through to the global coordinator.  If a point lands
    /// inside a scoped region but no widget inside it is hit, the search falls
    /// through to the global coordinator so chart-canvas clicks are not swallowed.
    ///
    /// If a global modal layer is active, scoped regions are **skipped** (the
    /// modal blocks everything below it).
    pub fn process_click(&self, x: f64, y: f64) -> Option<WidgetId> {
        // If a modal is active in the global coordinator, skip scoped regions.
        let has_modal = self.layers.iter().any(|l| l.modal);

        if !has_modal {
            // Check scoped regions in reverse order (last registered = top).
            for region in self.scoped_regions.iter().rev() {
                if region.contains(x, y) {
                    let (lx, ly) = region.to_local(x, y);
                    if let Some(local_id) = region.coordinator.process_click(lx, ly) {
                        // Found a widget inside the region — return prefixed id.
                        return Some(region.prefix_id(&local_id));
                    }
                    // Point inside region but no widget hit → fall through.
                }
            }
        }

        // Fall back to global hit test.
        self.hit_test_at(x, y).map(|w| w.id.clone())
    }

    /// Check if a point is inside any modal layer's registered area.
    /// Returns true if the point hits a modal layer but misses all widgets on it.
    /// This is useful for "click outside modal content to close" behavior.
    pub fn is_point_in_modal_layer(&self, x: f64, y: f64) -> bool {
        // Check if highest modal layer exists and point is NOT on any widget
        let hit = self.hit_test_at(x, y);
        let has_modal = self.layers.iter().any(|l| l.modal);
        has_modal && hit.is_none()
    }

    /// Check if a point is blocked from reaching non-modal layers.
    /// Returns true if any modal layer is active AND the point is either:
    /// - On a widget registered to the modal layer or above, OR
    /// - In the modal's blocking zone (no widget hit, modal blocks lower layers)
    ///
    /// Use this for drag blocking: when a modal is open, drags on/above the modal
    /// should not pass through to panel separators/headers on lower layers.
    pub fn is_blocked_by_modal(&self, x: f64, y: f64) -> bool {
        let modal_z = self.layers.iter()
            .filter(|l| l.modal)
            .map(|l| l.z_order)
            .min();
        let Some(modal_z) = modal_z else { return false };

        match self.hit_test_at(x, y) {
            None => true,
            Some(widget) => {
                self.layers.iter()
                    .find(|l| l.id == widget.layer)
                    .map(|l| l.z_order >= modal_z)
                    .unwrap_or(false)
            }
        }
    }

    /// Get the topmost modal layer ID (if any active)
    pub fn topmost_modal_layer(&self) -> Option<&LayerId> {
        self.layers.iter().rev().find(|l| l.modal).map(|l| &l.id)
    }
}

impl Default for InputCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::MouseButton;

    fn make_coordinator() -> InputCoordinator {
        InputCoordinator::new()
    }

    fn make_input_at(x: f64, y: f64) -> InputState {
        let mut input = InputState::default();
        input.pointer.pos = Some((x, y));
        input
    }

    fn make_click_at(x: f64, y: f64) -> InputState {
        let mut input = InputState::default();
        input.pointer.pos = Some((x, y));
        input.pointer.clicked = Some(MouseButton::Left);
        input
    }

    #[test]
    fn test_register_and_hit_test() {
        let mut coord = make_coordinator();
        let input = make_input_at(50.0, 30.0);
        coord.begin_frame(input);

        coord.register("button1", Rect::new(10.0, 10.0, 100.0, 40.0), Sense::CLICK);

        let hit = coord.hit_test_at(50.0, 30.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, WidgetId::new("button1"));

        let miss = coord.hit_test_at(5.0, 5.0);
        assert!(miss.is_none());
    }

    #[test]
    fn test_click_detection() {
        let mut coord = make_coordinator();
        let input = make_click_at(50.0, 30.0);
        coord.begin_frame(input);

        coord.register("button", Rect::new(10.0, 10.0, 100.0, 40.0), Sense::CLICK);

        let responses = coord.end_frame();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].0, WidgetId::new("button"));
        assert!(responses[0].1.clicked);
    }

    #[test]
    fn test_hover_start_end() {
        let mut coord = make_coordinator();

        // Frame 1: hover over button
        let input1 = make_input_at(50.0, 30.0);
        coord.begin_frame(input1);
        coord.register("button", Rect::new(10.0, 10.0, 100.0, 40.0), Sense::HOVER);
        let responses1 = coord.end_frame();

        assert_eq!(responses1.len(), 1);
        assert!(responses1[0].1.hover_started);

        // Frame 2: move off button
        let input2 = make_input_at(5.0, 5.0);
        coord.begin_frame(input2);
        coord.register("button", Rect::new(10.0, 10.0, 100.0, 40.0), Sense::HOVER);
        let responses2 = coord.end_frame();

        assert_eq!(responses2.len(), 1);
        assert!(responses2[0].1.hover_ended);
    }

    #[test]
    fn test_z_order_layers() {
        let mut coord = make_coordinator();
        let input = make_input_at(40.0, 40.0);
        coord.begin_frame(input);

        // Main layer button
        coord.register(
            "main_button",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Sense::CLICK,
        );

        // Modal layer button (overlapping)
        coord.push_layer(LayerId::new("modal"), 1, false);
        coord.register_on_layer(
            "modal_button",
            Rect::new(25.0, 25.0, 50.0, 50.0),
            Sense::CLICK,
            &LayerId::new("modal"),
        );

        // Hit test in overlap area - should hit modal button (higher z-order)
        let hit = coord.hit_test_at(40.0, 40.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().id, WidgetId::new("modal_button"));
    }

    #[test]
    fn test_modal_blocks_lower_layers() {
        let mut coord = make_coordinator();
        let input = make_input_at(50.0, 50.0);
        coord.begin_frame(input);

        coord.register(
            "main_button",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Sense::CLICK,
        );

        coord.push_layer(LayerId::new("modal"), 1, true); // modal=true
        coord.register_on_layer(
            "modal_button",
            Rect::new(200.0, 200.0, 50.0, 50.0),
            Sense::CLICK,
            &LayerId::new("modal"),
        );

        // Click on main area (outside modal widget) - should miss (blocked)
        let hit = coord.hit_test_at(50.0, 50.0);
        assert!(hit.is_none()); // Modal layer blocks
    }

    #[test]
    fn test_focus_management() {
        let mut coord = make_coordinator();
        let input = make_input_at(0.0, 0.0);
        coord.begin_frame(input);

        coord.register("input1", Rect::new(0.0, 0.0, 100.0, 30.0), Sense::FOCUSABLE);
        coord.register("input2", Rect::new(0.0, 40.0, 100.0, 30.0), Sense::FOCUSABLE);
        coord.register("input3", Rect::new(0.0, 80.0, 100.0, 30.0), Sense::FOCUSABLE);

        // Initially no focus
        assert!(coord.focused_widget().is_none());

        // Tab focuses first
        coord.focus_next();
        assert_eq!(
            coord.focused_widget(),
            Some(&WidgetId::new("input1"))
        );

        // Tab again focuses second
        coord.focus_next();
        assert_eq!(
            coord.focused_widget(),
            Some(&WidgetId::new("input2"))
        );

        // Shift+Tab goes back
        coord.focus_prev();
        assert_eq!(
            coord.focused_widget(),
            Some(&WidgetId::new("input1"))
        );

        // Shift+Tab wraps to last
        coord.focus_prev();
        assert_eq!(
            coord.focused_widget(),
            Some(&WidgetId::new("input3"))
        );
    }

    #[test]
    fn test_drag_start_continue_end() {
        let mut coord = make_coordinator();

        // Frame 1: pointer down on slider (drag start)
        let mut input1 = make_input_at(50.0, 20.0);
        input1.pointer.button_down = Some(MouseButton::Left);
        coord.begin_frame(input1);
        coord.register("slider", Rect::new(10.0, 10.0, 200.0, 20.0), Sense::DRAG);
        let responses1 = coord.end_frame();
        // Should get hover_started or drag_started response
        assert!(responses1.len() >= 1);

        // Manually set drag state for subsequent frames (simulating drag in progress)
        coord.widget_state.start_drag(WidgetId::new("slider"), 50.0, 20.0);

        // Frame 2: pointer moves while held down (drag continues)
        let mut input2 = make_input_at(100.0, 20.0);
        input2.pointer.button_down = Some(MouseButton::Left);
        coord.begin_frame(input2);
        coord.register("slider", Rect::new(10.0, 10.0, 200.0, 20.0), Sense::DRAG);
        coord.widget_state.drag.update(100.0, 20.0);
        let responses2 = coord.end_frame();
        assert_eq!(responses2.len(), 1);
        assert!(responses2[0].1.dragged);

        // Frame 3: pointer released (drag end)
        let input3 = make_input_at(100.0, 20.0);
        coord.begin_frame(input3);
        coord.register("slider", Rect::new(10.0, 10.0, 200.0, 20.0), Sense::DRAG);
        let responses3 = coord.end_frame();
        assert_eq!(responses3.len(), 1);
        assert!(responses3[0].1.drag_stopped);
    }

    #[test]
    fn test_no_click_on_drag_only_widget() {
        let mut coord = make_coordinator();
        let input = make_click_at(50.0, 20.0);
        coord.begin_frame(input);

        coord.register("slider", Rect::new(10.0, 10.0, 200.0, 20.0), Sense::DRAG);

        let responses = coord.end_frame();
        // Should not have clicked flag (only has drag sense)
        for (_id, response) in responses {
            assert!(!response.clicked);
        }
    }

    #[test]
    fn test_process_click() {
        let mut coord = make_coordinator();
        let input = make_input_at(50.0, 30.0);
        coord.begin_frame(input);
        coord.register("btn", Rect::new(10.0, 10.0, 100.0, 40.0), Sense::CLICK);

        assert_eq!(coord.process_click(50.0, 30.0), Some(WidgetId::new("btn")));
        assert_eq!(coord.process_click(5.0, 5.0), None);
    }

    #[test]
    fn test_modal_layer_blocks_and_process_click() {
        let mut coord = make_coordinator();
        let input = make_input_at(50.0, 50.0);
        coord.begin_frame(input);

        // Main layer button
        coord.register("main_btn", Rect::new(0.0, 0.0, 200.0, 200.0), Sense::CLICK);

        // Modal layer with button in corner
        coord.push_layer(LayerId::new("modal"), 1, true);
        coord.register_on_layer("modal_btn", Rect::new(300.0, 300.0, 50.0, 50.0), Sense::CLICK, &LayerId::new("modal"));

        // Click on main area — blocked by modal
        assert_eq!(coord.process_click(50.0, 50.0), None);
        // Click on modal button — works
        assert_eq!(coord.process_click(325.0, 325.0), Some(WidgetId::new("modal_btn")));
        // Point in modal layer but not on widget
        assert!(coord.is_point_in_modal_layer(150.0, 150.0));
    }

    #[test]
    fn test_text_field_integration() {
        let mut coord = make_coordinator();
        let input = make_click_at(50.0, 30.0);
        coord.begin_frame(input);

        let id = WidgetId::new("search_field");
        coord.register_text_field(id.clone(), Rect::new(10.0, 10.0, 100.0, 40.0), TextFieldConfig::text());

        let responses = coord.end_frame();
        assert!(responses.iter().any(|(wid, r)| wid == &id && r.clicked));

        // Text field should be focused after click
        assert!(coord.text_fields().is_focused(&id));
    }

    #[test]
    fn test_unified_focus() {
        let mut coord = make_coordinator();
        let input = make_input_at(0.0, 0.0);
        coord.begin_frame(input);

        let text_id = WidgetId::new("text1");
        coord.register_text_field(text_id.clone(), Rect::new(0.0, 0.0, 100.0, 30.0), TextFieldConfig::text());

        // Focus text field via unified method
        coord.focus_text_field(&text_id);
        assert!(coord.text_fields().is_focused(&text_id));
        assert!(coord.is_focused(&text_id));

        // Focus a non-text widget — text should blur
        let btn_id = WidgetId::new("button1");
        coord.register(btn_id.clone(), Rect::new(0.0, 40.0, 100.0, 30.0), Sense::CLICK);
        coord.set_focus(btn_id.clone());
        assert!(!coord.text_fields().is_focused(&text_id));
        assert!(coord.is_focused(&btn_id));
    }

    #[test]
    fn test_on_char_through_coordinator() {
        let mut coord = make_coordinator();
        let input = make_input_at(0.0, 0.0);
        coord.begin_frame(input);

        let id = WidgetId::new("field1");
        coord.register_text_field(id.clone(), Rect::new(0.0, 0.0, 100.0, 30.0), TextFieldConfig::text());
        coord.focus_text_field(&id);

        let action = coord.on_char('h');
        assert!(matches!(action, TextAction::TextChanged(_)));
        assert_eq!(coord.text_fields().text(&id), "h");
    }
}
