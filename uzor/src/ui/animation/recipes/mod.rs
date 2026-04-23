//! Animation recipes module
//!
//! Pre-configured animation patterns for common UI interactions.
//! Based on research from Material Design 3, iOS HIG, Framer Motion, and web animation best practices.

pub mod buttons;
pub mod charts;
pub mod lists;
pub mod loading;
pub mod modals;
pub mod scroll;
pub mod toasts;
pub mod transitions;

// Re-export main types only (avoid ambiguous glob collisions)
pub use buttons::ButtonAnimation;
pub use charts::ChartAnimation;
pub use lists::ListAnimation;
pub use loading::LoadingAnimation;
pub use modals::ModalAnimation;
pub use scroll::ScrollAnimation;
pub use toasts::ToastAnimation;
pub use transitions::TransitionAnimation;
