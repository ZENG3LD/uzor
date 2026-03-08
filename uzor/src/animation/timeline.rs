//! Timeline system for orchestrating multiple animations
//!
//! GSAP-inspired timeline for sequencing, overlapping, and controlling
//! animations with precise timing.

use super::easing::Easing;
use std::collections::HashMap;
use std::time::Duration;

/// Trait for types that can be animated (interpolated)
pub trait Animatable: Clone + Send + Sync + 'static {
    /// Linear interpolation between self and target
    /// t is normalized (0.0 to 1.0)
    fn lerp(&self, target: &Self, t: f64) -> Self;
}

impl Animatable for f64 {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        self + (target - self) * t
    }
}

impl Animatable for f32 {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        self + (target - self) * t as f32
    }
}

impl Animatable for (f64, f64) {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        (self.0.lerp(&target.0, t), self.1.lerp(&target.1, t))
    }
}

impl Animatable for (f64, f64, f64, f64) {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        (
            self.0.lerp(&target.0, t),
            self.1.lerp(&target.1, t),
            self.2.lerp(&target.2, t),
            self.3.lerp(&target.3, t),
        )
    }
}

/// A single animation from start to end value
#[derive(Debug, Clone)]
pub struct Tween<T: Animatable> {
    pub from: T,
    pub to: T,
    pub duration: Duration,
    pub easing: Easing,
    pub delay: Duration,
}

impl<T: Animatable> Tween<T> {
    /// Create a new tween from start to end value
    pub fn new(from: T, to: T) -> Self {
        Self {
            from,
            to,
            duration: Duration::from_secs(1),
            easing: Easing::default(),
            delay: Duration::ZERO,
        }
    }

    /// Set the duration of the tween
    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = d;
        self
    }

    /// Set the easing function
    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    /// Set the delay before the tween starts
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    /// Evaluate tween at time t (seconds from start, INCLUDING delay)
    /// Returns None if before delay, Some(value) during/after animation
    pub fn evaluate(&self, elapsed: Duration) -> Option<T> {
        if elapsed < self.delay {
            return None;
        }

        let t = elapsed.saturating_sub(self.delay);

        if t >= self.duration {
            return Some(self.to.clone());
        }

        let progress = t.as_secs_f64() / self.duration.as_secs_f64();
        let eased = self.easing.ease(progress);

        Some(self.from.lerp(&self.to, eased))
    }

    /// Is the tween complete at this time?
    pub fn is_complete(&self, elapsed: Duration) -> bool {
        elapsed >= self.total_duration()
    }

    /// Total duration including delay
    pub fn total_duration(&self) -> Duration {
        self.delay + self.duration
    }
}

/// Position in timeline for inserting entries
#[derive(Debug, Clone)]
pub enum Position {
    /// Absolute time from timeline start
    Absolute(Duration),
    /// Relative to end of previous entry: "+=100ms" or "-=50ms"
    AfterPrevious(Duration),
    /// At label
    AtLabel(String),
}

/// A single entry in the timeline
#[derive(Debug, Clone)]
struct TimelineEntry {
    start: Duration,
    duration: Duration,
    id: u64,
}

/// Orchestrates multiple animations with precise timing
#[derive(Debug, Clone)]
pub struct Timeline {
    entries: Vec<TimelineEntry>,
    labels: HashMap<String, Duration>,
    total_duration: Duration,
    repeat: u32,
    yoyo: bool,
    speed: f64,
    next_id: u64,
}

