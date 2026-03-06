use rand::Rng;

/// Decrypted text scramble/reveal effect.
///
/// Two modes:
/// - Sequential: reveals characters one by one (left→right, right→left, or center→out)
/// - Random scramble: all chars scramble N iterations then snap to original
///
/// # Algorithm (from React source)
///
/// Sequential mode:
/// - Each iteration reveals one more character based on direction
/// - start: index = revealed_count (left to right)
/// - end: index = text.len() - 1 - revealed_count (right to left)
/// - center: alternates outward from middle
/// - Unrevealed chars are randomly shuffled from character set
///
/// Random scramble mode:
/// - All characters scramble randomly each iteration
/// - After maxIterations, snaps to original text

#[derive(Debug, Clone)]
pub struct DecryptedTextConfig {
    /// Delay between iterations in milliseconds (default: 50)
    pub speed_ms: u64,
    /// Max iterations for random scramble mode (default: 10)
    pub max_iterations: usize,
    /// Sequential reveal vs random scramble (default: false)
    pub sequential: bool,
    /// Direction for sequential mode (default: Start)
    pub reveal_direction: RevealDirection,
    /// Use only chars from original text (default: false)
    pub use_original_chars_only: bool,
    /// Character set for scrambling (default: alphanumeric + symbols)
    pub characters: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealDirection {
    Start,   // Left to right
    End,     // Right to left
    Center,  // Center outward
}

impl Default for DecryptedTextConfig {
    fn default() -> Self {
        Self {
            speed_ms: 50,
            max_iterations: 10,
            sequential: false,
            reveal_direction: RevealDirection::Start,
            use_original_chars_only: false,
            characters: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!@#$%^&*()_+"
                .to_string(),
        }
    }
}

#[derive(Debug)]
pub struct DecryptedTextState {
    original_text: Vec<char>,
    current_text: Vec<char>,
    revealed_indices: Vec<bool>,
    iteration: usize,
    is_complete: bool,
}

impl DecryptedTextState {
    pub fn new(text: &str) -> Self {
        let original_text: Vec<char> = text.chars().collect();
        let current_text = original_text.clone();
        let revealed_indices = vec![false; original_text.len()];

        Self {
            original_text,
            current_text,
            revealed_indices,
            iteration: 0,
            is_complete: false,
        }
    }

    /// Advance one iteration and return current display text.
    pub fn update(&mut self, config: &DecryptedTextConfig) -> Vec<char> {
        if self.is_complete {
            return self.current_text.clone();
        }

        let mut rng = rand::thread_rng();

        if config.sequential {
            // Sequential mode: reveal one character per iteration
            let revealed_count = self.revealed_indices.iter().filter(|&&r| r).count();

            if revealed_count < self.original_text.len() {
                let next_index = self.get_next_index(revealed_count, config.reveal_direction);
                if next_index < self.original_text.len() {
                    self.revealed_indices[next_index] = true;
                }

                // Scramble unrevealed characters
                self.current_text = self.shuffle_text(config, &mut rng);
            } else {
                self.is_complete = true;
                self.current_text = self.original_text.clone();
            }
        } else {
            // Random scramble mode: scramble all, then snap after max iterations
            self.iteration += 1;

            if self.iteration >= config.max_iterations {
                self.is_complete = true;
                self.current_text = self.original_text.clone();
            } else {
                self.current_text = self.shuffle_text(config, &mut rng);
            }
        }

        self.current_text.clone()
    }

    /// Get the next index to reveal based on direction.
    fn get_next_index(&self, revealed_count: usize, direction: RevealDirection) -> usize {
        let text_len = self.original_text.len();

        match direction {
            RevealDirection::Start => revealed_count,
            RevealDirection::End => {
                if revealed_count < text_len {
                    text_len - 1 - revealed_count
                } else {
                    0
                }
            }
            RevealDirection::Center => {
                let middle = text_len / 2;
                let offset = revealed_count / 2;

                let next_index = if revealed_count % 2 == 0 {
                    middle + offset
                } else {
                    middle.saturating_sub(offset + 1)
                };

                // Ensure index is valid and not already revealed
                if next_index < text_len && !self.revealed_indices[next_index] {
                    next_index
                } else {
                    // Fallback: find first unrevealed
                    self.revealed_indices
                        .iter()
                        .position(|&r| !r)
                        .unwrap_or(0)
                }
            }
        }
    }

