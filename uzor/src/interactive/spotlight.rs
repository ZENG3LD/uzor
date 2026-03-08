//! Spotlight card effect - radial gradient following cursor
//!
//! Creates a spotlight effect that follows the mouse cursor within
//! card bounds. The spotlight is rendered as a radial gradient.

/// Spotlight card state
///
/// Tracks cursor position and computes spotlight parameters for rendering.
#[derive(Debug, Clone, Copy)]
pub struct SpotlightCard {
    /// Card width in pixels
    pub width: f32,

    /// Card height in pixels
    pub height: f32,

    /// Spotlight radius in pixels
    pub radius: f32,

    /// Current spotlight X position (relative to card, 0 = left edge)
    spotlight_x: f32,

    /// Current spotlight Y position (relative to card, 0 = top edge)
    spotlight_y: f32,

    /// Whether cursor is currently over the card
    is_active: bool,
}

impl Default for SpotlightCard {
    fn default() -> Self {
        Self::new(400.0, 300.0)
    }
}

impl SpotlightCard {
    /// Create a new spotlight card
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            radius: 200.0,
            spotlight_x: width / 2.0,
            spotlight_y: height / 2.0,
            is_active: false,
        }
    }

    /// Set spotlight radius
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Update card dimensions
    pub fn set_dimensions(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    /// Update cursor position
    ///
    /// # Arguments
    /// * `cursor_x` - Cursor X position in screen/window coordinates
    /// * `cursor_y` - Cursor Y position in screen/window coordinates
    /// * `card_x` - Card X position in screen/window coordinates
    /// * `card_y` - Card Y position in screen/window coordinates
    pub fn update_cursor(
        &mut self,
        cursor_x: f32,
        cursor_y: f32,
        card_x: f32,
        card_y: f32,
    ) {
        // Convert to card-relative coordinates
        self.spotlight_x = cursor_x - card_x;
        self.spotlight_y = cursor_y - card_y;

        // Check if cursor is within card bounds
        self.is_active = self.spotlight_x >= 0.0
            && self.spotlight_x <= self.width
            && self.spotlight_y >= 0.0
            && self.spotlight_y <= self.height;
    }

    /// Set cursor position directly in card-relative coordinates
    pub fn set_spotlight_position(&mut self, x: f32, y: f32) {
        self.spotlight_x = x;
        self.spotlight_y = y;

        self.is_active = x >= 0.0
            && x <= self.width
            && y >= 0.0
            && y <= self.height;
    }

    /// Get spotlight X position (card-relative)
    pub fn spotlight_x(&self) -> f32 {
        self.spotlight_x
    }

    /// Get spotlight Y position (card-relative)
    pub fn spotlight_y(&self) -> f32 {
        self.spotlight_y
    }

    /// Get spotlight center as (x, y) tuple
    pub fn spotlight_center(&self) -> (f32, f32) {
        (self.spotlight_x, self.spotlight_y)
    }

    /// Check if spotlight is active (cursor over card)
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Deactivate spotlight (e.g., when cursor leaves card)
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Get normalized spotlight position (0.0 to 1.0)
    pub fn normalized_position(&self) -> (f32, f32) {
        let x = if self.width > 0.0 {
            (self.spotlight_x / self.width).clamp(0.0, 1.0)
        } else {
            0.5
        };

        let y = if self.height > 0.0 {
            (self.spotlight_y / self.height).clamp(0.0, 1.0)
        } else {
            0.5
        };

        (x, y)
    }

    /// Get spotlight parameters for CSS radial-gradient
    ///
    /// Returns (x, y, radius, opacity) where:
    /// - x, y: position in pixels
    /// - radius: spotlight radius in pixels
    /// - opacity: 0.0 if inactive, 1.0 if active
    pub fn gradient_params(&self) -> (f32, f32, f32, f32) {
        let opacity = if self.is_active { 1.0 } else { 0.0 };
        (self.spotlight_x, self.spotlight_y, self.radius, opacity)
    }
}

