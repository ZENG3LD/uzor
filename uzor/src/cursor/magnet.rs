//! Magnet effect — elements are attracted to cursor within a radius
//!
//! Algorithm (from React source):
//! 1. Check if cursor is within element bounds + padding
//! 2. If within bounds: offset = (cursor - center) / strength
//! 3. If outside bounds: offset = (0, 0)
//! 4. Smooth transition via active/inactive transition timings

/// Magnet effect state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagnetState {
    /// Horizontal offset in pixels
    pub offset_x: f32,
    /// Vertical offset in pixels
    pub offset_y: f32,
    /// Whether the magnet is currently active (cursor is within bounds)
    pub is_active: bool,
}

impl Default for MagnetState {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            is_active: false,
        }
    }
}

/// Rectangle bounds
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn center(&self) -> (f32, f32) {
        (self.left + self.width / 2.0, self.top + self.height / 2.0)
    }
}

/// Magnet effect configuration and state management
pub struct Magnet {
    /// Activation radius around the element (default: 100.0)
    pub padding: f32,
    /// Strength divider — higher = weaker attraction (default: 2.0)
    pub strength: f32,
    /// Whether the effect is disabled
    pub disabled: bool,
}

impl Default for Magnet {
    fn default() -> Self {
        Self {
            padding: 100.0,
            strength: 2.0,
            disabled: false,
        }
    }
}

impl Magnet {
    /// Create a new magnet with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the activation padding/radius
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set the magnet strength (higher = weaker)
    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength;
        self
    }

    /// Set whether the effect is disabled
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Update the magnet state based on cursor position
    ///
    /// # Arguments
    /// * `cursor_x` - Cursor X position in screen coordinates
    /// * `cursor_y` - Cursor Y position in screen coordinates
    /// * `element_rect` - The element's bounding rectangle
    ///
    /// # Returns
    /// Updated magnet state with offset and active flag
    pub fn update(&self, cursor_x: f32, cursor_y: f32, element_rect: Rect) -> MagnetState {
        if self.disabled {
            return MagnetState::default();
        }

        let (center_x, center_y) = element_rect.center();
        let half_width = element_rect.width / 2.0;
        let half_height = element_rect.height / 2.0;

        // Check if cursor is within activation bounds (element + padding)
        let dist_x = (center_x - cursor_x).abs();
        let dist_y = (center_y - cursor_y).abs();

        let is_active = dist_x < half_width + self.padding && dist_y < half_height + self.padding;

        if is_active {
            // Calculate offset: cursor distance from center divided by strength
            let offset_x = (cursor_x - center_x) / self.strength;
            let offset_y = (cursor_y - center_y) / self.strength;
            MagnetState {
                offset_x,
                offset_y,
                is_active: true,
            }
        } else {
            MagnetState {
                offset_x: 0.0,
                offset_y: 0.0,
                is_active: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magnet_within_bounds() {
        let magnet = Magnet::new().with_strength(2.0).with_padding(100.0);

        let rect = Rect {
            left: 100.0,
            top: 100.0,
            width: 200.0,
            height: 200.0,
        };

        // Cursor at center (200, 200) — should be active with 0 offset
        let state = magnet.update(200.0, 200.0, rect);
        assert!(state.is_active);
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);

        // Cursor offset right and down (250, 250)
        // Center is (200, 200), offset = (50, 50) / 2 = (25, 25)
        let state = magnet.update(250.0, 250.0, rect);
        assert!(state.is_active);
        assert_eq!(state.offset_x, 25.0);
        assert_eq!(state.offset_y, 25.0);

        // Cursor offset left and up (150, 150)
        // Offset = (-50, -50) / 2 = (-25, -25)
        let state = magnet.update(150.0, 150.0, rect);
        assert!(state.is_active);
        assert_eq!(state.offset_x, -25.0);
        assert_eq!(state.offset_y, -25.0);
    }

    #[test]
    fn test_magnet_outside_bounds() {
        let magnet = Magnet::new().with_padding(50.0);

        let rect = Rect {
            left: 100.0,
            top: 100.0,
            width: 100.0,
            height: 100.0,
        };

        // Cursor far away (500, 500) — should not be active
        let state = magnet.update(500.0, 500.0, rect);
        assert!(!state.is_active);
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);
    }

    #[test]
    fn test_magnet_disabled() {
        let magnet = Magnet::new().with_disabled(true);

        let rect = Rect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Even with cursor at center, should be inactive when disabled
        let state = magnet.update(50.0, 50.0, rect);
        assert!(!state.is_active);
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);
    }

    #[test]
    fn test_magnet_strength() {
        let magnet_weak = Magnet::new().with_strength(4.0);
        let magnet_strong = Magnet::new().with_strength(1.0);

        let rect = Rect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Cursor at (60, 50), center at (50, 50), offset = 10 pixels
        let state_weak = magnet_weak.update(60.0, 50.0, rect);
        let state_strong = magnet_strong.update(60.0, 50.0, rect);

        // Weak magnet: 10 / 4 = 2.5
        assert_eq!(state_weak.offset_x, 2.5);

        // Strong magnet: 10 / 1 = 10
        assert_eq!(state_strong.offset_x, 10.0);
    }
}