    /// Shuffle unrevealed characters.
    fn shuffle_text(
        &self,
        config: &DecryptedTextConfig,
        rng: &mut impl Rng,
    ) -> Vec<char> {
        if config.use_original_chars_only {
            // Use only characters from original text (excluding spaces)
            let available_chars: Vec<char> = self
                .original_text
                .iter()
                .filter(|&&c| c != ' ')
                .copied()
                .collect();

            // Create shuffled version of non-space chars
            let mut shuffled_chars = available_chars.clone();
            // Fisher-Yates shuffle
            for i in (1..shuffled_chars.len()).rev() {
                let j = rng.gen_range(0..=i);
                shuffled_chars.swap(i, j);
            }

            let mut char_index = 0;
            self.original_text
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    if c == ' ' {
                        ' '
                    } else if config.sequential && self.revealed_indices[i] {
                        self.original_text[i]
                    } else if char_index < shuffled_chars.len() {
                        let result = shuffled_chars[char_index];
                        char_index += 1;
                        result
                    } else {
                        c
                    }
                })
                .collect()
        } else {
            // Use provided character set
            let available_chars: Vec<char> = config.characters.chars().collect();

            self.original_text
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    if c == ' ' {
                        ' '
                    } else if config.sequential && self.revealed_indices[i] {
                        self.original_text[i]
                    } else if !available_chars.is_empty() {
                        available_chars[rng.gen_range(0..available_chars.len())]
                    } else {
                        c
                    }
                })
                .collect()
        }
    }

    /// Check if animation is complete.
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Get which indices are revealed (for styling purposes).
    pub fn revealed_indices(&self) -> &[bool] {
        &self.revealed_indices
    }

    /// Reset animation state.
    pub fn reset(&mut self) {
        self.current_text = self.original_text.clone();
        self.revealed_indices.fill(false);
        self.iteration = 0;
        self.is_complete = false;
    }

    /// Get current display text as String.
    pub fn display_text(&self) -> String {
        self.current_text.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_start() {
        let config = DecryptedTextConfig {
            sequential: true,
            reveal_direction: RevealDirection::Start,
            ..Default::default()
        };
        let mut state = DecryptedTextState::new("HELLO");

        // First iteration: reveals 'H'
        state.update(&config);
        assert!(state.revealed_indices[0]);
        assert!(!state.revealed_indices[1]);

        // Second iteration: reveals 'E'
        state.update(&config);
        assert!(state.revealed_indices[1]);

        // Continue until all revealed (5 chars = 5 updates)
        state.update(&config);
        state.update(&config);
        state.update(&config);

        // One more update to trigger completion
        state.update(&config);

        assert!(state.is_complete());
        assert_eq!(state.display_text(), "HELLO");
    }

    #[test]
    fn test_sequential_end() {
        let config = DecryptedTextConfig {
            sequential: true,
            reveal_direction: RevealDirection::End,
            ..Default::default()
        };
        let mut state = DecryptedTextState::new("HELLO");

        // First iteration: reveals 'O' (last char)
        state.update(&config);
        assert!(state.revealed_indices[4]);
        assert!(!state.revealed_indices[3]);

        // Second iteration: reveals 'L'
        state.update(&config);
        assert!(state.revealed_indices[3]);
    }

    #[test]
    fn test_sequential_center() {
        let config = DecryptedTextConfig {
            sequential: true,
            reveal_direction: RevealDirection::Center,
            ..Default::default()
        };
        let mut state = DecryptedTextState::new("HELLO");

        // First iteration: reveals middle char
        state.update(&config);
        assert!(state.revealed_indices[2]); // 'L'

        // Second iteration: reveals left of middle
        state.update(&config);
        assert!(state.revealed_indices[1]); // 'E'
    }

    #[test]
    fn test_random_scramble() {
        let config = DecryptedTextConfig {
            sequential: false,
            max_iterations: 5,
            ..Default::default()
        };
        let mut state = DecryptedTextState::new("TEST");

        // Run iterations
        for _ in 0..4 {
            let text = state.update(&config);
            // Text should be scrambled (likely different from original)
            assert_eq!(text.len(), 4);
            assert!(!state.is_complete());
        }

        // Final iteration: snaps to original
        let final_text = state.update(&config);
        assert!(state.is_complete());
        assert_eq!(final_text.iter().collect::<String>(), "TEST");
    }

    #[test]
    fn test_spaces_preserved() {
        let config = DecryptedTextConfig::default();
        let mut state = DecryptedTextState::new("HI WORLD");

        let text = state.update(&config);
        // Space should be preserved at index 2
        assert_eq!(text[2], ' ');
    }

    #[test]
    fn test_reset() {
        let config = DecryptedTextConfig {
            sequential: true,
            ..Default::default()
        };
        let mut state = DecryptedTextState::new("TEST");

        state.update(&config);
        state.update(&config);
        assert!(state.revealed_indices[0]);

        state.reset();
        assert!(!state.revealed_indices[0]);
        assert_eq!(state.iteration, 0);
        assert!(!state.is_complete());
    }
}
