//! Interactive component animations
//!
//! Provides animation state management for interactive UI components:
//! - ElasticSlider: Slider with elastic overflow and spring snap-back
//! - AnimatedList: Staggered entry/exit animations for list items
//! - SpotlightCard: Cursor-following spotlight effect
//! - ElectricBorder: Animated electric/lightning border effect
//!
//! All components are rendering-agnostic and compute animation state only.
//! The actual rendering is left to the UI framework.

pub mod animated_list;
pub mod elastic_slider;
pub mod electric_border;
pub mod spotlight;

pub use animated_list::{AnimatedList, ItemState};
pub use elastic_slider::{ElasticSlider, OverflowRegion};
pub use electric_border::ElectricBorder;
pub use spotlight::{SpotlightCard, SpotlightColor};
