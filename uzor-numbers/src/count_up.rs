//! Spring-based count up/down animation
//!
//! Animates a number from start value to end value using spring physics.
//! The damping and stiffness are calculated from duration for smooth motion.
//!
//! # Algorithm (from React CountUp.tsx)
//!
//! - damping = 20 + 40 * (1 / duration)
//! - stiffness = 100 * (1 / duration)
//! - Spring animates from `from` to `to` (or reversed for countdown)
//! - Supports decimal places (auto-detected from input values)
//! - Can format with thousands separators

#[cfg(feature = "animation")]
use uzor_animation::Spring;

/// Direction of counting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Count up from start to end
    Up,
    /// Count down from start to end
    Down,
}

/// State of the count-up animation
#[derive(Debug, Clone)]
pub struct CountUpState {
    /// Current animated value
    pub value: f64,

    /// Is animation complete?
    pub is_complete: bool,
}

/// Spring-animated number counter
///
/// # Example
///
/// ```
/// use uzor_numbers::{CountUp, Direction};
///
/// let count_up = CountUp::new(0.0, 100.0)
///     .with_duration(2.0)
///     .with_direction(Direction::Up);
///
/// let state = count_up.evaluate(1.0); // Halfway through
/// println!("Value: {}", state.value); // ~50
/// ```
#[derive(Debug, Clone)]
pub struct CountUp {
    /// Start value
    pub from: f64,

    /// End value
    pub to: f64,

    /// Direction (up or down)
    pub direction: Direction,

    /// Animation duration in seconds
    pub duration: f64,

    /// Delay before starting (seconds)
    pub delay: f64,

    /// Spring configuration (auto-calculated from duration if None)
    #[cfg(feature = "animation")]
    pub spring: Option<Spring>,
}

impl CountUp {
    /// Create a new count-up animation
    pub fn new(from: f64, to: f64) -> Self {
        Self {
            from,
            to,
            direction: Direction::Up,
            duration: 2.0,
            delay: 0.0,
            #[cfg(feature = "animation")]
            spring: None,
        }
    }

