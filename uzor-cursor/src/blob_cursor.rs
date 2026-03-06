//! Blob cursor effect — trailing blobs follow cursor with different lag
//!
//! Algorithm (from React source using GSAP):
//! 1. Multiple blobs (default: 3) with different sizes
//! 2. Lead blob (index 0): fast transition (0.1s default)
//! 3. Trailing blobs: slow transition (0.5s default)
//! 4. Each blob smoothly interpolates to cursor position
//! 5. Gooey merge effect via SVG filter (blur + color matrix)

/// State of a single blob
#[derive(Debug, Clone, Copy)]
pub struct BlobState {
    /// Current X position
    pub x: f32,
    /// Current Y position
    pub y: f32,
    /// Current X velocity (for smooth interpolation)
    pub vx: f32,
    /// Current Y velocity (for smooth interpolation)
    pub vy: f32,
    /// Blob size (diameter in pixels)
    pub size: f32,
    /// Blob opacity (0.0..1.0)
    pub opacity: f32,
}

/// Blob cursor effect state
#[derive(Debug, Clone)]
pub struct BlobCursorState {
    /// All blob states
    pub blobs: Vec<BlobState>,
}

impl Default for BlobCursorState {
    fn default() -> Self {
        Self { blobs: Vec::new() }
    }
}

/// Blob shape type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobType {
    Circle,
    Square,
}

/// Blob cursor effect configuration
pub struct BlobCursor {
    /// Number of trailing blobs (default: 3)
    pub count: usize,
    /// Blob sizes in pixels (default: [60, 125, 75])
    pub sizes: Vec<f32>,
    /// Blob opacities (default: [0.6, 0.6, 0.6])
    pub opacities: Vec<f32>,
    /// Blob shape (default: Circle)
    pub blob_type: BlobType,
    /// Fast transition duration for lead blob in seconds (default: 0.1)
    pub fast_duration: f64,
    /// Slow transition duration for trailing blobs in seconds (default: 0.5)
    pub slow_duration: f64,
    /// Inner dot sizes in pixels (default: [20, 35, 25])
    pub inner_sizes: Vec<f32>,
}

impl Default for BlobCursor {
    fn default() -> Self {
        Self {
            count: 3,
            sizes: vec![60.0, 125.0, 75.0],
            opacities: vec![0.6, 0.6, 0.6],
            blob_type: BlobType::Circle,
            fast_duration: 0.1,
            slow_duration: 0.5,
            inner_sizes: vec![20.0, 35.0, 25.0],
        }
    }
}

impl BlobCursor {
    /// Create a new blob cursor with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the blob count
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Set the blob sizes
    pub fn with_sizes(mut self, sizes: Vec<f32>) -> Self {
        self.sizes = sizes;
        self
    }

    /// Set the blob opacities
    pub fn with_opacities(mut self, opacities: Vec<f32>) -> Self {
        self.opacities = opacities;
        self
    }

    /// Set the blob type
    pub fn with_blob_type(mut self, blob_type: BlobType) -> Self {
        self.blob_type = blob_type;
        self
    }

    /// Set the transition durations
    pub fn with_durations(mut self, fast: f64, slow: f64) -> Self {
        self.fast_duration = fast;
        self.slow_duration = slow;
        self
    }

    /// Set the inner dot sizes
    pub fn with_inner_sizes(mut self, inner_sizes: Vec<f32>) -> Self {
        self.inner_sizes = inner_sizes;
        self
    }

    /// Initialize the state with default blob positions
    pub fn init_state(&self, initial_x: f32, initial_y: f32) -> BlobCursorState {
        let blobs = (0..self.count)
            .map(|i| BlobState {
                x: initial_x,
                y: initial_y,
                vx: 0.0,
                vy: 0.0,
                size: self.sizes.get(i).copied().unwrap_or(60.0),
                opacity: self.opacities.get(i).copied().unwrap_or(0.6),
            })
            .collect();

        BlobCursorState { blobs }
    }

