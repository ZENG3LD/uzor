//! Electric border animation with perlin-like noise displacement
//!
//! Animates a border path with randomized displacement along the perimeter,
//! creating an electric/lightning effect. Uses multi-octave noise for
//! organic-looking animation.

use std::f32::consts::PI;

/// Electric border animator
///
/// Generates displaced points along a rounded rectangle border path,
/// creating an animated electric effect using noise displacement.
#[derive(Debug, Clone)]
pub struct ElectricBorder {
    /// Border width in pixels
    pub width: f32,

    /// Border height in pixels
    pub height: f32,

    /// Border radius for rounded corners (pixels)
    pub border_radius: f32,

    /// Animation speed multiplier (1.0 = normal)
    pub speed: f32,

    /// Chaos level - displacement amplitude (0.0 = none, 0.2 = high)
    pub chaos: f32,

    /// Current animation time (seconds)
    time: f32,

    /// Number of sample points along border
    sample_count: usize,

    /// Displacement scale in pixels
    displacement: f32,
}

impl Default for ElectricBorder {
    fn default() -> Self {
        Self::new(400.0, 300.0)
    }
}

impl ElectricBorder {
    /// Create a new electric border
    pub fn new(width: f32, height: f32) -> Self {
        let perimeter = 2.0 * (width + height);
        let sample_count = (perimeter / 2.0) as usize;

        Self {
            width,
            height,
            border_radius: 24.0,
            speed: 1.0,
            chaos: 0.12,
            time: 0.0,
            sample_count,
            displacement: 60.0,
        }
    }

    /// Set border radius
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set animation speed
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Set chaos level (displacement amount)
    pub fn with_chaos(mut self, chaos: f32) -> Self {
        self.chaos = chaos;
        self
    }

    /// Update dimensions
    pub fn set_dimensions(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;

        // Recalculate sample count based on perimeter
        let perimeter = 2.0 * (width + height);
        self.sample_count = (perimeter / 2.0) as usize;
    }

    /// Update animation time
    pub fn update(&mut self, delta_time: f32) {
        self.time += delta_time * self.speed;
    }

    /// Set absolute time
    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    /// Get current animation time
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Generate displaced border points
    ///
    /// Returns a vector of (x, y) coordinates along the border path
    /// with noise-based displacement applied.
    pub fn generate_points(&self) -> Vec<(f32, f32)> {
        let mut points = Vec::with_capacity(self.sample_count + 1);

        let max_radius = (self.width.min(self.height) / 2.0).min(self.border_radius);
        let radius = max_radius.max(0.0);

        for i in 0..=self.sample_count {
            let t = i as f32 / self.sample_count as f32;

            // Get base point on rounded rectangle
            let (base_x, base_y) = self.get_rounded_rect_point(t, radius);

            // Apply noise displacement
            let (dx, dy) = self.get_displacement(t);

            points.push((base_x + dx, base_y + dy));
        }

        points
    }

    /// Get point on rounded rectangle perimeter at parameter t (0.0 to 1.0)
    fn get_rounded_rect_point(&self, t: f32, radius: f32) -> (f32, f32) {
        let straight_width = self.width - 2.0 * radius;
        let straight_height = self.height - 2.0 * radius;
        let corner_arc = (PI * radius) / 2.0;

        let total_perimeter =
            2.0 * straight_width + 2.0 * straight_height + 4.0 * corner_arc;

        let distance = t * total_perimeter;
        let mut accumulated = 0.0;

        // Top edge
        if distance <= accumulated + straight_width {
            let progress = (distance - accumulated) / straight_width;
            return (radius + progress * straight_width, 0.0);
        }
        accumulated += straight_width;

        // Top-right corner
        if distance <= accumulated + corner_arc {
            let progress = (distance - accumulated) / corner_arc;
            return self.get_corner_point(
                self.width - radius,
                radius,
                radius,
                -PI / 2.0,
                progress,
            );
        }
        accumulated += corner_arc;

        // Right edge
        if distance <= accumulated + straight_height {
            let progress = (distance - accumulated) / straight_height;
            return (self.width, radius + progress * straight_height);
        }
        accumulated += straight_height;

        // Bottom-right corner
        if distance <= accumulated + corner_arc {
            let progress = (distance - accumulated) / corner_arc;
            return self.get_corner_point(
                self.width - radius,
                self.height - radius,
                radius,
                0.0,
                progress,
            );
        }
        accumulated += corner_arc;

        // Bottom edge
        if distance <= accumulated + straight_width {
            let progress = (distance - accumulated) / straight_width;
            return (
                self.width - radius - progress * straight_width,
                self.height,
            );
        }
        accumulated += straight_width;

        // Bottom-left corner
        if distance <= accumulated + corner_arc {
            let progress = (distance - accumulated) / corner_arc;
            return self.get_corner_point(
                radius,
                self.height - radius,
                radius,
                PI / 2.0,
                progress,
            );
        }
        accumulated += corner_arc;

        // Left edge
        if distance <= accumulated + straight_height {
            let progress = (distance - accumulated) / straight_height;
            return (0.0, self.height - radius - progress * straight_height);
        }
        accumulated += straight_height;

        // Top-left corner (remaining)
        let progress = (distance - accumulated) / corner_arc;
        self.get_corner_point(radius, radius, radius, PI, progress)
    }

    /// Get point on circular arc
    fn get_corner_point(
        &self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        start_angle: f32,
        progress: f32,
    ) -> (f32, f32) {
        let angle = start_angle + progress * (PI / 2.0);
        (
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
        )
    }

