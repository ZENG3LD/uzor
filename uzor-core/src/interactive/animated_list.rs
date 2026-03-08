//! Animated list with stagger entry/exit animations
//!
//! Items fade in with slide-up animation on entry, and fade out
//! with slide-down + scale on exit. Stagger delay creates wave effect.

/// Animation state for a single list item
#[derive(Debug, Clone, Copy)]
pub struct ItemState {
    /// Opacity (0.0 = invisible, 1.0 = fully visible)
    pub opacity: f32,

    /// Vertical offset in pixels (positive = down)
    pub y_offset: f32,

    /// Scale factor (1.0 = normal size)
    pub scale: f32,
}

impl Default for ItemState {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            y_offset: 0.0,
            scale: 0.7,
        }
    }
}

impl ItemState {
    /// Create entry animation state (invisible, below, scaled down)
    pub fn entry_start() -> Self {
        Self {
            opacity: 0.0,
            y_offset: 20.0,
            scale: 0.7,
        }
    }

    /// Create visible state (fully opaque, no offset, normal scale)
    pub fn visible() -> Self {
        Self {
            opacity: 1.0,
            y_offset: 0.0,
            scale: 1.0,
        }
    }

    /// Create exit animation state (invisible, below, scaled down)
    pub fn exit_end() -> Self {
        Self {
            opacity: 0.0,
            y_offset: 20.0,
            scale: 0.7,
        }
    }

    /// Interpolate between two states
    pub fn lerp(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            opacity: from.opacity + (to.opacity - from.opacity) * t,
            y_offset: from.y_offset + (to.y_offset - from.y_offset) * t,
            scale: from.scale + (to.scale - from.scale) * t,
        }
    }
}

/// Animated list manager
///
/// Tracks animation state for multiple items with staggered entry/exit.
#[derive(Debug, Clone)]
pub struct AnimatedList {
    /// Number of items in the list
    item_count: usize,

    /// Animation states for each item
    states: Vec<ItemAnimationState>,

    /// Stagger delay between items (seconds)
    pub stagger_delay: f32,

    /// Animation duration per item (seconds)
    pub animation_duration: f32,
}

#[derive(Debug, Clone)]
struct ItemAnimationState {
    /// Current visual state
    current: ItemState,

    /// Animation progress (0.0 = start, 1.0 = complete)
    progress: f32,

    /// Animation type
    animation: AnimationType,

    /// Start time of animation
    start_time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimationType {
    None,
    Entry,
    Exit,
}

impl Default for AnimatedList {
    fn default() -> Self {
        Self::new(0)
    }
}

impl AnimatedList {
    /// Create a new animated list
    pub fn new(item_count: usize) -> Self {
        Self {
            item_count,
            states: vec![
                ItemAnimationState {
                    current: ItemState::entry_start(),
                    progress: 0.0,
                    animation: AnimationType::Entry,
                    start_time: 0.0,
                };
                item_count
            ],
            stagger_delay: 0.05,
            animation_duration: 0.2,
        }
    }

    /// Set stagger delay between items
    pub fn with_stagger_delay(mut self, delay: f32) -> Self {
        self.stagger_delay = delay;
        self
    }

    /// Set animation duration per item
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.animation_duration = duration;
        self
    }

    /// Update item count (triggers entry/exit animations as needed)
    pub fn set_item_count(&mut self, new_count: usize, current_time: f64) {
        if new_count > self.item_count {
            // Add new items with entry animation
            for i in self.item_count..new_count {
                self.states.push(ItemAnimationState {
                    current: ItemState::entry_start(),
                    progress: 0.0,
                    animation: AnimationType::Entry,
                    start_time: current_time + (i - self.item_count) as f64 * self.stagger_delay as f64,
                });
            }
        } else if new_count < self.item_count {
            // Mark removed items for exit animation
            for state in self.states.iter_mut().skip(new_count) {
                if state.animation != AnimationType::Exit {
                    state.animation = AnimationType::Exit;
                    state.progress = 0.0;
                    state.start_time = current_time;
                }
            }
        }

        self.item_count = new_count;
    }

    /// Update all animations
    pub fn update(&mut self, current_time: f64) {
        for (index, state) in self.states.iter_mut().enumerate() {
            if state.animation == AnimationType::None {
                continue;
            }

            let elapsed = (current_time - state.start_time) as f32;
            let stagger_offset = index as f32 * self.stagger_delay;

            // Calculate progress with stagger
            let effective_elapsed = (elapsed - stagger_offset).max(0.0);
            state.progress = (effective_elapsed / self.animation_duration).clamp(0.0, 1.0);

            // Apply easing (ease-out cubic)
            let eased_progress = 1.0 - (1.0 - state.progress).powi(3);

            // Update current state based on animation type
            match state.animation {
                AnimationType::Entry => {
                    state.current = ItemState::lerp(
                        ItemState::entry_start(),
                        ItemState::visible(),
                        eased_progress,
                    );

                    if state.progress >= 1.0 {
                        state.animation = AnimationType::None;
                        state.current = ItemState::visible();
                    }
                }
                AnimationType::Exit => {
                    state.current = ItemState::lerp(
                        ItemState::visible(),
                        ItemState::exit_end(),
                        eased_progress,
                    );
                }
                AnimationType::None => {}
            }
        }

        // Remove fully exited items
        self.states.retain(|state| {
            state.animation != AnimationType::Exit || state.progress < 1.0
        });
    }

