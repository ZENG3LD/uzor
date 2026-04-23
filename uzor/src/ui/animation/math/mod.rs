//! Animation math — pure physics and interpolation primitives.
//! No widget knowledge. Coordinator and recipes live in ui/animation/.

pub mod blend;
pub mod color;
pub mod decay;
pub mod easing;
pub mod layers;
pub mod path;
pub mod scroll;
pub mod spring;
pub mod stagger;
pub mod stroke;
pub mod timeline;

pub use blend::{
    blend, blend_weighted, resolve_layers, AnimationLayer, AnimationSlot, AnimationTransition,
    CompositeMode, InterruptionStrategy,
};
pub use color::{Color, ColorSpace, Oklab, Oklch};
pub use decay::Decay;
pub use easing::{Easing, StepPosition};
pub use layers::{LayerStack, ManagedLayer};
pub use path::{MotionPath, PathSample, PathSegment, Point};
pub use scroll::{ParallaxLayer, ScrollTimeline, ScrollTween, ViewTimeline};
pub use spring::Spring;
pub use stagger::{
    DistanceMetric, GridOrigin, GridStagger, LinearStagger, StaggerOrigin,
};
pub use stroke::{StrokeAnimation, StrokeState};
pub use timeline::{Animatable, Position, Timeline, TimelinePlayback, Tween};
