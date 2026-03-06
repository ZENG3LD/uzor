//! Page transition animation types catalog
//!
//! Defines all page transition variants with their parameters.
//! Based on research from 04-page-transitions.md.

use crate::{Easing, Timeline};
use std::time::Duration;

/// Catalog of page transition patterns
#[derive(Debug, Clone)]
pub enum TransitionAnimation {
    /// Material Design Shared Axis X (horizontal slide + fade)
    /// Old page slides left while fading, new page slides in from right
    SharedAxisX {
        enter_duration_ms: u64,
        exit_duration_ms: u64,
        overlap_ms: u64,
        easing: Easing,
        distance: f64,
    },

    /// Material Design Shared Axis Y (vertical slide + fade)
    /// Old page slides up while fading, new page slides in from below
    SharedAxisY {
        enter_duration_ms: u64,
        exit_duration_ms: u64,
        overlap_ms: u64,
        easing: Easing,
        distance: f64,
    },

    /// Material Design Fade Through (sequential fade with scale)
    /// Old page fades out, new page fades in with slight scale-up
    FadeThrough {
        exit_duration_ms: u64,
        enter_duration_ms: u64,
        exit_easing: Easing,
        enter_easing: Easing,
        enter_scale_from: f64,
    },

    /// Simple crossfade (opacity only, parallel)
    /// Old page fades out while new page fades in simultaneously
    CrossFade {
        duration_ms: u64,
        easing: Easing,
    },

    /// iOS Push Navigation (horizontal slide)
    /// New page slides in from right, old page slides left partially
    PushLeft {
        duration_ms: u64,
        easing: Easing,
        old_page_offset: f64,
    },

    /// Slide Over (new page slides over stationary old page)
    /// New page slides in from right, old page stays put
    SlideOver {
        duration_ms: u64,
        easing: Easing,
        direction: SlideDirection,
        enter_scale: f64,
    },

    /// Zoom In (old scales down, new scales up)
    /// Creates depth effect without 3D transforms
    ZoomIn {
        duration_ms: u64,
        old_scale_to: f64,
        new_scale_from: f64,
        easing: Easing,
    },

    /// Circle Reveal (expanding circle clip-path)
    /// New page reveals from center (or click point) outward
    CircleReveal {
        duration_ms: u64,
        easing: Easing,
        origin_x: f64,
        origin_y: f64,
    },

    /// Stair Cascade (staggered element entrance)
    /// New page elements appear sequentially with delay
    StairCascade {
        element_duration_ms: u64,
        stagger_delay_ms: u64,
        easing: Easing,
        old_page_fade_ms: u64,
        translate_distance: f64,
    },

    /// Parallax Slide (multi-layer movement)
    /// Old page moves at different speed than new page
    ParallaxSlide {
        duration_ms: u64,
        easing: Easing,
        old_speed: f64,
        new_speed: f64,
    },
}

/// Direction for slide animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideDirection {
    Left,
    Right,
    Up,
    Down,
}

impl TransitionAnimation {
    /// Get the total combined duration of this transition in milliseconds
    pub fn combined_duration_ms(&self) -> u64 {
        match self {
            TransitionAnimation::SharedAxisX {
                enter_duration_ms,
                exit_duration_ms,
                overlap_ms,
                ..
            } => {
                // Exit starts at 0, enter starts at (exit_duration - overlap)
                let enter_start = exit_duration_ms.saturating_sub(*overlap_ms);
                enter_start + enter_duration_ms
            }
            TransitionAnimation::SharedAxisY {
                enter_duration_ms,
                exit_duration_ms,
                overlap_ms,
                ..
            } => {
                let enter_start = exit_duration_ms.saturating_sub(*overlap_ms);
                enter_start + enter_duration_ms
            }
            TransitionAnimation::FadeThrough {
                exit_duration_ms,
                enter_duration_ms,
                ..
            } => exit_duration_ms + enter_duration_ms,
            TransitionAnimation::CrossFade { duration_ms, .. } => *duration_ms,
            TransitionAnimation::PushLeft { duration_ms, .. } => *duration_ms,
            TransitionAnimation::SlideOver { duration_ms, .. } => *duration_ms,
            TransitionAnimation::ZoomIn { duration_ms, .. } => *duration_ms,
            TransitionAnimation::CircleReveal { duration_ms, .. } => *duration_ms,
            TransitionAnimation::StairCascade {
                element_duration_ms,
                stagger_delay_ms,
                old_page_fade_ms,
                ..
            } => {
                // Assuming 5 elements by default for duration calculation
                let element_count = 5;
                let stagger_total = stagger_delay_ms * (element_count - 1);
                old_page_fade_ms + element_duration_ms + stagger_total
            }
            TransitionAnimation::ParallaxSlide { duration_ms, .. } => *duration_ms,
        }
    }

    /// Get the combined duration as a std::time::Duration
    pub fn combined_duration(&self) -> Duration {
        Duration::from_millis(self.combined_duration_ms())
    }