    /// Get animation state for item at index
    pub fn get_item_state(&self, index: usize) -> Option<ItemState> {
        self.states.get(index).map(|s| s.current)
    }

    /// Get all item states
    pub fn item_states(&self) -> impl Iterator<Item = (usize, ItemState)> + '_ {
        self.states
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < self.item_count)
            .map(|(i, s)| (i, s.current))
    }

    /// Check if any animations are in progress
    pub fn is_animating(&self) -> bool {
        self.states.iter().any(|s| s.animation != AnimationType::None)
    }

    /// Trigger entry animation for all items
    pub fn animate_in(&mut self, current_time: f64) {
        for (index, state) in self.states.iter_mut().enumerate() {
            state.animation = AnimationType::Entry;
            state.progress = 0.0;
            state.start_time = current_time + index as f64 * self.stagger_delay as f64;
            state.current = ItemState::entry_start();
        }
    }

    /// Trigger exit animation for all items
    pub fn animate_out(&mut self, current_time: f64) {
        for (index, state) in self.states.iter_mut().enumerate() {
            state.animation = AnimationType::Exit;
            state.progress = 0.0;
            state.start_time = current_time + index as f64 * self.stagger_delay as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_state_lerp() {
        let start = ItemState::entry_start();
        let end = ItemState::visible();

        let mid = ItemState::lerp(start, end, 0.5);
        assert!((mid.opacity - 0.5).abs() < 0.01);
        assert!(mid.y_offset > 0.0 && mid.y_offset < 20.0);
        assert!(mid.scale > 0.7 && mid.scale < 1.0);
    }

    #[test]
    fn test_animated_list_creation() {
        let list = AnimatedList::new(5);
        assert_eq!(list.item_count, 5);
        assert_eq!(list.states.len(), 5);
    }

    #[test]
    fn test_entry_animation() {
        let mut list = AnimatedList::new(3);

        // At t=0, all items should be in entry state
        list.update(0.0);
        for i in 0..3 {
            let state = list.get_item_state(i).unwrap();
            assert!(state.opacity < 0.1); // Nearly invisible
        }

        // At t=0.1, first item should be partially visible
        list.update(0.1);
        let state0 = list.get_item_state(0).unwrap();
        assert!(state0.opacity > 0.0);

        // At t=1.0, all items should be fully visible
        list.update(1.0);
        for i in 0..3 {
            let state = list.get_item_state(i).unwrap();
            assert!((state.opacity - 1.0).abs() < 0.1);
            assert!(state.y_offset.abs() < 1.0);
        }
    }

    #[test]
    fn test_stagger_delay() {
        let mut list = AnimatedList::new(3).with_stagger_delay(0.1);

        list.update(0.0);

        // At t=0.05 (before stagger), second item should still be invisible
        list.update(0.05);
        let state1 = list.get_item_state(1).unwrap();
        assert!(state1.opacity < 0.1);

        // At t=0.15 (after stagger), second item should be animating
        list.update(0.15);
        let state1 = list.get_item_state(1).unwrap();
        assert!(state1.opacity > 0.1);
    }

    #[test]
    fn test_add_items() {
        let mut list = AnimatedList::new(2);
        list.update(1.0); // Complete initial animation

        // Add one more item
        list.set_item_count(3, 1.0);
        assert_eq!(list.states.len(), 3);

        // New item should start with entry animation
        list.update(1.0);
        let state2 = list.get_item_state(2).unwrap();
        assert!(state2.opacity < 1.0);
    }

    #[test]
    fn test_remove_items() {
        let mut list = AnimatedList::new(3);
        list.update(1.0); // Complete entry animations

        // Remove one item
        list.set_item_count(2, 1.0);

        // Third item should be animating out
        list.update(1.0);
        assert!(list.is_animating());

        // After exit animation completes, item should be removed
        list.update(2.0);
        assert_eq!(list.states.len(), 2);
    }

    #[test]
    fn test_animate_in_out() {
        let mut list = AnimatedList::new(2);

        // Animate in
        list.animate_in(0.0);
        assert!(list.is_animating());

        list.update(1.0);
        assert!(!list.is_animating());

        // Animate out
        list.animate_out(1.0);
        assert!(list.is_animating());

        // Check mid-exit animation (before items are removed)
        list.update(1.15);
        if let Some(state0) = list.get_item_state(0) {
            assert!(state0.opacity < 1.0); // Should be fading out
        }

        // After full exit animation, items should be removed
        list.update(2.0);
        assert_eq!(list.states.len(), 0);
    }
}
