//! Rolling counter (slot machine style)
//!
//! Each digit is a vertical column of 0-9 that scrolls to the target digit.
//! Uses spring physics for smooth rolling with wrap-around effect.
//!
//! # Algorithm (from React source)
//!
//! For each digit position:
//! 1. Round the value to the current place (e.g., 1234 at place=10 → 123)
//! 2. Create a spring that animates to this rounded value
//! 3. For each number 0-9 in the column:
//!    - Calculate offset = (10 + number - placeValue%10) % 10
//!    - If offset > 5, subtract 10 (wrap around for continuity)
//!    - Multiply by digit height to get Y position
//!
//! This creates a slot-machine effect where digits roll smoothly.

#[cfg(feature = "animation")]
use uzor_animation::Spring;

/// Place value in the number (powers of 10 or decimal point)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceValue {
    /// Decimal point separator
    Dot,
    /// Power of 10 (e.g., 1, 10, 100, 0.1, 0.01)
    Power(f64),
}

impl PlaceValue {
    /// Check if this is a decimal point
    #[inline]
    pub fn is_dot(&self) -> bool {
        matches!(self, PlaceValue::Dot)
    }

    /// Get the power value (returns None for Dot)
    #[inline]
    pub fn power(&self) -> Option<f64> {
        match self {
            PlaceValue::Power(p) => Some(*p),
            PlaceValue::Dot => None,
        }
    }
}

/// State of a single digit column (0-9 positions)
#[derive(Debug, Clone)]
pub struct DigitState {
    /// Y offsets for each digit 0-9 in the column
    /// Relative to digit height (multiply by actual height when rendering)
    pub digit_offsets: [f32; 10],

    /// Current spring value (rounded to this place)
    pub spring_value: f64,
}

/// Complete counter state for rendering
#[derive(Debug, Clone)]
pub struct CounterState {
    /// State for each digit/separator
    pub digits: Vec<(PlaceValue, Option<DigitState>)>,
}

impl CounterState {
    /// Get number of places (digits + separators)
    #[inline]
    pub fn len(&self) -> usize {
        self.digits.len()
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.digits.is_empty()
    }
}

/// Rolling counter with slot-machine digit display
///
/// # Example
///
/// ```
/// use uzor_numbers::Counter;
///
/// let counter = Counter::new(1234.56);
/// let state = counter.evaluate(0.1); // Get state at t=0.1s
///
/// // Render each digit using state.digits
/// for (place, digit_state) in &state.digits {
///     if place.is_dot() {
///         // Render decimal point
///     } else if let Some(ds) = digit_state {
///         // Render column of digits with Y offsets: ds.digit_offsets
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Counter {
    /// Target value to display
    pub value: f64,

    /// Place values (auto-detected from value if None)
    pub places: Option<Vec<PlaceValue>>,

    /// Spring configuration for digit rolling
    #[cfg(feature = "animation")]
    pub spring: Spring,

    /// Time offset (for animations that don't start at t=0)
    pub time_offset: f64,
}

impl Counter {
    /// Create a new counter with the given value
    pub fn new(value: f64) -> Self {
        Self {
            value,
            places: None,
            #[cfg(feature = "animation")]
            spring: Spring::new().stiffness(100.0).damping(10.0),
            time_offset: 0.0,
        }
    }

    /// Set explicit place values
    pub fn with_places(mut self, places: Vec<PlaceValue>) -> Self {
        self.places = Some(places);
        self
    }

    /// Set spring configuration
    #[cfg(feature = "animation")]
    pub fn with_spring(mut self, spring: Spring) -> Self {
        self.spring = spring;
        self
    }

    /// Set time offset
    pub fn with_time_offset(mut self, offset: f64) -> Self {
        self.time_offset = offset;
        self
    }

