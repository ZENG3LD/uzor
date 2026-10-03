//! Panel Application API data types.
//!
//! `PanelApp` and the string-id trait surface were removed in brief C1.
//! Toolbar / panel data types that embedders still share stay here.

mod toolbar;
mod types;

pub use toolbar::*;
pub use types::*;