    /// Update blob positions towards cursor using smooth exponential decay
    ///
    /// This simulates GSAP's ease-out behavior using exponential smoothing:
    /// new_pos = current_pos + (target_pos - current_pos) * factor
    ///
    /// # Arguments
    /// * `state` - Current state (will be mutated)
    /// * `cursor_x` - Target cursor X position
    /// * `cursor_y` - Target cursor Y position
    /// * `dt` - Delta time since last update in seconds
    pub fn update(&self, state: &mut BlobCursorState, cursor_x: f32, cursor_y: f32, dt: f64) {
        for (i, blob) in state.blobs.iter_mut().enumerate() {
            let is_lead = i == 0;

            // Choose duration based on whether this is the lead blob
            let duration = if is_lead {
                self.fast_duration
            } else {
                self.slow_duration
            };

            // Calculate smoothing factor
            // factor = 1 - e^(-dt / (duration / 4))
            // The /4 is to match GSAP's power3.out / power1.out feel
            let decay_rate = 4.0;
            let factor = 1.0 - (-dt / (duration / decay_rate)).exp();
            let factor = factor as f32;

            // Smooth interpolation towards cursor
            let dx = cursor_x - blob.x;
            let dy = cursor_y - blob.y;

            blob.x += dx * factor;
            blob.y += dy * factor;

            // Update velocity for potential future use
            blob.vx = dx * factor / dt as f32;
            blob.vy = dy * factor / dt as f32;
        }
    }
}

/// SVG filter parameters for gooey merge effect
#[derive(Debug, Clone)]
pub struct GooeyFilter {
    /// Gaussian blur standard deviation (default: 30.0)
    pub std_deviation: f32,
    /// Color matrix values for contrast boost
    /// Default: "1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 35 -10"
    pub color_matrix: String,
}

impl Default for GooeyFilter {
    fn default() -> Self {
        Self {
            std_deviation: 30.0,
            color_matrix: "1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 35 -10".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_cursor_init() {
        let cursor = BlobCursor::new().with_count(3);
        let state = cursor.init_state(100.0, 200.0);

        assert_eq!(state.blobs.len(), 3);

        for blob in &state.blobs {
            assert_eq!(blob.x, 100.0);
            assert_eq!(blob.y, 200.0);
            assert_eq!(blob.vx, 0.0);
            assert_eq!(blob.vy, 0.0);
        }
    }

    #[test]
    fn test_blob_sizes() {
        let cursor = BlobCursor::new().with_sizes(vec![50.0, 100.0, 75.0]);
        let state = cursor.init_state(0.0, 0.0);

        assert_eq!(state.blobs[0].size, 50.0);
        assert_eq!(state.blobs[1].size, 100.0);
        assert_eq!(state.blobs[2].size, 75.0);
    }

    #[test]
    fn test_blob_opacities() {
        let cursor = BlobCursor::new().with_opacities(vec![0.5, 0.7, 0.9]);
        let state = cursor.init_state(0.0, 0.0);

        assert_eq!(state.blobs[0].opacity, 0.5);
        assert_eq!(state.blobs[1].opacity, 0.7);
        assert_eq!(state.blobs[2].opacity, 0.9);
    }

    #[test]
    fn test_blob_update_moves_towards_cursor() {
        let cursor = BlobCursor::new().with_count(2);
        let mut state = cursor.init_state(0.0, 0.0);

        // Update towards (100, 100)
        cursor.update(&mut state, 100.0, 100.0, 0.016); // ~60fps

        // Blobs should have moved towards cursor
        assert!(state.blobs[0].x > 0.0);
        assert!(state.blobs[0].y > 0.0);
        assert!(state.blobs[0].x < 100.0); // But not all the way there
        assert!(state.blobs[0].y < 100.0);

        // Lead blob (index 0) should move faster than trailing blob
        let lead_dist = state.blobs[0].x * state.blobs[0].x + state.blobs[0].y * state.blobs[0].y;
        let trail_dist =
            state.blobs[1].x * state.blobs[1].x + state.blobs[1].y * state.blobs[1].y;
        assert!(lead_dist > trail_dist);
    }

    #[test]
    fn test_blob_convergence() {
        let cursor = BlobCursor::new().with_count(1);
        let mut state = cursor.init_state(0.0, 0.0);

        // Run many updates
        for _ in 0..1000 {
            cursor.update(&mut state, 100.0, 100.0, 0.016);
        }

        // Should converge very close to target
        assert!((state.blobs[0].x - 100.0).abs() < 0.1);
        assert!((state.blobs[0].y - 100.0).abs() < 0.1);
    }
}