    /// Auto-detect place values from the current value
    ///
    /// Implements the algorithm from React Counter.tsx lines 95-105
    fn auto_detect_places(&self) -> Vec<PlaceValue> {
        let value_str = self.value.to_string();
        let chars: Vec<char> = value_str.chars().collect();

        chars
            .iter()
            .enumerate()
            .map(|(i, &ch)| {
                if ch == '.' {
                    return PlaceValue::Dot;
                }

                // Find decimal point position
                let dot_index = chars.iter().position(|&c| c == '.');
                let is_integer = dot_index.is_none();

                // Calculate exponent for this position
                let exponent = if is_integer {
                    (chars.len() - i - 1) as i32
                } else {
                    let dot_idx = dot_index.unwrap();
                    if i < dot_idx {
                        (dot_idx - i - 1) as i32
                    } else {
                        -((i - dot_idx) as i32)
                    }
                };

                PlaceValue::Power(10_f64.powi(exponent))
            })
            .collect()
    }

    /// Get place values (auto-detect if not set)
    fn get_places(&self) -> Vec<PlaceValue> {
        self.places.clone().unwrap_or_else(|| self.auto_detect_places())
    }

    /// Evaluate counter state at time t (seconds)
    #[cfg(feature = "animation")]
    pub fn evaluate(&self, t: f64) -> CounterState {
        let adjusted_t = t - self.time_offset;
        let places = self.get_places();

        let digits = places
            .into_iter()
            .map(|place| {
                if place.is_dot() {
                    // Decimal point - no animation
                    (place, None)
                } else {
                    let power = place.power().unwrap();
                    let digit_state = self.evaluate_digit(power, adjusted_t);
                    (place, Some(digit_state))
                }
            })
            .collect();

        CounterState { digits }
    }

    /// Evaluate counter state without animation (static display)
    #[cfg(not(feature = "animation"))]
    pub fn evaluate(&self, _t: f64) -> CounterState {
        let places = self.get_places();

        let digits = places
            .into_iter()
            .map(|place| {
                if place.is_dot() {
                    (place, None)
                } else {
                    let power = place.power().unwrap();
                    let digit_state = self.evaluate_digit_static(power);
                    (place, Some(digit_state))
                }
            })
            .collect();

        CounterState { digits }
    }

    /// Evaluate a single digit column with spring animation
    ///
    /// Implements algorithm from React Counter.tsx Number component
    #[cfg(feature = "animation")]
    fn evaluate_digit(&self, place: f64, t: f64) -> DigitState {
        // Round value to this place
        let value_rounded = (self.value / place).floor();

        // Create spring from current position
        // Spring evaluates displacement from target (1.0 = start, 0.0 = target)
        let (displacement, _velocity) = self.spring.evaluate(t);

        // Current animated value (interpolate from 0 to target)
        let spring_value = value_rounded * (1.0 - displacement);

        // Calculate Y offset for each digit 0-9
        let place_value = spring_value % 10.0;

        let mut digit_offsets = [0.0f32; 10];
        for (number, slot) in digit_offsets.iter_mut().enumerate() {
            // Algorithm from React lines 17-23
            let offset = (10.0 + number as f64 - place_value) % 10.0;
            let mut memo = offset;

            // Wrap around for smooth rolling
            if offset > 5.0 {
                memo -= 10.0;
            }

            *slot = memo as f32;
        }

        DigitState {
            digit_offsets,
            spring_value,
        }
    }