    /// Create an exit timeline for the old page
    pub fn exit_timeline(&self) -> Timeline {
        let mut timeline = Timeline::new();

        match self {
            TransitionAnimation::SharedAxisX {
                exit_duration_ms,
                easing: _,
                distance: _,
                ..
            } => {
                // Opacity fades out quickly (first 100ms of exit)
                let opacity_duration = (*exit_duration_ms).min(100);
                timeline.add(
                    Duration::from_millis(opacity_duration),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );

                // Translation happens over full duration
                timeline.add(
                    Duration::from_millis(*exit_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::SharedAxisY {
                exit_duration_ms,
                easing: _,
                distance: _,
                ..
            } => {
                let opacity_duration = (*exit_duration_ms).min(100);
                timeline.add(
                    Duration::from_millis(opacity_duration),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline.add(
                    Duration::from_millis(*exit_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::FadeThrough {
                exit_duration_ms,
                exit_easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*exit_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::CrossFade {
                duration_ms,
                easing: _,
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::PushLeft {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::SlideOver { .. } => {
                // Old page doesn't animate in slide-over
            }
            TransitionAnimation::ZoomIn {
                duration_ms,
                easing: _,
                ..
            } => {
                // Both scale and opacity
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::CircleReveal { .. } => {
                // Old page just sits underneath
            }
            TransitionAnimation::StairCascade {
                old_page_fade_ms, ..
            } => {
                timeline.add(
                    Duration::from_millis(*old_page_fade_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::ParallaxSlide {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
        }

        timeline
    }

    /// Create an enter timeline for the new page
    pub fn enter_timeline(&self) -> Timeline {
        let mut timeline = Timeline::new();

        match self {
            TransitionAnimation::SharedAxisX {
                enter_duration_ms,
                exit_duration_ms,
                overlap_ms,
                easing: _,
                ..
            } => {
                let enter_start = exit_duration_ms.saturating_sub(*overlap_ms);
                // Opacity and translation
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(enter_start)),
                );
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(enter_start)),
                );
            }
            TransitionAnimation::SharedAxisY {
                enter_duration_ms,
                exit_duration_ms,
                overlap_ms,
                easing: _,
                ..
            } => {
                let enter_start = exit_duration_ms.saturating_sub(*overlap_ms);
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(enter_start)),
                );
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(enter_start)),
                );
            }
            TransitionAnimation::FadeThrough {
                exit_duration_ms,
                enter_duration_ms,
                enter_easing: _,
                ..
            } => {
                // Enter starts after exit completes
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(*exit_duration_ms)),
                );
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::from_millis(*exit_duration_ms)),
                );
            }
            TransitionAnimation::CrossFade {
                duration_ms,
                easing: _,
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::PushLeft {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::SlideOver {
                duration_ms,
                easing: _,
                ..
            } => {
                // Slide in + scale
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::ZoomIn {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::CircleReveal {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
            TransitionAnimation::StairCascade {
                element_duration_ms,
                stagger_delay_ms,
                old_page_fade_ms,
                ..
            } => {
                // Start after old page fades
                let start_time = Duration::from_millis(*old_page_fade_ms);
                // Add entries for first 5 elements (example)
                for i in 0..5 {
                    let stagger_offset = Duration::from_millis(stagger_delay_ms * i);
                    timeline.add(
                        Duration::from_millis(*element_duration_ms),
                        crate::timeline::Position::Absolute(start_time + stagger_offset),
                    );
                }
            }
            TransitionAnimation::ParallaxSlide {
                duration_ms,
                easing: _,
                ..
            } => {
                timeline.add(
                    Duration::from_millis(*duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
            }
        }

        timeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_axis_duration() {
        let transition = TransitionAnimation::SharedAxisX {
            enter_duration_ms: 200,
            exit_duration_ms: 100,
            overlap_ms: 0,
            easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            distance: 45.0,
        };

        // Exit 100ms, then enter 200ms (no overlap) = 300ms total
        assert_eq!(transition.combined_duration_ms(), 300);
    }

    #[test]
    fn test_shared_axis_overlap() {
        let transition = TransitionAnimation::SharedAxisX {
            enter_duration_ms: 200,
            exit_duration_ms: 100,
            overlap_ms: 50,
            easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            distance: 45.0,
        };

        // Exit 100ms, enter starts at 50ms (overlap 50ms), total = 50 + 200 = 250ms
        assert_eq!(transition.combined_duration_ms(), 250);
    }

    #[test]
    fn test_fade_through_sequential() {
        let transition = TransitionAnimation::FadeThrough {
            exit_duration_ms: 100,
            enter_duration_ms: 200,
            exit_easing: Easing::Linear,
            enter_easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0),
            enter_scale_from: 0.92,
        };

        // 100ms exit + 200ms enter = 300ms
        assert_eq!(transition.combined_duration_ms(), 300);
    }

    #[test]
    fn test_crossfade_duration() {
        let transition = TransitionAnimation::CrossFade {
            duration_ms: 500,
            easing: Easing::EaseInOutQuad,
        };

        assert_eq!(transition.combined_duration_ms(), 500);
    }

    #[test]
    fn test_timelines_created() {
        let transition = TransitionAnimation::CrossFade {
            duration_ms: 300,
            easing: Easing::Linear,
        };

        let exit = transition.exit_timeline();
        let enter = transition.enter_timeline();

        // Timelines should be created (basic smoke test)
        assert_eq!(exit.total_duration(), Duration::from_millis(300));
        assert_eq!(enter.total_duration(), Duration::from_millis(300));
    }
}
