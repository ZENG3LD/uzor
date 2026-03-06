//! Toast and notification animation recipes
//!
//! Pre-configured toast, snackbar, alert banner, and feedback animations following
//! Material Design, iOS, Sonner, React ecosystem, and web best practices.
//!
//! Based on research from:
//! - Material Design 3 Snackbar specifications
//! - iOS notification banners
//! - Sonner by Emil Kowalski
//! - React-Toastify, React Hot Toast, Radix UI Toast
//! - Chakra UI, Framer Motion patterns

pub mod types;
pub mod presets;
pub mod defaults;
pub mod builders;

pub use types::*;
pub use presets::*;
pub use defaults::*;
pub use builders::*;