    /// Evaluate a single digit column without animation
    #[cfg(not(feature = "animation"))]
    fn evaluate_digit_static(&self, place: f64) -> DigitState {
        let value_rounded = (self.value / place).floor();
        let place_value = value_rounded % 10.0;

        let mut digit_offsets = [0.0f32; 10];
        for number in 0..10 {
            let offset = (10.0 + number as f64 - place_value) % 10.0;
            let mut memo = offset;

            if offset > 5.0 {
                memo -= 10.0;
            }

            digit_offsets[number] = memo as f32;
        }

        DigitState {
            digit_offsets,
            spring_value: value_rounded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_value() {
        let dot = PlaceValue::Dot;
        assert!(dot.is_dot());
        assert_eq!(dot.power(), None);

        let hundreds = PlaceValue::Power(100.0);
        assert!(!hundreds.is_dot());
        assert_eq!(hundreds.power(), Some(100.0));
    }

    #[test]
    fn test_auto_detect_places_integer() {
        let counter = Counter::new(1234.0);
        let places = counter.auto_detect_places();

        assert_eq!(places.len(), 4);
        assert_eq!(places[0], PlaceValue::Power(1000.0));
        assert_eq!(places[1], PlaceValue::Power(100.0));
        assert_eq!(places[2], PlaceValue::Power(10.0));
        assert_eq!(places[3], PlaceValue::Power(1.0));
    }

    #[test]
    fn test_auto_detect_places_decimal() {
        let counter = Counter::new(12.34);
        let places = counter.auto_detect_places();

        assert_eq!(places.len(), 5);
        assert_eq!(places[0], PlaceValue::Power(10.0));
        assert_eq!(places[1], PlaceValue::Power(1.0));
        assert_eq!(places[2], PlaceValue::Dot);
        assert_eq!(places[3], PlaceValue::Power(0.1));
        assert_eq!(places[4], PlaceValue::Power(0.01));
    }

    #[test]
    fn test_evaluate_at_start() {
        let counter = Counter::new(123.0);
        let state = counter.evaluate(0.0);

        assert_eq!(state.len(), 3);
        assert!(!state.is_empty());
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_digit_offsets() {
        let counter = Counter::new(5.0);
        let state = counter.evaluate(0.0);

        // At t=0, spring displacement = 1.0, so spring_value ≈ 0
        // place_value = 0 % 10 = 0
        // digit 0: offset = (10 + 0 - 0) % 10 = 0
        // digit 5: offset = (10 + 5 - 0) % 10 = 5

        assert_eq!(state.digits.len(), 1);
        let (place, digit_state) = &state.digits[0];
        assert_eq!(place, &PlaceValue::Power(1.0));

        let ds = digit_state.as_ref().unwrap();
        // At start, all digits should have calculated offsets
        assert_eq!(ds.digit_offsets.len(), 10);
    }

    #[test]
    fn test_custom_places() {
        let counter = Counter::new(1234.0).with_places(vec![
            PlaceValue::Power(100.0),
            PlaceValue::Power(10.0),
        ]);

        let state = counter.evaluate(0.0);
        assert_eq!(state.len(), 2);
    }

    #[test]
    fn test_decimal_point_in_places() {
        let counter = Counter::new(12.3);
        let state = counter.evaluate(0.0);

        // Should have 4 places: 10, 1, dot, 0.1
        assert_eq!(state.len(), 4);

        let (place, digit_state) = &state.digits[2];
        assert_eq!(place, &PlaceValue::Dot);
        assert!(digit_state.is_none());
    }

    #[test]
    #[cfg(feature = "animation")]
    fn test_spring_progression() {
        let counter = Counter::new(5.0);

        let state_start = counter.evaluate(0.0);
        let state_mid = counter.evaluate(0.3);
        let state_end = counter.evaluate(0.8);

        let ds_start = state_start.digits[0].1.as_ref().unwrap();
        let ds_mid = state_mid.digits[0].1.as_ref().unwrap();
        let ds_end = state_end.digits[0].1.as_ref().unwrap();

        // Spring value should increase over time (from 0 towards 5)
        assert!(ds_mid.spring_value > ds_start.spring_value);
        // At later time, should be closer to target (5.0)
        assert!((ds_end.spring_value - 5.0).abs() < (ds_start.spring_value - 5.0).abs());
    }

    #[test]
    fn test_time_offset() {
        let counter = Counter::new(5.0).with_time_offset(1.0);

        let state = counter.evaluate(1.0);
        let ds = state.digits[0].1.as_ref().unwrap();

        // At t=1.0 with offset=1.0, effective t=0.0
        // So spring_value should be near 0
        assert!(ds.spring_value.abs() < 1.0);
    }
}