    /// Set direction
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration.max(0.1); // Prevent division by zero
        self
    }

    /// Set delay
    pub fn with_delay(mut self, delay: f64) -> Self {
        self.delay = delay.max(0.0);
        self
    }

    /// Set custom spring (overrides duration-based calculation)
    #[cfg(feature = "animation")]
    pub fn with_spring(mut self, spring: Spring) -> Self {
        self.spring = Some(spring);
        self
    }

    /// Calculate spring parameters from duration
    ///
    /// Implements React CountUp.tsx lines 32-38
    #[cfg(feature = "animation")]
    fn calculate_spring(&self) -> Spring {
        if let Some(spring) = self.spring {
            return spring;
        }

        let damping = 20.0 + 40.0 * (1.0 / self.duration);
        let stiffness = 100.0 * (1.0 / self.duration);

        Spring::new()
            .damping(damping)
            .stiffness(stiffness)
            .mass(1.0)
    }

    /// Evaluate animation state at time t (seconds)
    #[cfg(feature = "animation")]
    pub fn evaluate(&self, t: f64) -> CountUpState {
        // Account for delay
        let effective_t = t - self.delay;

        if effective_t < 0.0 {
            // Before animation starts
            let initial = match self.direction {
                Direction::Up => self.from,
                Direction::Down => self.to,
            };
            return CountUpState {
                value: initial,
                is_complete: false,
            };
        }

        let spring = self.calculate_spring();

        // Evaluate spring (displacement from target: 1.0 at start, 0.0 at end)
        let (displacement, _velocity) = spring.evaluate(effective_t);
        let is_complete = spring.is_at_rest(effective_t);

        // Calculate current value
        let value = match self.direction {
            Direction::Up => {
                // Animate from `from` to `to`
                self.from + (self.to - self.from) * (1.0 - displacement)
            }
            Direction::Down => {
                // Animate from `to` to `from`
                self.to + (self.from - self.to) * (1.0 - displacement)
            }
        };

        CountUpState { value, is_complete }
    }

    /// Evaluate without animation (returns final value)
    #[cfg(not(feature = "animation"))]
    pub fn evaluate(&self, t: f64) -> CountUpState {
        let effective_t = t - self.delay;

        let value = if effective_t < 0.0 {
            match self.direction {
                Direction::Up => self.from,
                Direction::Down => self.to,
            }
        } else {
            match self.direction {
                Direction::Up => self.to,
                Direction::Down => self.from,
            }
        };

        CountUpState {
            value,
            is_complete: effective_t >= 0.0,
        }
    }

    /// Get number of decimal places for formatting
    ///
    /// Implements React CountUp.tsx getDecimalPlaces (lines 42-51)
    pub fn get_decimal_places(&self) -> usize {
        let from_decimals = Self::count_decimals(self.from);
        let to_decimals = Self::count_decimals(self.to);
        from_decimals.max(to_decimals)
    }

    /// Count decimal places in a number
    fn count_decimals(num: f64) -> usize {
        let s = num.to_string();
        if let Some(dot_pos) = s.find('.') {
            let decimals = &s[dot_pos + 1..];
            // Check if decimals are non-zero
            if decimals.parse::<i64>().ok().filter(|&d| d != 0).is_some() {
                return decimals.len();
            }
        }
        0
    }

    /// Format the current value with proper decimal places
    ///
    /// Returns (formatted_string, decimal_places)
    pub fn format_value(&self, value: f64) -> (String, usize) {
        let decimals = self.get_decimal_places();

        if decimals > 0 {
            (format!("{:.prec$}", value, prec = decimals), decimals)
        } else {
            (format!("{:.0}", value), 0)
        }
    }

    /// Format with thousands separator
    pub fn format_with_separator(&self, value: f64, separator: &str) -> String {
        let (base, _decimals) = self.format_value(value);

        if separator.is_empty() {
            return base;
        }

        // Split into integer and decimal parts
        let parts: Vec<&str> = base.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

        // Add separators to integer part (from right to left, every 3 digits)
        let mut formatted = String::new();
        let chars: Vec<char> = integer_part.chars().collect();
        let len = chars.len();

        for (i, &ch) in chars.iter().enumerate() {
            formatted.push(ch);
            let remaining = len - i - 1;
            if remaining > 0 && remaining % 3 == 0 {
                formatted.push_str(separator);
            }
        }

        // Add decimal part if exists
        if !decimal_part.is_empty() {
            formatted.push('.');
            formatted.push_str(decimal_part);
        }

        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction() {
        let up = CountUp::new(0.0, 100.0);
        assert_eq!(up.direction, Direction::Up);

        let down = CountUp::new(0.0, 100.0).with_direction(Direction::Down);
        assert_eq!(down.direction, Direction::Down);
    }

    #[test]
    fn test_count_decimals() {
        assert_eq!(CountUp::count_decimals(123.0), 0);
        assert_eq!(CountUp::count_decimals(123.45), 2);
        assert_eq!(CountUp::count_decimals(0.001), 3);
        assert_eq!(CountUp::count_decimals(100.00), 0); // Trailing zeros removed
    }

    #[test]
    fn test_get_decimal_places() {
        let cu1 = CountUp::new(0.0, 100.0);
        assert_eq!(cu1.get_decimal_places(), 0);

        let cu2 = CountUp::new(0.0, 100.5);
        assert_eq!(cu2.get_decimal_places(), 1);

        let cu3 = CountUp::new(0.123, 100.0);
        assert_eq!(cu3.get_decimal_places(), 3);

        let cu4 = CountUp::new(0.1, 100.99);
        assert_eq!(cu4.get_decimal_places(), 2);
    }

    #[test]
    fn test_format_value() {
        let cu = CountUp::new(0.0, 100.5);
        let (formatted, decimals) = cu.format_value(50.25);
        assert_eq!(formatted, "50.2");
        assert_eq!(decimals, 1);

        let cu2 = CountUp::new(0.0, 100.0);
        let (formatted2, decimals2) = cu2.format_value(50.0);
        assert_eq!(formatted2, "50");
        assert_eq!(decimals2, 0);
    }

    #[test]
    fn test_format_with_separator() {
        let cu = CountUp::new(0.0, 100000.0);
        let formatted = cu.format_with_separator(12345.0, ",");
        assert_eq!(formatted, "12,345");

        let cu2 = CountUp::new(0.0, 1000000.5);
        let formatted2 = cu2.format_with_separator(1234567.5, ",");
        assert_eq!(formatted2, "1,234,567.5");

        let cu3 = CountUp::new(0.0, 100.0);
        let formatted3 = cu3.format_with_separator(100.0, "");
        assert_eq!(formatted3, "100");
    }

    #[test]
    fn test_evaluate_before_delay() {
        let cu = CountUp::new(0.0, 100.0).with_delay(1.0);
        let state = cu.evaluate(0.5);

        assert_eq!(state.value, 0.0);
        assert!(!state.is_complete);
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_evaluate_up() {
        let cu = CountUp::new(0.0, 100.0).with_duration(2.0);

        let state_start = cu.evaluate(0.0);
        let state_mid = cu.evaluate(1.0);
        let state_end = cu.evaluate(10.0); // Longer time for spring to settle

        // Should start near 0
        assert!(state_start.value < 10.0);

        // Should progress towards 100
        assert!(state_mid.value > state_start.value);
        assert!(state_mid.value < 100.0);

        // Should reach near 100
        assert!(state_end.value > 90.0);
        assert!(state_end.is_complete);
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_evaluate_down() {
        let cu = CountUp::new(0.0, 100.0)
            .with_direction(Direction::Down)
            .with_duration(2.0);

        let state_start = cu.evaluate(0.0);
        let state_end = cu.evaluate(5.0);

        // Should start near 100 (counting down)
        assert!(state_start.value > 90.0);

        // Should reach near 0
        assert!(state_end.value < 10.0);
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_custom_spring() {
        let spring = Spring::bouncy();
        let cu = CountUp::new(0.0, 100.0).with_spring(spring);

        let state = cu.evaluate(0.5);
        // Should animate with bouncy spring
        assert!(state.value > 0.0);
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_duration_affects_speed() {
        let cu_fast = CountUp::new(0.0, 100.0).with_duration(0.5);
        let cu_slow = CountUp::new(0.0, 100.0).with_duration(5.0);

        let state_fast = cu_fast.evaluate(0.25);
        let state_slow = cu_slow.evaluate(0.25);

        // Fast should progress more than slow at same time
        assert!(state_fast.value > state_slow.value);
    }

    #[test]
    fn test_with_delay() {
        let cu = CountUp::new(0.0, 100.0).with_delay(2.0);
        assert_eq!(cu.delay, 2.0);

        let cu2 = CountUp::new(0.0, 100.0).with_delay(-1.0);
        assert_eq!(cu2.delay, 0.0); // Negative clamped to 0
    }

    #[test]
    fn test_with_duration() {
        let cu = CountUp::new(0.0, 100.0).with_duration(3.5);
        assert_eq!(cu.duration, 3.5);

        let cu2 = CountUp::new(0.0, 100.0).with_duration(0.01);
        assert!(cu2.duration >= 0.1); // Too small clamped to 0.1
    }
}