impl Timeline {
    /// Create a new empty timeline
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            labels: HashMap::new(),
            total_duration: Duration::ZERO,
            repeat: 0,
            yoyo: false,
            speed: 1.0,
            next_id: 0,
        }
    }

    /// Add a tween at the given position
    /// Returns entry id for later reference
    pub fn add(&mut self, duration: Duration, position: Position) -> u64 {
        let start = self.resolve_position(position);
        let id = self.next_id;
        self.next_id += 1;

        self.entries.push(TimelineEntry {
            start,
            duration,
            id,
        });

        // Update total duration
        let end = start + duration;
        if end > self.total_duration {
            self.total_duration = end;
        }

        // Keep entries sorted by start time
        self.entries.sort_by_key(|e| e.start);

        id
    }

    /// Add a label at current position (end of timeline)
    pub fn label(&mut self, name: &str) -> &mut Self {
        self.labels.insert(name.to_string(), self.total_duration);
        self
    }

    /// Add a label at specific time
    pub fn label_at(&mut self, name: &str, time: Duration) -> &mut Self {
        self.labels.insert(name.to_string(), time);
        self
    }

    /// Set repeat count (0 = no repeat, play once)
    pub fn repeat(mut self, count: u32) -> Self {
        self.repeat = count;
        self
    }

    /// Enable yoyo (reverse on repeat)
    pub fn yoyo(mut self, enabled: bool) -> Self {
        self.yoyo = enabled;
        self
    }

    /// Set playback speed multiplier
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Total timeline duration (without repeat)
    pub fn total_duration(&self) -> Duration {
        self.total_duration
    }

    /// Resolve a Position to an absolute Duration
    fn resolve_position(&self, position: Position) -> Duration {
        match position {
            Position::Absolute(time) => time,
            Position::AfterPrevious(offset) => {
                if offset.as_nanos() > 0 {
                    self.total_duration + offset
                } else {
                    self.total_duration.saturating_sub(offset.abs_diff(Duration::ZERO))
                }
            }
            Position::AtLabel(label) => {
                *self.labels.get(&label).unwrap_or(&Duration::ZERO)
            }
        }
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Playback direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayDirection {
    Forward,
    Reverse,
}

/// Tracks playback state of a timeline
pub struct TimelinePlayback {
    timeline: Timeline,
    elapsed: Duration,
    playing: bool,
    direction: PlayDirection,
    current_repeat: u32,
}

impl TimelinePlayback {
    /// Create a new playback instance from a timeline
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            elapsed: Duration::ZERO,
            playing: false,
            direction: PlayDirection::Forward,
            current_repeat: 0,
        }
    }

    /// Advance time by dt
    pub fn tick(&mut self, dt: Duration) {
        if !self.playing {
            return;
        }

        let dt_scaled = dt.mul_f64(self.timeline.speed);

        match self.direction {
            PlayDirection::Forward => {
                self.elapsed += dt_scaled;

                // Check if we've reached the end
                if self.elapsed >= self.timeline.total_duration {
                    if self.current_repeat < self.timeline.repeat {
                        self.current_repeat += 1;

                        if self.timeline.yoyo {
                            self.direction = PlayDirection::Reverse;
                        } else {
                            self.elapsed = Duration::ZERO;
                        }
                    } else {
                        self.elapsed = self.timeline.total_duration;
                        self.playing = false;
                    }
                }
            }
            PlayDirection::Reverse => {
                if self.elapsed > dt_scaled {
                    self.elapsed -= dt_scaled;
                } else {
                    self.elapsed = Duration::ZERO;

                    if self.current_repeat < self.timeline.repeat {
                        self.current_repeat += 1;

                        if self.timeline.yoyo {
                            self.direction = PlayDirection::Forward;
                            self.elapsed = Duration::ZERO;
                        }
                    } else {
                        self.playing = false;
                    }
                }
            }
        }
    }

    /// Get progress of entry at given id (0.0..=1.0)
    /// Returns 0.0 if not started, 1.0 if complete
    pub fn progress_of(&self, entry_id: u64) -> f64 {
        let entry = self.timeline.entries.iter().find(|e| e.id == entry_id);

        match entry {
            Some(entry) => {
                if self.elapsed < entry.start {
                    0.0
                } else if self.elapsed >= entry.start + entry.duration {
                    1.0
                } else {
                    let local_time = self.elapsed.saturating_sub(entry.start);
                    local_time.as_secs_f64() / entry.duration.as_secs_f64()
                }
            }
            None => 0.0,
        }
    }

    /// Is a specific entry active at current time?
    pub fn is_active(&self, entry_id: u64) -> bool {
        let entry = self.timeline.entries.iter().find(|e| e.id == entry_id);

        match entry {
            Some(entry) => {
                self.elapsed >= entry.start && self.elapsed < entry.start + entry.duration
            }
            None => false,
        }
    }

    /// Is the entire timeline complete?
    pub fn is_complete(&self) -> bool {
        !self.playing && self.elapsed >= self.timeline.total_duration
    }

    /// Start playing
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Restart from beginning
    pub fn restart(&mut self) {
        self.elapsed = Duration::ZERO;
        self.playing = true;
        self.direction = PlayDirection::Forward;
        self.current_repeat = 0;
    }

    /// Reverse playback direction
    pub fn reverse(&mut self) {
        self.direction = match self.direction {
            PlayDirection::Forward => PlayDirection::Reverse,
            PlayDirection::Reverse => PlayDirection::Forward,
        };
    }

    /// Seek to specific time
    pub fn seek(&mut self, time: Duration) {
        self.elapsed = time.min(self.timeline.total_duration);
    }

    /// Get current elapsed time
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Is currently playing?
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Get playback direction
    pub fn direction(&self) -> PlayDirection {
        self.direction
    }
}
