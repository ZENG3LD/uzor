//! Page transition animation recipes
//!
//! Pre-configured page/view transition animations following Material Design, iOS,
//! and web best practices. Based on research from Material Motion, Apple HIG,
//! Framer Motion, and modern transition libraries.

pub mod types;
pub mod presets;
pub mod defaults;
pub mod builders;

// Re-export main types with explicit names to avoid glob conflicts
pub use types::{SlideDirection, TransitionAnimation};

// Re-export all presets
pub use presets::{
    circle_reveal, circle_reveal_from, cross_fade, cross_fade_fast, cross_fade_slow, ios_push,
    material_fade_through, material_shared_axis_x, material_shared_axis_y, parallax_slide,
    slide_over, slide_over_left, slide_over_top, stair_cascade, stair_cascade_bounce, zoom_in,
    zoom_out,
};

// Re-export default structs
pub use defaults::{
    CircleRevealDefaults, CrossFadeDefaults, FadeThroughDefaults, ParallaxSlideDefaults,
    PushDefaults, SharedAxisDefaults, SlideOverDefaults, StairCascadeDefaults, ZoomDefaults,
};

// Re-export builders
pub use builders::{
    CircleRevealBuilder, CrossFadeBuilder, FadeThroughBuilder, ParallaxSlideBuilder, PushBuilder,
    SharedAxisXBuilder, SharedAxisYBuilder, SlideOverBuilder, StairCascadeBuilder, ZoomBuilder,
};