/// Spotlight color configuration
#[derive(Debug, Clone, Copy)]
pub struct SpotlightColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Default for SpotlightColor {
    fn default() -> Self {
        Self::white(0.25)
    }
}

impl SpotlightColor {
    /// White spotlight with given opacity
    pub fn white(alpha: f32) -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: alpha,
        }
    }

    /// Create from RGBA values
    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to CSS rgba string
    pub fn to_css(&self) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }

    /// Get color components as (r, g, b, a)
    pub fn components(&self) -> (u8, u8, u8, f32) {
        (self.r, self.g, self.b, self.a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_creation() {
        let spotlight = SpotlightCard::new(400.0, 300.0);
        assert_eq!(spotlight.width, 400.0);
        assert_eq!(spotlight.height, 300.0);
        assert!(!spotlight.is_active());
    }

    #[test]
    fn test_cursor_update() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0);

        // Cursor at (150, 100) screen, card at (50, 50)
        // Spotlight should be at (100, 50) relative to card
        spotlight.update_cursor(150.0, 100.0, 50.0, 50.0);

        assert_eq!(spotlight.spotlight_x(), 100.0);
        assert_eq!(spotlight.spotlight_y(), 50.0);
        assert!(spotlight.is_active());
    }

    #[test]
    fn test_cursor_outside_bounds() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0);

        // Cursor outside card bounds (negative relative position)
        spotlight.update_cursor(10.0, 10.0, 50.0, 50.0);

        assert!(!spotlight.is_active());
    }

    #[test]
    fn test_direct_position_set() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0);

        spotlight.set_spotlight_position(200.0, 150.0);
        assert_eq!(spotlight.spotlight_center(), (200.0, 150.0));
        assert!(spotlight.is_active());

        // Set position outside bounds
        spotlight.set_spotlight_position(-10.0, 150.0);
        assert!(!spotlight.is_active());
    }

    #[test]
    fn test_normalized_position() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0);

        // Center of card
        spotlight.set_spotlight_position(200.0, 150.0);
        let (nx, ny) = spotlight.normalized_position();
        assert!((nx - 0.5).abs() < 0.01);
        assert!((ny - 0.5).abs() < 0.01);

        // Top-left corner
        spotlight.set_spotlight_position(0.0, 0.0);
        let (nx, ny) = spotlight.normalized_position();
        assert!(nx < 0.01);
        assert!(ny < 0.01);

        // Bottom-right corner
        spotlight.set_spotlight_position(400.0, 300.0);
        let (nx, ny) = spotlight.normalized_position();
        assert!((nx - 1.0).abs() < 0.01);
        assert!((ny - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_gradient_params() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0)
            .with_radius(150.0);

        spotlight.set_spotlight_position(200.0, 150.0);
        let (x, y, r, opacity) = spotlight.gradient_params();

        assert_eq!(x, 200.0);
        assert_eq!(y, 150.0);
        assert_eq!(r, 150.0);
        assert_eq!(opacity, 1.0);

        // Deactivate
        spotlight.deactivate();
        let (_, _, _, opacity) = spotlight.gradient_params();
        assert_eq!(opacity, 0.0);
    }

    #[test]
    fn test_spotlight_color() {
        let white = SpotlightColor::white(0.5);
        assert_eq!(white.r, 255);
        assert_eq!(white.g, 255);
        assert_eq!(white.b, 255);
        assert!((white.a - 0.5).abs() < 0.01);

        let custom = SpotlightColor::rgba(128, 64, 32, 0.75);
        assert_eq!(custom.r, 128);
        assert_eq!(custom.g, 64);
        assert_eq!(custom.b, 32);
        assert!((custom.a - 0.75).abs() < 0.01);

        let css = white.to_css();
        assert_eq!(css, "rgba(255, 255, 255, 0.5)");
    }

    #[test]
    fn test_dimensions_update() {
        let mut spotlight = SpotlightCard::new(400.0, 300.0);

        spotlight.set_dimensions(800.0, 600.0);
        assert_eq!(spotlight.width, 800.0);
        assert_eq!(spotlight.height, 600.0);
    }
}
