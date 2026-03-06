//! Scroll animation recipes
//!
//! Pre-configured scroll-driven and parallax animations following modern web patterns.
//! Based on research from GSAP ScrollTrigger, CSS Scroll-Driven Animations spec,
//! Locomotive Scroll, Apple product pages, and Motion.dev examples.
//!
//! # Architecture
//!
//! - **types**: Enum variants for each scroll animation pattern
//! - **presets**: Ready-to-use preset functions (12 common patterns)
//! - **defaults**: Default parameter values for each variant
//! - **builders**: Fluent builder API for custom configurations
//!
//! # Usage Examples
//!
//! ## Using Presets
//!
//! ```rust
//! use uzor_animation::recipes::scroll::presets::*;
//!
//! // Quick horizontal progress bar
//! let progress = progress_bar_horizontal();
//!
//! // 3-layer parallax hero
//! let parallax = parallax_hero();
//!
//! // Sticky header that shrinks
//! let header = sticky_shrink_header();
//! ```
//!
//! ## Using Builders
//!
//! ```rust
//! use uzor_animation::recipes::scroll::builders::*;
//! use uzor_animation::easing::Easing;
//! use std::time::Duration;
//!
//! // Custom progress bar
//! let progress = ProgressBarBuilder::new()
//!     .scroll_start(0.0)
//!     .scroll_end(5000.0)
//!     .easing(Easing::EaseInOutQuad)
//!     .build();
//!
//! // Custom number counter
//! let counter = NumberCounterBuilder::new()
//!     .from(0.0)
//!     .to(1500.0)
//!     .duration(Duration::from_millis(3000))
//!     .threshold(0.6)
//!     .build();
//! ```
//!
//! # Preset Catalog
//!
//! | Preset | Description | Use Case |
//! |--------|-------------|----------|
//! | `progress_bar_horizontal()` | 0-100% linear bar | Reading progress |
//! | `progress_ring()` | Circular SVG progress | Scroll-to-top button |
//! | `parallax_hero()` | 3-layer depth (0.3x, 0.6x, 1.0x) | Hero sections |
//! | `fade_in_on_enter()` | Opacity 0→1 on entry | Content reveals |
//! | `slide_up_on_enter()` | Slide + fade combo | Section entrances |
//! | `reveal_from_left()` | Horizontal slide-in | Side content |
//! | `sticky_shrink_header()` | Height 80→48px | Compact nav |
//! | `horizontal_pin_scroll()` | Pinned horizontal gallery | Card carousels |
//! | `number_counter()` | Count 0→target | Statistics |
//! | `color_shift_sections()` | Gradient transitions | Multi-section pages |
//! | `scale_on_scroll()` | Scale 0.8→1.0 | Image zoom |
//! | `parallax_text()` | Text layers at different speeds | Hero text |
//!
//! # Research Sources
//!
//! - **GSAP ScrollTrigger**: Industry-standard scroll animations
//! - **CSS Scroll-Driven Animations**: W3C spec implementation patterns
//! - **Locomotive Scroll**: Smooth scroll + parallax library
//! - **Apple Product Pages**: High-quality scroll experiences
//! - **Framer Motion**: React animation patterns
//! - **Motion.dev**: CSS-first animation platform
//! - **AnimeJS**: Stagger and timeline patterns
//!
//! See `research/recipes/06-scroll-parallax.md` for full details.

pub mod types;
pub mod presets;
pub mod defaults;
pub mod builders;

pub use types::*;
pub use presets::*;
pub use defaults::*;
pub use builders::*;
