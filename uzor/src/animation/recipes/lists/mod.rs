//! List and stagger animation recipes
//!
//! Pre-configured animations for lists, grids, and staggered reveals.
//! Based on research from AnimeJS, GSAP, Framer Motion, and modern web animation patterns.

pub mod types;
pub mod presets;
pub mod defaults;
pub mod builders;

// Re-export main types
pub use types::ListAnimation;

// Re-export preset factory functions
pub use presets::{
    anime_grid_ripple, cascade_dramatic, cascade_fade_in, cascade_fast,
    checkerboard_reveal, diagonal_sweep, expand_collapse, flip_reorder,
    framer_stagger_children, grid_compact, grid_from_corners, grid_large,
    grid_wave_from_corner, masonry_random, scale_pop_stagger,
    slide_from_left, slide_from_right, snake_pattern, spiral_reveal,
};

// Re-export default structs
pub use defaults::{
    CascadeFadeInDefaults, CheckerboardRevealDefaults, DiagonalSweepDefaults,
    ExpandCollapseDefaults, FlipReorderDefaults, FramerStaggerDefaults,
    GridRippleDefaults, GridWaveDefaults, MasonryLoadDefaults,
    ScalePopInDefaults, SlideFromSideDefaults, SnakePatternDefaults,
    SpiralRevealDefaults,
};

// Re-export builder structs
pub use builders::{
    CascadeFadeInBuilder, DiagonalSweepBuilder, GridRippleBuilder,
    GridWaveBuilder, ScalePopInBuilder, SlideFromSideBuilder,
    SpiralRevealBuilder,
};