    /// Get noise displacement at parameter t
    fn get_displacement(&self, t: f32) -> (f32, f32) {
        let octaves = 10;
        let lacunarity = 1.6;
        let gain = 0.7;
        let amplitude = self.chaos;
        let frequency = 10.0;

        let x_noise = self.octaved_noise(
            t * 8.0,
            octaves,
            lacunarity,
            gain,
            amplitude,
            frequency,
            self.time,
            0.0,
        );

        let y_noise = self.octaved_noise(
            t * 8.0,
            octaves,
            lacunarity,
            gain,
            amplitude,
            frequency,
            self.time,
            1.0,
        );

        let scale = self.displacement;
        (x_noise * scale, y_noise * scale)
    }

    /// Multi-octave noise function
    fn octaved_noise(
        &self,
        x: f32,
        octaves: usize,
        lacunarity: f32,
        gain: f32,
        base_amplitude: f32,
        base_frequency: f32,
        time: f32,
        seed: f32,
    ) -> f32 {
        let mut result = 0.0;
        let mut amplitude = base_amplitude;
        let mut frequency = base_frequency;

        for _i in 0..octaves {
            result += amplitude
                * self.noise_2d(
                    frequency * x + seed * 100.0,
                    time * frequency * 0.3,
                );
            frequency *= lacunarity;
            amplitude *= gain;
        }

        result
    }

    /// 2D noise function (simplified perlin-style)
    fn noise_2d(&self, x: f32, y: f32) -> f32 {
        let i = x.floor();
        let j = y.floor();
        let fx = x - i;
        let fy = y - j;

        // Sample grid corners
        let a = self.random(i + j * 57.0);
        let b = self.random(i + 1.0 + j * 57.0);
        let c = self.random(i + (j + 1.0) * 57.0);
        let d = self.random(i + 1.0 + (j + 1.0) * 57.0);

        // Smoothstep interpolation
        let ux = fx * fx * (3.0 - 2.0 * fx);
        let uy = fy * fy * (3.0 - 2.0 * fy);

        // Bilinear interpolation
        a * (1.0 - ux) * (1.0 - uy)
            + b * ux * (1.0 - uy)
            + c * (1.0 - ux) * uy
            + d * ux * uy
    }

    /// Pseudo-random function
    fn random(&self, x: f32) -> f32 {
        ((x * 12.9898).sin() * 43758.5453) % 1.0
    }

    /// Get number of sample points
    pub fn sample_count(&self) -> usize {
        self.sample_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_electric_border_creation() {
        let border = ElectricBorder::new(400.0, 300.0);
        assert_eq!(border.width, 400.0);
        assert_eq!(border.height, 300.0);
        assert!(border.sample_count > 0);
    }

    #[test]
    fn test_time_update() {
        let mut border = ElectricBorder::new(400.0, 300.0);

        border.update(0.1);
        assert!((border.time() - 0.1).abs() < 0.01);

        border.update(0.1);
        assert!((border.time() - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_speed_multiplier() {
        let mut border = ElectricBorder::new(400.0, 300.0).with_speed(2.0);

        border.update(0.1);
        assert!((border.time() - 0.2).abs() < 0.01); // 0.1 * 2.0
    }

    #[test]
    fn test_generate_points() {
        let border = ElectricBorder::new(400.0, 300.0);
        let points = border.generate_points();

        // Should have sample_count + 1 points (including closing point)
        assert_eq!(points.len(), border.sample_count() + 1);

        // Points should be roughly within bounds (plus displacement margin)
        for (x, y) in points.iter() {
            assert!(
                x >= &-border.displacement && x <= &(border.width + border.displacement)
            );
            assert!(
                y >= &-border.displacement && y <= &(border.height + border.displacement)
            );
        }
    }

    #[test]
    fn test_rounded_rect_corners() {
        let border = ElectricBorder::new(400.0, 300.0).with_radius(24.0);

        // Test corner points (approximately)
        let radius = border.border_radius;

        // Top-left corner region (t near 0)
        let (x, y) = border.get_rounded_rect_point(0.0, radius);
        assert!(x >= radius - 1.0 && x <= radius + 1.0);
        assert!(y < 1.0);

        // Closing point should match start
        let (x_end, y_end) = border.get_rounded_rect_point(1.0, radius);
        assert!((x - x_end).abs() < 10.0);
        assert!((y - y_end).abs() < 10.0);
    }

    #[test]
    fn test_noise_consistency() {
        let border = ElectricBorder::new(400.0, 300.0);

        // Same input should produce same output
        let noise1 = border.noise_2d(1.5, 2.5);
        let noise2 = border.noise_2d(1.5, 2.5);
        assert_eq!(noise1, noise2);
    }

    #[test]
    fn test_random_function() {
        let border = ElectricBorder::new(400.0, 300.0);

        // Random should be deterministic
        let r1 = border.random(42.0);
        let r2 = border.random(42.0);
        assert_eq!(r1, r2);

        // Different inputs should give different outputs
        let r3 = border.random(43.0);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_dimensions_update() {
        let mut border = ElectricBorder::new(400.0, 300.0);
        let old_count = border.sample_count();

        border.set_dimensions(800.0, 600.0);
        assert_eq!(border.width, 800.0);
        assert_eq!(border.height, 600.0);

        // Sample count should increase with perimeter
        assert!(border.sample_count() > old_count);
    }

    #[test]
    fn test_displacement_changes_over_time() {
        let mut border = ElectricBorder::new(400.0, 300.0);

        let points_t0 = border.generate_points();

        border.update(1.0);
        let points_t1 = border.generate_points();

        // Points should be different due to time-varying noise
        let mut different = false;
        for i in 0..points_t0.len().min(points_t1.len()) {
            if (points_t0[i].0 - points_t1[i].0).abs() > 0.1
                || (points_t0[i].1 - points_t1[i].1).abs() > 0.1
            {
                different = true;
                break;
            }
        }
        assert!(different, "Points should change over time");
    }
}
